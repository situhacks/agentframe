// Calendar tab: view switcher (Day/Week/Month via FullCalendar, Timeline via
// timeline.js), URL-persisted state, filters, and legend. Derived data
// (worked_days, work_blocks, active-first sort) comes from the snapshot API.

import { el, keycapEl } from './api.js';
import { navigate, parseHash } from './app.js';
import { renderTimeline } from './timeline.js';

const VIEWS = new Set(['timeGridDay', 'timeGridWeek', 'dayGridMonth', 'timeline']);
const DAY_MS = 86400000;

const calendarState = {
  snapshot: null,
  selected: new Set(),
  known: new Set(),
  view: 'timeGridWeek',
  ghost: true,
  ribbons: true,
  expandLabels: false,
  showFuture: true,
  focusDate: null,   // 'YYYY-MM-DD' anchor for FullCalendar
  fc: null,          // FullCalendar instance (lazy)
};

function plainLabel(value) {
  return String(value || 'event').replace(/[-_]+/g, ' ').trim();
}

function readCalendarHash() {
  const { params } = parseHash();
  const view = params.get('view');
  if (view && VIEWS.has(view)) calendarState.view = view;
  if (params.get('ghost') === '0') calendarState.ghost = false;
  if (params.get('expand') === '1') calendarState.expandLabels = true;
  if (params.get('future') === '0') calendarState.showFuture = false;
  const date = params.get('date');
  if (date && /^\d{4}-\d{2}-\d{2}$/.test(date)) calendarState.focusDate = date;
}

function writeCalendarHash() {
  const params = {};
  params.view = calendarState.view;
  if (!calendarState.ghost) params.ghost = '0';
  if (calendarState.expandLabels) params.expand = '1';
  if (!calendarState.showFuture) params.future = '0';
  if (calendarState.focusDate) params.date = calendarState.focusDate;
  navigate('calendar', params);
}

function setActiveViewButton() {
  for (const btn of document.querySelectorAll('#calendar-views button')) {
    btn.classList.toggle('on', btn.dataset.view === calendarState.view);
  }
}

export function setupCalendar() {
  readCalendarHash();
  setActiveViewButton();
  syncToggleInputs();
  for (const btn of document.querySelectorAll('#calendar-views button')) {
    btn.addEventListener('click', () => {
      calendarState.view = btn.dataset.view;
      setActiveViewButton();
      writeCalendarHash();
      if (calendarState.snapshot) renderCalendar(calendarState.snapshot);
    });
  }
  const bind = (id, key) =>
    document.getElementById(id)?.addEventListener('change', (e) => {
      calendarState[key] = e.target.checked;
      writeCalendarHash();
      if (calendarState.snapshot) renderCalendar(calendarState.snapshot);
    });
  bind('calendar-ghost', 'ghost');
  bind('calendar-ribbons', 'ribbons');
  bind('calendar-expand', 'expandLabels');
  bind('calendar-future', 'showFuture');
  document.getElementById('calendar-all')?.addEventListener('click', () => {
    calendarState.selected = new Set(calendarState.known);
    if (calendarState.snapshot) renderCalendar(calendarState.snapshot);
  });
  document.getElementById('calendar-none')?.addEventListener('click', () => {
    calendarState.selected.clear();
    if (calendarState.snapshot) renderCalendar(calendarState.snapshot);
  });
  document.getElementById('calendar-active')?.addEventListener('click', () => {
    const active = (calendarState.snapshot?.timeline_projects || [])
      .filter((p) => p.status === 'active').map((p) => p.slug);
    calendarState.selected = new Set(active);
    if (calendarState.snapshot) renderCalendar(calendarState.snapshot);
  });
  document.getElementById('calendar-print')?.addEventListener('click', () => window.print());
  renderLegend();
}

function syncToggleInputs() {
  const set = (id, checked) => {
    const input = document.getElementById(id);
    if (input) input.checked = checked;
  };
  set('calendar-ghost', calendarState.ghost);
  set('calendar-ribbons', calendarState.ribbons);
  set('calendar-expand', calendarState.expandLabels);
  set('calendar-future', calendarState.showFuture);
}

function renderLegend() {
  const host = document.getElementById('calendar-legend');
  if (!host) return;
  host.replaceChildren(
    el('span', {}, el('i', { class: 'legend-ghost' }), 'active, no logged work'),
    el('span', {}, el('i', { class: 'legend-solid' }), 'worked that day'),
    el('span', {}, el('i', { class: 'legend-dot' }), 'deliverable (hover)'),
    el('span', {}, el('i', { class: 'legend-block' }), 'synthesized work block'),
    el('span', {}, el('i', { class: 'legend-square' }), 'future commitment'),
  );
}

function syncSelections(projects) {
  const slugs = new Set(projects.map((p) => p.slug));
  for (const slug of slugs) if (!calendarState.known.has(slug)) calendarState.selected.add(slug);
  for (const slug of [...calendarState.selected]) if (!slugs.has(slug)) calendarState.selected.delete(slug);
  calendarState.known = slugs;
}

