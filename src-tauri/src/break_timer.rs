//! BreakTimer：休息提醒状态机——对照移植 `src/main/breakTimer.js`
//!
//!   working --到时--> alerting --[开始休息]--> resting --到时--> working
//!      ^                |--[推迟]--> working(短计时)
//!      |                |--[跳过]--> working(完整计时)
//!      +--[暂停]--< [恢复]（暂停期间不触发；恢复后按剩余时间继续）
//!
//! 时间戳基准（非 tick 累计）：睡眠/唤醒、时钟跳跃后仍正确。
//! 时钟由调用方显式传入（对照 JS 的注入 now()）——生产为系统时钟，测试为假时钟。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakState {
    Idle,
    Working,
    Alerting,
    Resting,
    Paused,
}

impl BreakState {
    pub fn as_str(&self) -> &'static str {
        match self {
            BreakState::Idle => "idle",
            BreakState::Working => "working",
            BreakState::Alerting => "alerting",
            BreakState::Resting => "resting",
            BreakState::Paused => "paused",
        }
    }
}

/// 状态快照（对照 getState()）
#[derive(Debug, Clone)]
pub struct BreakSnapshot {
    pub state: BreakState,
    pub remaining_seconds: Option<i64>,
    pub paused_until: Option<i64>,
}

pub struct BreakTimer {
    pub work_seconds: u64,
    pub break_seconds: u64,
    postpone_seconds: u64,
    state: BreakState,
    phase_end_ts: Option<i64>,
    paused_state: Option<BreakState>,
    paused_remain_ms: Option<i64>,
    paused_until_ts: Option<i64>,
}

impl Default for BreakTimer {
    fn default() -> Self {
        Self {
            work_seconds: 1200, // 默认对齐 AAO 20-20-20
            break_seconds: 20,
            postpone_seconds: 300,
            state: BreakState::Idle,
            phase_end_ts: None,
            paused_state: None,
            paused_remain_ms: None,
            paused_until_ts: None,
        }
    }
}

