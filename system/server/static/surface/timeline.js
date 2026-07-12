// Swimlane (Timeline view): skinny per-project lanes with two-layer duration
// bars (ghost track + solid worked-day segments, or full solid when the ghost
// toggle is off), clustered deliverable dots, material-activity + future
// markers, month axis, and a summary bar. Ported from the pre-v2 calendar
// renderer and slimmed for many-project scale.

import { el, keycapEl, navigate } from './api.js?v=5';
import { colorFor, ghostTint, deliverableDot, attachPopover, popContent } from './ribbons.js?v=5';

const DAY_MS = 86400000;
const MIN_PX_PER_DAY = 11;  // per-day numbers/ticks whenever a 2-digit label fits
const LABEL_COL_PX = 260;   // keep in sync with the .calendar-row grid template

function day(value) {
  const text = String(value || '').slice(0, 10);
  return /^\d{4}-\d{2}-\d{2}$/.test(text) ? text : null;
}

function dateOf(value) {
  const normalized = day(value);
  return normalized ? new Date(`${normalized}T00:00:00Z`) : null;
}

function isoDate(value) {
  return value.toISOString().slice(0, 10);
}

function addDays(value, amount) {
  return new Date(value.getTime() + amount * DAY_MS);
}

function dateLabel(value, { year = true } = {}) {
  const parsed = dateOf(value);
  if (!parsed) return '--';
  return parsed.toLocaleDateString('en-US', {
    month: 'short', day: 'numeric', ...(year ? { year: 'numeric' } : {}), timeZone: 'UTC',
  });
}

function plainLabel(value) {
  return String(value || 'event').replace(/[-_]+/g, ' ').trim();
}

function todayUtc() {
  const now = new Date();
  return new Date(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate()));
}

function projectEnd(project, today) {
  return dateOf(project.completed_at || project.cancelled_at) || today;
}

function datedAttention(project, today) {
  return (project.attention || []).filter((item) => {
    const parsed = dateOf(item.date);
    return parsed && parsed > today;
  });
}

function collectDates(projects, showFuture, today) {
  const dates = [today];
  for (const project of projects) {
    for (const value of [project.created_at, project.completed_at, project.cancelled_at, project.last_activity]) {
      const parsed = dateOf(value);
      if (parsed) dates.push(parsed);
    }
    for (const item of project.deliverables || []) {
      const parsed = dateOf(item.last_updated);
      if (parsed) dates.push(parsed);
    }
    for (const item of project.activity || []) {
      const parsed = dateOf(item.timestamp);
      if (parsed) dates.push(parsed);
    }
    if (showFuture) {
      for (const item of datedAttention(project, today)) dates.push(dateOf(item.date));
    }
  }
  return dates;
}

function rangeFor(projects, showFuture, windowMonths) {
  const today = todayUtc();
  const dates = collectDates(projects, showFuture, today);
  let start = addDays(new Date(Math.min(...dates.map(Number))), -7);
  if (windowMonths) {
    const clip = addDays(today, -Math.round(windowMonths * 30.4));
    if (start < clip) start = clip;
  }
  let end = addDays(today, 7);
  if (showFuture) {
    const maxDate = new Date(Math.max(...dates.map(Number)));
    if (addDays(maxDate, 4) > end) end = addDays(maxDate, 4);
  }
  if (end <= start) end = addDays(start, 30);
  return { start, end, today };
}

function position(value, range) {
  const parsed = dateOf(value);
  if (!parsed) return null;
  const total = range.end - range.start;
  return Math.max(0, Math.min(100, ((parsed - range.start) / total) * 100));
}

function inRange(value, range) {
  const parsed = dateOf(value);
  return parsed && parsed >= range.start && parsed <= range.end;
}

function projectIntersects(project, range) {
  const start = dateOf(project.created_at) || dateOf(project.last_activity) || range.start;
  const end = projectEnd(project, range.today);
  return start <= range.end && end >= range.start;
}

function monthSegments(range) {
  const cursor = new Date(Date.UTC(range.start.getUTCFullYear(), range.start.getUTCMonth(), 1));
  const segments = [];
  while (cursor <= range.end) {
    const next = new Date(Date.UTC(cursor.getUTCFullYear(), cursor.getUTCMonth() + 1, 1));
    const visibleStart = cursor < range.start ? range.start : cursor;
    const visibleEnd = next > range.end ? range.end : next;
    const left = position(isoDate(visibleStart), range);
    const right = position(isoDate(visibleEnd), range);
    segments.push({
      left,
      width: Math.max(0, right - left),
      label: cursor.toLocaleDateString('en-US', { month: 'short', year: 'numeric', timeZone: 'UTC' }),
      boundary: position(isoDate(cursor), range),
    });
    cursor.setUTCMonth(cursor.getUTCMonth() + 1);
  }
  return segments;
}

function popLines(meta, body) {
  const lines = [{ text: meta }];
  if (body) lines.push({ text: body });
  return lines;
}