function renderFilters(projects) {
  const host = document.getElementById('calendar-project-filters');
  const scrollTop = host.scrollTop;
  host.replaceChildren();
  for (const project of projects) {
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.checked = calendarState.selected.has(project.slug);
    input.addEventListener('change', () => {
      if (input.checked) calendarState.selected.add(project.slug);
      else calendarState.selected.delete(project.slug);
      renderCalendar(calendarState.snapshot);
    });
    host.append(el('label', { class: 'calendar-filter-row' },
      input,
      keycapEl(project.slug, project.name),
      el('span', { class: 'grow', text: project.name || project.slug, title: project.name || project.slug }),
      el('span', { class: `calendar-filter-status ${project.status}`, text: plainLabel(project.status) })));
  }
  host.scrollTop = scrollTop;
}

// ---- FullCalendar ----

const FC_SRC = 'https://cdn.jsdelivr.net/npm/fullcalendar@6.1.21/index.global.min.js';
let fcLoad = null;

function ensureFullCalendar() {
  if (window.FullCalendar) return Promise.resolve(window.FullCalendar);
  if (!fcLoad) {
    fcLoad = new Promise((resolve, reject) => {
      const s = document.createElement('script');
      s.src = FC_SRC;
      s.onload = () => resolve(window.FullCalendar);
      s.onerror = () => reject(new Error('FullCalendar failed to load from CDN'));
      document.head.append(s);
    });
  }
  return fcLoad;
}

