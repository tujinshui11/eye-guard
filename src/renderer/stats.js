
// ---- 用眼统计板块（手写 SVG 图表，零依赖）----

/// 秒 → 小时（1 位小数）
function toHours(sec) {
  return ((sec || 0) / 3600).toFixed(1);
}

/// 秒 → 人类可读（"2 小时 15 分" / "45 分钟"）
function humanDuration(sec) {
  const s = Math.max(0, sec || 0);
  if (s < 60) return s + ' 秒';
  const m = Math.floor(s / 60);
  if (m < 60) return m + ' 分钟';
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return rm === 0 ? h + ' 小时' : h + ' 小时 ' + rm + ' 分';
}

/// 今日 24 小时分布柱状图
function renderHourly(hourly) {
  const el = $('stats-hourly');
  if (!el) return;
  const data = Array.isArray(hourly) && hourly.length === 24 ? hourly : new Array(24).fill(0);
  const max = Math.max(1, ...data);
  const W = 620, H = 130, padL = 4, padB = 20, padT = 8;
  const barW = (W - padL * 2) / 24;
  const nowHour = new Date().getHours();

  let bars = '';
  data.forEach((v, h) => {
    const bh = Math.round(((H - padB - padT) * v) / max);
    const x = padL + h * barW;
    const y = H - padB - bh;
    const isNow = h === nowHour;
    const fill = v > 0 ? (isNow ? 'var(--accent)' : 'var(--accent-line)') : 'var(--fill-1)';
    bars += `<rect x="${(x + 1.5).toFixed(1)}" y="${y.toFixed(1)}" width="${(barW - 3).toFixed(1)}" height="${Math.max(2, bh)}" rx="2" fill="${fill}"></rect>`;
  });
  let labels = '';
  [0, 6, 12, 18].forEach((h) => {
    const x = padL + h * barW + barW / 2;
    labels += `<text x="${x.toFixed(1)}" y="${H - 5}" font-size="9" fill="var(--muted)" text-anchor="middle">${h}时</text>`;
  });
  el.innerHTML =
    `<svg viewBox="0 0 ${W} ${H}" width="100%" height="${H}" preserveAspectRatio="none" role="img" aria-label="今日各小时护眼时长分布">${bars}${labels}</svg>`;
}

/// 近 7 日趋势柱状图（含每日休息完成/跳过数）
function renderTrend(days) {
  const el = $('stats-trend');
  if (!el) return;
  const list = Array.isArray(days) ? days : [];
  if (list.length === 0) {
    el.innerHTML = '<p class="chart-empty">暂无数据——护眼生效后会逐日累计</p>';
    return;
  }
  const max = Math.max(1, ...list.map((d) => d.activeSeconds || 0));
  const W = 620, H = 150, padB = 34, padT = 10;
  const gap = 10;
  const barW = (W - gap * (list.length + 1)) / list.length;

  let bars = '';
  list.forEach((d, i) => {
    const bh = Math.round(((H - padB - padT) * (d.activeSeconds || 0)) / max);
    const x = gap + i * (barW + gap);
    const y = H - padB - bh;
    const label = (d.date || '').slice(5); // MM-DD
    const hours = toHours(d.activeSeconds);
    bars += `<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${barW.toFixed(1)}" height="${Math.max(2, bh)}" rx="3" fill="var(--accent-line)"></rect>`;
    bars += `<text x="${(x + barW / 2).toFixed(1)}" y="${(y - 3).toFixed(1)}" font-size="9" fill="var(--accent)" text-anchor="middle">${bh > 14 ? hours : ''}</text>`;
    bars += `<text x="${(x + barW / 2).toFixed(1)}" y="${H - 18}" font-size="9" fill="var(--muted)" text-anchor="middle">${label}</text>`;
    bars += `<text x="${(x + barW / 2).toFixed(1)}" y="${H - 5}" font-size="9" fill="var(--muted)" text-anchor="middle">${d.breaksCompleted || 0}✓</text>`;
  });
  el.innerHTML = `<svg viewBox="0 0 ${W} ${H}" width="100%" height="${H}" role="img" aria-label="近 7 日护眼时长趋势">${bars}</svg>`;
}

/// 休息明细行
function renderBreaksDetail(days) {
  const el = $('stats-breaks-detail');
  if (!el) return;
  const list = Array.isArray(days) ? days : [];
  const withData = list.filter((d) => (d.breaksCompleted || 0) + (d.breaksSkipped || 0) > 0);
  if (withData.length === 0) {
    el.innerHTML = '';
    return;
  }
  el.innerHTML = withData
    .map(
      (d) =>
        `<span class="breaks-item">${(d.date || '').slice(5)}：完成 ${d.breaksCompleted || 0} · 跳过 ${d.breaksSkipped || 0}</span>`
    )
    .join('');
}

/// 加载并渲染统计（切到统计板块时调用）
function refreshUsageStats() {
  window.eyeGuard
    .usageSummary()
    .then((s) => {
      if (!s) return;
      const today = s.today || {};
      const week = s.week || {};
      const set = (id, text) => {
        const el = $(id);
        if (el) el.textContent = text;
      };
      set('stats-today-hours', toHours(today.activeSeconds));
      set('stats-week-hours', toHours(week.activeSeconds));
      set('stats-breaks', String(week.breaksCompleted || 0));
      set('stats-hourly-hint', '今日 ' + humanDuration(today.activeSeconds));
      set(
        'stats-trend-hint',
        '本周 ' + humanDuration(week.activeSeconds) + ' · 完成 ' + (week.breaksCompleted || 0) + ' 次'
      );
      renderHourly(today.hourly);
      renderTrend(week.days);
      renderBreaksDetail(week.days);
    })
    .catch(() => {
      const el = $('stats-trend');
      if (el) el.innerHTML = '<p class="chart-empty">统计数据读取失败</p>';
    });
}