// Cluster deliverables by day into one ribbon-style dot per project-day.
function deliverableDots(project, range, color) {
  const byDay = {};
  for (const item of project.deliverables || []) {
    const d = day(item.last_updated);
    if (d && inRange(item.last_updated, range)) (byDay[d] = byDay[d] || []).push(item);
  }
  const dots = [];
  for (const [d, items] of Object.entries(byDay)) {
    const pos = position(d, range);
    const dot = deliverableDot(project, d, items, pos, color);
    dot.classList.add('on-lane');
    dots.push(dot);
  }
  return dots;
}

// Material-activity diamond: project-colored, embedded in the bar row.
function activityMarker(project, item, range, color) {
  const marker = el('button', {
    class: 'calendar-marker activity',
    style: `left:${position(item.timestamp, range)}%;background:${color}`,
    'aria-label': `${plainLabel(item.event)}, ${item.timestamp}`,
  });
  attachPopover(marker, () => popContent(plainLabel(item.event), popLines(dateLabel(item.timestamp), item.text)));
  if (item.file) marker.addEventListener('click', () => navigate('preview', { project: project.slug, file: item.file }));
  else marker.classList.add('no-link');
  return marker;
}

function attentionMarker(project, item, range) {
  const marker = el('button', {
    class: 'calendar-marker future',
    style: `left:${position(item.date, range)}%`,
    'aria-label': `${item.kind || 'future commitment'}, ${item.date}`,
  });
  attachPopover(marker, () => popContent(
    plainLabel(item.kind || 'future commitment'), popLines(dateLabel(item.date), item.text)));
  if (item.file) marker.addEventListener('click', () => navigate('preview', { project: project.slug, file: item.file }));
  else marker.classList.add('no-link');
  return marker;
}

const BANDS_MAX_DAYS = 400; // beyond this, per-day/weekly texture is noise

function gridRules(range) {
  const frag = document.createDocumentFragment();
  const totalDays = Math.round((range.end - range.start) / DAY_MS);
  if (totalDays <= BANDS_MAX_DAYS) {
    const everyDay = range.dayDetail;
    for (let d = 0; d < totalDays; d++) {
      const cur = addDays(range.start, d);
      const dow = cur.getUTCDay();
      if (dow === 0 || dow === 6) {
        frag.append(el('i', {
          class: 'calendar-weekend',
          style: `left:${(d / totalDays) * 100}%;width:${100 / totalDays}%`,
        }));
      }
      // day ticks when zoomed in; weekly (Monday) rhythm when zoomed out
      if (d > 0 && (everyDay || dow === 1)) {
        frag.append(el('i', { class: 'calendar-dayline', style: `left:${(d / totalDays) * 100}%` }));
      }
    }
  }
  for (const segment of monthSegments(range)) {
    if (segment.boundary > 0) frag.append(el('i', { class: 'calendar-gridline', style: `left:${segment.boundary}%` }));
  }
  const todayPos = position(isoDate(range.today), range);
  if (todayPos >= 0 && todayPos <= 100) frag.append(el('i', { class: 'calendar-today-line', style: `left:${todayPos}%` }));
  return frag;
}

// Two-layer duration bar in the shared ribbon language: flat tint ghost
// track spanning the whole project, hard solid segments on worked days.
// When ghost is off, one full solid bar.
function durationBar(project, range, ghost, color) {
  const created = dateOf(project.created_at) || dateOf(project.last_activity) || range.start;
  const rawEnd = projectEnd(project, range.today);
  const visibleStart = created < range.start ? range.start : created;
  const visibleEnd = rawEnd > range.end ? range.end : rawEnd;
  const left = position(isoDate(visibleStart), range);
  const right = position(isoDate(visibleEnd), range);
  const width = Math.max(0.5, right - left);
  const frag = document.createDocumentFragment();
  frag.append(el('span', {
    class: 'calendar-duration',
    style: `left:${left}%;width:${width}%;background:${ghost ? ghostTint(color) : color}`,
    title: `${dateLabel(project.created_at)} to ${project.status === 'active' ? 'today' : dateLabel(project.completed_at || project.cancelled_at)}`,
  }));
  if (ghost) {
    for (const d of project.worked_days || []) {
      if (!inRange(d, range)) continue;
      const segLeft = position(d, range);
      const segRight = position(isoDate(addDays(dateOf(d), 1)), range);
      frag.append(el('span', {
        class: 'calendar-worked-seg',
        style: `left:${segLeft}%;width:${Math.max(0.4, segRight - segLeft)}%;background:${color}`,
      }));
    }
  }
  return frag;
}

// Dashed continuation from the bar's end to the furthest future commitment,
// so future squares read as part of the project's lane.
function futureExtension(project, range, color) {
  let maxFuture = null;
  for (const item of datedAttention(project, range.today)) {
    const parsed = dateOf(item.date);
    if (parsed && (!maxFuture || parsed > maxFuture)) maxFuture = parsed;
  }
  if (!maxFuture) return null;
  const barEnd = projectEnd(project, range.today);
  const left = position(isoDate(barEnd), range);
  const right = position(isoDate(addDays(maxFuture, 1)), range);
  if (right <= left) return null;
  return el('span', {
    class: 'lane-ext',
    style: `left:${left}%;width:${right - left}%;border-color:${color}`,
  });
}