function colorFor(slug) {
  // deterministic hue from slug; matches keycap spread
  let hash = 0;
  for (const ch of String(slug)) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 45% 45%)`;
}

function endExclusive(dateStr) {
  const d = new Date(`${dateStr}T00:00:00Z`);
  return new Date(d.getTime() + DAY_MS).toISOString().slice(0, 10);
}

function pad(minutes) {
  const h = String(Math.floor(minutes / 60)).padStart(2, '0');
  const m = String(minutes % 60).padStart(2, '0');
  return `${h}:${m}:00`;
}

function projectEvents(project) {
  const color = colorFor(project.slug);
  const events = [];
  const start = String(project.created_at || project.last_activity || '').slice(0, 10);
  const rawEnd = String(project.completed_at || project.cancelled_at || '').slice(0, 10);
  const today = new Date().toISOString().slice(0, 10);
  const end = rawEnd || today;
  if (calendarState.ribbons && start) {
    events.push({
      start, end: endExclusive(end), allDay: true,
      title: project.name || project.slug,
      classNames: ['af-span', calendarState.ghost ? 'ghost' : 'solid'],
      extendedProps: { project: project.slug, color, worked: project.worked_days || [], kind: 'span' },
    });
  }
  for (const block of project.work_blocks || []) {
    events.push({
      start: `${block.date}T${pad(block.start)}`,
      end: `${block.date}T${pad(block.end)}`,
      title: project.name || project.slug,
      backgroundColor: color, borderColor: color,
      extendedProps: { project: project.slug, color, kind: 'block', events: block.events },
    });
  }
  const delByDay = {};
  for (const item of project.deliverables || []) {
    const day = String(item.last_updated || '').slice(0, 10);
    if (day.length === 10) (delByDay[day] = delByDay[day] || []).push(item);
  }
  for (const [day, items] of Object.entries(delByDay)) {
    events.push({
      start: day, allDay: true, title: '',
      classNames: ['af-deliverable'],
      extendedProps: { project: project.slug, color, kind: 'deliverable', items },
    });
  }
  if (calendarState.showFuture) {
    for (const item of project.attention || []) {
      const day = String(item.date || '').slice(0, 10);
      if (day.length === 10 && day > today) {
        events.push({
          start: day, allDay: true, title: '',
          classNames: ['af-future'],
          extendedProps: { project: project.slug, color, kind: 'future', item },
        });
      }
    }
  }
  return events;
}

function minutesLabel(minutes) {
  const h = Math.floor(minutes / 60), m = String(minutes % 60).padStart(2, '0');
  const ampm = h >= 12 ? 'PM' : 'AM';
  const h12 = ((h + 11) % 12) + 1;
  return `${h12}:${m} ${ampm}`;
}

function popoverEl(title, lines) {
  const pop = el('span', { class: 'calendar-popover' }, el('strong', { text: title }));
  for (const line of lines) {
    const row = el('span', { class: 'pop-line', text: line.text });
    if (line.onClick) { row.classList.add('pop-link'); row.addEventListener('click', line.onClick); }
    pop.append(row);
  }
  return pop;
}

function toPreview(project, file) {
  if (file) navigate('preview', { project, file });
}

function eventContent(arg) {
  const p = arg.event.extendedProps;
  if (p.kind === 'block') {
    const wrap = el('div', { class: 'af-block-inner' },
      el('div', { class: 'af-block-title', text: `${arg.event.title} · ${p.events.length}` }));
    const col = el('div', { class: 'af-block-dots' });
    for (const ev of p.events) {
      const dot = el('span', { class: 'af-bdot' },
        popoverEl(plainLabel(ev.label), [{ text: minutesLabel(ev.time), onClick: ev.file ? () => toPreview(p.project, ev.file) : null }]));
      col.append(dot);
    }
    wrap.append(col);
    return { domNodes: [wrap] };
  }
  if (p.kind === 'deliverable') {
    if (calendarState.expandLabels) {
      const chips = el('div', { class: 'af-chips' });
      for (const item of p.items) {
        const chip = el('span', { class: 'af-chip', style: `--pc:${p.color}`, text: plainLabel(item.slug) });
        if (item.file) { chip.classList.add('linked'); chip.addEventListener('click', () => toPreview(p.project, item.file)); }
        chips.append(chip);
      }
      return { domNodes: [chips] };
    }
    const dot = el('span', { class: 'af-dot', style: `border-color:${p.color}`, text: p.items.length > 1 ? String(p.items.length) : '' });
    dot.append(popoverEl(p.items.length > 1 ? `${p.items.length} deliverables` : plainLabel(p.items[0].slug),
      p.items.map((item) => ({ text: `● ${plainLabel(item.slug)}`, onClick: item.file ? () => toPreview(p.project, item.file) : null }))));
    return { domNodes: [dot] };
  }
  if (p.kind === 'future') {
    const sq = el('span', { class: 'af-future-marker' },
      popoverEl(plainLabel(p.item.kind || 'future commitment'), [{ text: String(p.item.text || '') }]));
    return { domNodes: [sq] };
  }
  return true;
}

function paintWorkedSegments(elm, event, worked) {
  const color = elm.style.getPropertyValue('--pc') || 'currentColor';
  if (!calendarState.ghost) {
    // solid fill across the whole span
    elm.style.background = color;
    return;
  }
  // ghost mode: faint base band, solid segments on days with logged work
  const startMs = event.start.getTime();
  const endMs = event.end.getTime();
  const span = Math.max(1, endMs - startMs);
  const stops = [];
  for (const day of worked || []) {
    const d0 = new Date(`${day}T00:00:00Z`).getTime();
    if (d0 < startMs || d0 >= endMs) continue;
    const from = ((d0 - startMs) / span) * 100;
    const to = ((d0 + DAY_MS - startMs) / span) * 100;
    stops.push(`${color} ${from}%`, `${color} ${to}%`, `transparent ${to}%`);
  }
  // transparent gradient (solid worked segments) over a faint tint base
  elm.style.background = stops.length
    ? `linear-gradient(90deg, ${['transparent 0%', ...stops].join(', ')})`
    : 'transparent';
  elm.classList.add('ghost-band');
}

function eventDidMount(info) {
  const p = info.event.extendedProps;
  if (p.kind === 'span') {
    info.el.style.setProperty('--pc', p.color);
    paintWorkedSegments(info.el, info.event, p.worked);
  } else if (p.kind === 'block') {
    info.el.style.setProperty('--pc', p.color);
  }
}

async function renderFullCalendar(projects) {
  const host = document.getElementById('calendar-fc');
  let FC;
  try {
    FC = await ensureFullCalendar();
  } catch (err) {
    host.innerHTML = `<div class="viewer-note"><b>Calendar failed to load.</b><br>` +
      `FullCalendar loads from cdn.jsdelivr.net — check the network connection and refresh.<br>` +
      `<span class="warn">${String(err.message || err)}</span></div>`;
    return;
  }
  const events = projects.flatMap(projectEvents);
  if (!calendarState.fc) {
    calendarState.fc = new FC.Calendar(host, {
      initialView: calendarState.view === 'timeline' ? 'timeGridWeek' : calendarState.view,
      initialDate: calendarState.focusDate || undefined,
      headerToolbar: { left: 'prev,next today', center: 'title', right: '' },
      allDaySlot: true,
      nowIndicator: true,
      height: 'auto',
      eventContent,
      eventDidMount,
      datesSet: (info) => {
        calendarState.focusDate = info.startStr.slice(0, 10);
        writeCalendarHash();
      },
    });
    calendarState.fc.render();
  }
  const fc = calendarState.fc;
  fc.changeView(calendarState.view === 'timeline' ? 'timeGridWeek' : calendarState.view);
  fc.removeAllEvents();
  for (const ev of events) fc.addEvent(ev);
}

// ---- Timeline (Task 7) ----

function renderTimelineView(projects) {
  const board = document.getElementById('calendar-board');
  renderTimeline(board, projects, {
    ghost: calendarState.ghost,
    expandLabels: calendarState.expandLabels,
    showFuture: calendarState.showFuture,
  });
}

export { calendarState };

export function renderCalendar(snapshot) {
  calendarState.snapshot = snapshot;
  const allProjects = snapshot.timeline_projects || [];
  syncSelections(allProjects);
  renderFilters(allProjects);
  const selected = allProjects.filter((p) => calendarState.selected.has(p.slug));
  const fcEl = document.getElementById('calendar-fc');
  const boardEl = document.getElementById('calendar-board');
  const isTimeline = calendarState.view === 'timeline';
  fcEl.hidden = isTimeline;
  boardEl.hidden = !isTimeline;
  if (isTimeline) renderTimelineView(selected);
  else renderFullCalendar(selected);
}
