//! 感光监测分析器——对照移植 `src/main/ambient.js` 的 LumaAnalyzer
//!
//! 纯计算模块：无 I/O。输入为逐次采样的环境亮度（luma），输出为状态事件。
//! 判定模型：EMA 平滑 → normal 态样本 P75 基线 → 骤暗阈值/回滞恢复 → 冷却抑制。

/// 默认参数（对照 DEFAULT_OPTIONS）
#[derive(Debug, Clone, Copy)]
pub struct AnalyzerOptions {
    pub ema_alpha: f64,
    pub baseline_samples: usize,
    pub drop_threshold_percent: f64,
    pub cooldown_ms: i64,
}

impl Default for AnalyzerOptions {
    fn default() -> Self {
        Self {
            ema_alpha: 0.3,
            baseline_samples: 60,
            drop_threshold_percent: 35.0,
            cooldown_ms: 900_000,
        }
    }
}

/// 基线样本不足此数时不作判定
const MIN_BASELINE_SAMPLES: usize = 5;

/// feed 的输出事件
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedEvent {
    Darker,
    Recovered,
}

/// 分析器状态快照（供 UI / 调试）
#[derive(Debug, Clone, Copy)]
pub struct AnalyzerSnapshot {
    pub smooth: Option<f64>,
    pub baseline: Option<f64>,
    pub dark: bool,
    pub samples: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Normal,
    Dark,
}

/// 百分位（线性插值）。入参须为已升序排序的数组。
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return sorted[0];
    }
    let idx = (n - 1) as f64 * p;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

pub struct LumaAnalyzer {
    opts: AnalyzerOptions,
    smooth: Option<f64>,
    baseline: Option<f64>,
    state: State,
    last_darker_at: i64,
    samples: u64,
    baseline_buffer: Vec<f64>,
}

impl LumaAnalyzer {
    pub fn new(opts: AnalyzerOptions) -> Self {
        Self {
            opts,
            smooth: None,
            baseline: None,
            state: State::Normal,
            last_darker_at: 0,
            samples: 0,
            baseline_buffer: Vec::new(),
        }
    }

    /// 喂入一次亮度采样；返回事件（None / Darker / Recovered）
    pub fn feed(&mut self, luma: f64, now_ms: i64) -> Option<FeedEvent> {
        // 1) 平滑：首个样本直接取原值，后续 EMA
        self.smooth = Some(match self.smooth {
            None => luma,
            Some(s) => self.opts.ema_alpha * luma + (1.0 - self.opts.ema_alpha) * s,
        });
        self.samples += 1;

        // 2) 基线只在 normal 态吸收平滑样本；dark 期间不吸收暗值
        if self.state == State::Normal {
            self.baseline_buffer.push(self.smooth.unwrap());
            if self.baseline_buffer.len() > self.opts.baseline_samples {
                self.baseline_buffer.remove(0);
            }
        }

        // 3) 样本不足，不判定
        if self.baseline_buffer.len() < MIN_BASELINE_SAMPLES {
            return None;
        }

        // 4) 基线 = normal 样本 P75
        let mut sorted = self.baseline_buffer.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let baseline = percentile(&sorted, 0.75);
        self.baseline = Some(baseline);

        let drop = self.opts.drop_threshold_percent;
        let dark_threshold = baseline * (1.0 - drop / 100.0);
        let recover_threshold = baseline * (1.0 - drop / 200.0);
        let smooth = self.smooth.unwrap();

        // 5) 状态机
        if self.state == State::Normal {
            if smooth < dark_threshold {
                self.state = State::Dark;
                // 冷却期内仍转入 dark，但不重复发 darker 事件
                if now_ms - self.last_darker_at >= self.opts.cooldown_ms {
                    self.last_darker_at = now_ms;
                    return Some(FeedEvent::Darker);
                }
            }
            return None;
        }

        // state == Dark
        if smooth > recover_threshold {
            self.state = State::Normal;
            return Some(FeedEvent::Recovered); // recovered 不受冷却限制
        }
        None
    }

    /// 供 UI / 调试的只读快照
    pub fn snapshot(&self) -> AnalyzerSnapshot {
        AnalyzerSnapshot {
            smooth: self.smooth,
            baseline: self.baseline,
            dark: self.state == State::Dark,
            samples: self.samples,
        }
    }
}