impl BreakTimer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 配置工作/休息时长（运行中调整工作时长：按剩余时间重排当前阶段）
    pub fn configure(&mut self, work_seconds: Option<u64>, break_seconds: Option<u64>, now: i64) {
        if let Some(w) = work_seconds {
            if w > 0 {
                self.work_seconds = w;
            }
        }
        if let Some(b) = break_seconds {
            if b > 0 {
                self.break_seconds = b;
            }
        }
        if self.state == BreakState::Working {
            if let Some(end) = self.phase_end_ts {
                // 对照 JS：elapsed = 总时长 - max(0, 剩余)；newEnd = now + max(1000ms, 总时长 - max(0, elapsed))
                let elapsed_ms = self.work_seconds as i64 * 1000 - (end - now).max(0);
                let new_end =
                    now + (self.work_seconds as i64 * 1000 - elapsed_ms.max(0)).max(1000);
                self.phase_end_ts = Some(new_end);
            }
        }
    }

    pub fn start(&mut self, now: i64) {
        self.enter_working(None, now);
    }

    /// 停止（用于退出/禁用场景）
    pub fn stop(&mut self) {
        self.state = BreakState::Idle;
        self.phase_end_ts = None;
        self.paused_state = None;
        self.paused_remain_ms = None;
        self.paused_until_ts = None;
    }

    fn enter_working(&mut self, seconds: Option<u64>, now: i64) {
        self.state = BreakState::Working;
        self.phase_end_ts = Some(now + seconds.unwrap_or(self.work_seconds) as i64 * 1000);
    }

    /// alerting → resting（开始休息）
    pub fn begin_rest(&mut self, now: i64) -> bool {
        if self.state != BreakState::Alerting {
            return false;
        }
        self.state = BreakState::Resting;
        self.phase_end_ts = Some(now + self.break_seconds as i64 * 1000);
        true
    }

    /// alerting → working（推迟 postpone_seconds）
    pub fn postpone(&mut self, now: i64) -> bool {
        if self.state != BreakState::Alerting {
            return false;
        }
        self.enter_working(Some(self.postpone_seconds), now);
        true
    }

    /// alerting/resting → working（完整工作计时）
    pub fn skip(&mut self, now: i64) -> bool {
        if self.state != BreakState::Alerting && self.state != BreakState::Resting {
            return false;
        }
        self.enter_working(None, now);
        true
    }

    /// 暂停提醒；until_ts = 自动恢复时间戳（None = 手动恢复）
    pub fn pause(&mut self, now: i64, until_ts: Option<i64>) -> bool {
        if self.state == BreakState::Paused || self.state == BreakState::Idle {
            return false;
        }
        self.paused_state = Some(self.state);
        self.paused_remain_ms = self.phase_end_ts.map(|e| (e - now).max(0));
        self.state = BreakState::Paused;
        self.paused_until_ts = until_ts;
        true
    }

    pub fn resume(&mut self, now: i64) -> bool {
        if self.state != BreakState::Paused {
            return false;
        }
        let back = self.paused_state.unwrap_or(BreakState::Working);
        self.state = back;
        if let Some(remain) = self.paused_remain_ms {
            self.phase_end_ts = Some(now + remain);
        }
        self.paused_state = None;
        self.paused_until_ts = None;
        true
    }

    /// 外部定时器驱动（建议 1s 间隔）；时间戳基准，tick 频率不影响正确性
    pub fn tick(&mut self, now: i64) {
        if self.state == BreakState::Paused {
            if let Some(until) = self.paused_until_ts {
                if now >= until {
                    self.resume(now);
                }
            }
            return;
        }
        if let Some(end) = self.phase_end_ts {
            if now >= end {
                match self.state {
                    BreakState::Working => {
                        self.state = BreakState::Alerting;
                        self.phase_end_ts = None;
                    }
                    BreakState::Resting => {
                        self.state = BreakState::Working;
                        self.phase_end_ts = Some(now + self.work_seconds as i64 * 1000);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn get_state(&self, now: i64) -> BreakSnapshot {
        let counting = matches!(self.state, BreakState::Working | BreakState::Resting);
        let remaining_seconds = if counting {
            self.phase_end_ts
                .map(|e| ((e - now) as f64 / 1000.0).ceil() as i64)
                .map(|r| r.max(0))
        } else {
            None
        };
        BreakSnapshot {
            state: self.state,
            remaining_seconds,
            paused_until: self.paused_until_ts,
        }
    }
}

// ============================================================
// 测试——对照移植 tests/breakTimer.test.js（8 用例，假时钟）
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    struct Harness {
        timer: BreakTimer,
        t: i64,
    }

    impl Harness {
        fn new() -> Self {
            let mut timer = BreakTimer::new();
            timer.configure(Some(100), Some(20), 0);
            Self { timer, t: 0 }
        }
        fn set(&mut self, v: i64) {
            self.t = v;
        }
        fn state(&self) -> BreakState {
            self.timer.get_state(self.t).state
        }
    }

    #[test]
    fn working_to_alerting_on_timeout() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        assert_eq!(h.state(), BreakState::Working);

        h.set(99_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working);

        h.set(100_000);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Alerting);
    }

    #[test]
    fn full_cycle_rest_then_working_then_alert() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        h.set(100_000);
        h.timer.tick(h.t);

        assert!(h.timer.begin_rest(h.t));
        assert_eq!(h.state(), BreakState::Resting);

        h.set(119_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Resting);

        h.set(120_000);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working);

        h.set(120_000 + 99_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working);
        h.set(120_000 + 100_000);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Alerting);
    }

    #[test]
    fn postpone_delays_by_300s() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        h.set(100_000);
        h.timer.tick(h.t);

        assert!(h.timer.postpone(h.t));
        assert_eq!(h.state(), BreakState::Working);

        h.set(100_000 + 299_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working);

        h.set(100_000 + 300_000);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Alerting);
    }

    #[test]
    fn skip_returns_to_full_work_cycle() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        h.set(100_000);
        h.timer.tick(h.t);

        assert!(h.timer.skip(h.t));
        h.set(100_000 + 99_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working);
        h.set(100_000 + 100_000);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Alerting);
    }

    #[test]
    fn pause_no_trigger_then_auto_resume_with_remaining() {
        let mut h = Harness::new();
        h.timer.start(h.t);

        h.set(50_000);
        assert!(h.timer.pause(h.t, Some(50_000 + 3_600_000))); // 暂停至 1 小时后
        assert_eq!(h.state(), BreakState::Paused);

        // 时间大跳（跨过原工作到点与暂停截止）
        h.set(50_000 + 3_600_001);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working, "暂停到点自动恢复");

        // 剩余 50s
        let resume_at = 50_000 + 3_600_001;
        h.set(resume_at + 49_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Working);
        h.set(resume_at + 50_000);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Alerting);
    }

    #[test]
    fn manual_resume_no_negative_remaining_after_clock_jump() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        h.set(50_000);
        assert!(h.timer.pause(h.t, None)); // 手动暂停
        h.set(9_999_999);
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Paused);

        assert!(h.timer.resume(h.t));
        let st = h.timer.get_state(h.t);
        assert_eq!(st.state, BreakState::Working);
        assert!(
            st.remaining_seconds.unwrap() >= 0,
            "剩余 {:?}",
            st.remaining_seconds
        );
        assert!(
            st.remaining_seconds.unwrap() <= 50,
            "剩余应约 50s：{:?}",
            st.remaining_seconds
        );
    }

    #[test]
    fn long_sleep_jump_triggers_alert() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        h.set(0);
        h.set(30_000_000); // 睡 8 小时后唤醒
        h.timer.tick(h.t);
        assert_eq!(h.state(), BreakState::Alerting);
    }

    #[test]
    fn skip_from_resting_returns_to_work() {
        let mut h = Harness::new();
        h.timer.start(h.t);
        h.set(100_000);
        h.timer.tick(h.t);
        h.timer.begin_rest(h.t);
        assert_eq!(h.state(), BreakState::Resting);

        assert!(h.timer.skip(h.t));
        assert_eq!(h.state(), BreakState::Working);
    }
}