function projectRow(project, range, opts) {
  const color = colorFor(project.slug);
  const deliverables = project.deliverables || [];
  const summary = project.previewable === false
    ? `${plainLabel(project.domain)} · ${(project.attention || []).length} commitments`
    : `${plainLabel(project.status)} · ${dateLabel(project.created_at, { year: false })} → ${project.status === 'active' ? 'now' : dateLabel(project.completed_at || project.cancelled_at, { year: false })} · ${deliverables.length} deliverables`;
  const label = el(project.previewable === false ? 'div' : 'button', { class: 'calendar-project-label' },
    keycapEl(project.slug, project.name),
    el('span', { class: 'calendar-project-copy' },
      el('strong', { text: project.name || project.slug, title: project.name || project.slug }),
      el('span', {
        text: summary,
      })));
  if (project.previewable !== false) {
    label.addEventListener('click', () => navigate('preview', { project: project.slug }));
  }

  const track = el('div', { class: 'calendar-track' });
  track.append(gridRules(range));
  track.append(durationBar(project, range, opts.ghost, color));
  for (const dot of deliverableDots(project, range, color)) track.append(dot);
  for (const item of project.activity || []) {
    if (inRange(item.timestamp, range)) track.append(activityMarker(project, item, range, color));
  }
  if (opts.showFuture) {
    const ext = futureExtension(project, range, color);
    if (ext) track.append(ext);
    for (const item of datedAttention(project, range.today)) {
      if (inRange(item.date, range)) track.append(attentionMarker(project, item, range));
    }
  }
  return el('div', { class: 'calendar-row' }, label, track);
}

function renderAxis(range) {
  const months = el('div', { class: 'calendar-months' });
  for (const segment of monthSegments(range)) {
    months.append(el('span', {
      style: `left:${segment.left}%;width:${segment.width}%`,
      text: segment.label,
    }));
  }
  months.append(el('i', {
    class: 'calendar-today-axis',
    style: `left:${position(isoDate(range.today), range)}%`,
    text: 'today',
  }));
  const right = el('div', { class: 'calendar-axis-right' }, months);
  const totalDays = Math.round((range.end - range.start) / DAY_MS);
  if (totalDays <= BANDS_MAX_DAYS) {
    // dates at every zoom: per-day numbers whenever the pixels fit,
    // Monday dates only when they genuinely don't
    const everyDay = range.dayDetail;
    const days = el('div', { class: 'calendar-days' });
    for (let d = 0; d < totalDays; d++) {
      const cur = addDays(range.start, d);
      const dow = cur.getUTCDay();
      if (everyDay) {
        days.append(el('span', {
          class: dow === 0 || dow === 6 ? 'wknd' : '',
          style: `left:${(d / totalDays) * 100}%;width:${100 / totalDays}%`,
          text: String(cur.getUTCDate()),
        }));
      } else if (dow === 1) {
        days.append(el('span', {
          class: 'wkstart',
          style: `left:${(d / totalDays) * 100}%;width:${(7 / totalDays) * 100}%`,
          text: String(cur.getUTCDate()),
        }));
      }
    }
    right.append(days);
  }
  return el('div', { class: 'calendar-axis' },
    el('div', { class: 'calendar-axis-label', text: `${dateLabel(isoDate(range.start))} — ${dateLabel(isoDate(range.end))}` }),
    right);
}

function renderSummary(projects, range) {
  const summary = document.getElementById('calendar-summary');
  if (!summary) return;
  const deliverables = projects.reduce((sum, project) => sum + (project.deliverables || []).length, 0);
  const completed = projects.filter((project) => project.status === 'complete').length;
  const active = projects.filter((project) => project.status === 'active').length;
  const values = [
    [projects.length, 'visible projects'],
    [active, 'active now'],
    [completed, 'completed'],
    [deliverables, 'deliverables tracked'],
    [`${dateLabel(isoDate(range.start), { year: false })}–${dateLabel(isoDate(range.end), { year: false })}`, 'visible window'],
  ];
  summary.replaceChildren(...values.map(([value, label]) =>
    el('div', {}, el('strong', { text: String(value) }), el('span', { text: label }))));
}

export function renderTimeline(board, projects, opts) {
  const range = rangeFor(projects.length ? projects : [], opts.showFuture, opts.windowMonths);
  const totalDays = Math.max(1, Math.round((range.end - range.start) / DAY_MS));
  const trackWidth = Math.max(0, board.clientWidth - LABEL_COL_PX);
  range.dayDetail = trackWidth / totalDays >= MIN_PX_PER_DAY;
  const visible = projects.filter((project) => projectIntersects(project, range));
  renderSummary(visible, range);
  const scrollTop = board.scrollTop;
  board.replaceChildren(renderAxis(range));
  if (!visible.length) {
    board.append(el('div', { class: 'calendar-empty', text: projects.length ? 'No selected projects overlap this window.' : 'Select at least one project.' }));
  } else {
    for (const project of visible) board.append(projectRow(project, range, opts));
  }
  board.scrollTop = scrollTop;
}