// ============================================================
// 测试——对照移植 tests/ambientAnalyzer.test.js
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    const STEP: i64 = 60_000; // 采样间隔 60s
    const T0: i64 = 1_700_000_000_000; // 大基值，确保首个 darker 不被 cooldown(last_darker_at=0) 误伤

    /// 顺序喂样器：每次 feed 自动推进 now，可手动跳转 now
    struct Feeder<'a> {
        analyzer: &'a mut LumaAnalyzer,
        t: i64,
    }

    impl<'a> Feeder<'a> {
        fn new(analyzer: &'a mut LumaAnalyzer) -> Self {
            Self { analyzer, t: T0 }
        }
        fn feed(&mut self, luma: f64) -> Option<FeedEvent> {
            let ev = self.analyzer.feed(luma, self.t);
            self.t += STEP;
            ev
        }
        fn forward(&mut self, ms: i64) {
            self.t += ms;
        }
        fn snapshot(&self) -> AnalyzerSnapshot {
            self.analyzer.snapshot()
        }
    }

    // T8.1 恒定光照：喂 100 次恒值不触发任何事件
    #[test]
    fn constant_light_no_events() {
        let mut a = LumaAnalyzer::new(AnalyzerOptions::default());
        let mut f = Feeder::new(&mut a);
        for i in 0..100 {
            assert_eq!(f.feed(100.0), None, "第 {i} 次不应有事件");
        }
        let st = f.snapshot();
        assert!(!st.dark);
        assert!(st.smooth.is_some());
        assert!(st.baseline.is_some());
        assert_eq!(st.samples, 100);
    }

    // T8.1 骤降触发：稳定 100 后骤降 50 持续 → 出现 darker
    #[test]
    fn sudden_drop_triggers_darker() {
        let mut a = LumaAnalyzer::new(AnalyzerOptions::default());
        let mut f = Feeder::new(&mut a);
        let mut events = Vec::new();
        for _ in 0..20 {
            events.push(f.feed(100.0));
        }
        for _ in 0..30 {
            events.push(f.feed(50.0));
        }
        assert!(
            events.contains(&Some(FeedEvent::Darker)),
            "骤降应触发 darker，实际事件序列：{events:?}"
        );
        assert!(f.snapshot().dark);
    }

    // T8.1 降幅不足阈值不触发：100 → 80（降 20% < 35%）无 darker
    #[test]
    fn insufficient_drop_no_event() {
        let mut a = LumaAnalyzer::new(AnalyzerOptions::default());
        let mut f = Feeder::new(&mut a);
        let mut events = Vec::new();
        for _ in 0..20 {
            events.push(f.feed(100.0));
        }
        for _ in 0..30 {
            events.push(f.feed(80.0));
        }
        assert!(
            !events.contains(&Some(FeedEvent::Darker)),
            "降幅不足不应触发 darker，实际事件序列：{events:?}"
        );
        assert!(!f.snapshot().dark);
    }

    // T8.2 回升恢复：darker 后回到 100 → recovered
    #[test]
    fn recovery_after_darker() {
        let mut a = LumaAnalyzer::new(AnalyzerOptions::default());
        let mut f = Feeder::new(&mut a);
        for _ in 0..20 {
            f.feed(100.0);
        }
        let mut saw_darker = false;
        for _ in 0..30 {
            if f.feed(50.0) == Some(FeedEvent::Darker) {
                saw_darker = true;
                break;
            }
        }
        assert!(saw_darker, "前置条件：应先出现 darker");

        let mut saw_recovered = false;
        for _ in 0..30 {
            if f.feed(100.0) == Some(FeedEvent::Recovered) {
                saw_recovered = true;
                break;
            }
        }
        assert!(saw_recovered, "回升应触发 recovered");
        assert!(!f.snapshot().dark);
    }

    // T8.2 冷却：冷却期内再次骤降不重复 darker，超时后可再触发
    #[test]
    fn cooldown_suppresses_repeat_then_allows() {
        let mut a = LumaAnalyzer::new(AnalyzerOptions {
            cooldown_ms: 900_000,
            ..AnalyzerOptions::default()
        });
        let mut f = Feeder::new(&mut a);
        for _ in 0..30 {
            f.feed(100.0);
        }

        let mut first = false;
        for _ in 0..30 {
            if f.feed(50.0) == Some(FeedEvent::Darker) {
                first = true;
                break;
            }
        }
        assert!(first, "前置条件：应出现首个 darker");

        let mut rec = false;
        for _ in 0..30 {
            if f.feed(100.0) == Some(FeedEvent::Recovered) {
                rec = true;
                break;
            }
        }
        assert!(rec, "前置条件：应恢复");

        // 冷却期内再次骤降 → 无第二个 darker，但状态仍转入 dark
        let mut suppressed = Vec::new();
        for _ in 0..30 {
            suppressed.push(f.feed(50.0));
        }
        assert!(
            !suppressed.contains(&Some(FeedEvent::Darker)),
            "冷却期内不应重复 darker，实际序列：{suppressed:?}"
        );
        assert!(f.snapshot().dark, "冷却内应仍转入 dark，仅事件被抑制");

        // 再次恢复，并推进 now 越过 cooldown_ms
        let mut rec2 = false;
        for _ in 0..30 {
            if f.feed(100.0) == Some(FeedEvent::Recovered) {
                rec2 = true;
                break;
            }
        }
        assert!(rec2, "第二次恢复应成功");
        f.forward(1_000_000); // 远超 cooldown(900000)

        let mut again = Vec::new();
        for _ in 0..30 {
            again.push(f.feed(50.0));
        }
        assert!(
            again.contains(&Some(FeedEvent::Darker)),
            "冷却结束后应能再次触发 darker，实际序列：{again:?}"
        );
    }

    // T8.2 基线不吸黑：dark 期间喂暗值基线不变，回升仍能 recovered
    #[test]
    fn baseline_does_not_absorb_dark() {
        let mut a = LumaAnalyzer::new(AnalyzerOptions::default());
        let mut f = Feeder::new(&mut a);
        for _ in 0..30 {
            f.feed(100.0);
        }

        let mut dark = false;
        for _ in 0..30 {
            if f.feed(40.0) == Some(FeedEvent::Darker) {
                dark = true;
                break;
            }
        }
        assert!(dark, "前置条件：应先出现 darker");

        let baseline_at_dark = f.snapshot().baseline;

        for _ in 0..40 {
            f.feed(40.0);
        }
        assert!(f.snapshot().dark);
        assert_eq!(
            f.snapshot().baseline,
            baseline_at_dark,
            "dark 期间基线不得吸收暗值"
        );

        let mut rec = false;
        for _ in 0..30 {
            if f.feed(100.0) == Some(FeedEvent::Recovered) {
                rec = true;
                break;
            }
        }
        assert!(rec, "回升应能正常 recovered");
    }
}
