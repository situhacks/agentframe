// Calendar tab: four views sharing one design language.
//   Day/Week  — FullCalendar renders ONLY the hour grid (work blocks); the
//               project ribbons are our own header strip above it.
//   Month     — fully custom: mockup-style week rows (date strip + ribbons).
//   Timeline  — swimlane in timeline.js.
// Derived data (worked_days, work_blocks, active-first sort) comes from the
// snapshot API. View, focus date, and toggles persist in the URL hash.

import { el, keycapEl } from './api.js';
import { navigate, parseHash } from './app.js';
import { renderTimeline } from './timeline.js';
import { renderRibbons, colorFor, attachPopover, popContent, plainLabel, todayIso, DAY_MS } from './ribbons.js';

const VIEWS = new Set(['timeGridDay', 'timeGridWeek', 'dayGridMonth', 'timeline']);

const calendarState = {
  snapshot: null,
  selected: new Set(),
  known: new Set(),
  view: 'timeGridWeek',
  ghost: true,
  ribbons: true,
  expandLabels: false,
  showFuture: true,
  window: 'all',     // timeline lookback: all | 12 | 6 | 3 | 1 (months)
  focusDate: null,   // 'YYYY-MM-DD' anchor
  fc: null,          // FullCalendar instance (lazy)
};

function opts() {
  return {
    ghost: calendarState.ghost,
    expandLabels: calendarState.expandLabels,
    showFuture: calendarState.showFuture,
  };
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
  const params = { view: calendarState.view };
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

function rerender() {
  if (calendarState.snapshot) renderCalendar(calendarState.snapshot);
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
      rerender();
    });
  }
  const bind = (id, key) =>
    document.getElementById(id)?.addEventListener('change', (e) => {
      calendarState[key] = e.target.checked;
      writeCalendarHash();
      rerender();
    });
  bind('calendar-ghost', 'ghost');
  bind('calendar-ribbons', 'ribbons');
  bind('calendar-expand', 'expandLabels');
  bind('calendar-future', 'showFuture');
  document.getElementById('calendar-all')?.addEventListener('click', () => {
    calendarState.selected = new Set(calendarState.known);
    rerender();
  });
  document.getElementById('calendar-none')?.addEventListener('click', () => {
    calendarState.selected.clear();
    rerender();
  });
  document.getElementById('calendar-active')?.addEventListener('click', () => {
    const active = (calendarState.snapshot?.timeline_projects || [])
      .filter((p) => p.status === 'active').map((p) => p.slug);
    calendarState.selected = new Set(active);
    rerender();
  });
  document.getElementById('calendar-prev')?.addEventListener('click', () => navByStep(-1));
  document.getElementById('calendar-next')?.addEventListener('click', () => navByStep(1));
  document.getElementById('calendar-window')?.addEventListener('change', (e) => {
    calendarState.window = e.target.value;
    rerender();
  });
  document.getElementById('calendar-today-btn')?.addEventListener('click', () => {
    calendarState.focusDate = todayIso();
    if (calendarState.fc && calendarState.view.startsWith('timeGrid')) calendarState.fc.today();
    writeCalendarHash();
    rerender();
  });
  document.getElementById('calendar-print')?.addEventListener('click', () => window.print());
  renderLegend();
}

function navByStep(step) {
  if (calendarState.view === 'dayGridMonth') {
    const base = calendarState.focusDate ? new Date(`${calendarState.focusDate}T00:00:00Z`) : new Date(`${todayIso()}T00:00:00Z`);
    const moved = new Date(Date.UTC(base.getUTCFullYear(), base.getUTCMonth() + step, 1));
    calendarState.focusDate = moved.toISOString().slice(0, 10);
    writeCalendarHash();
    rerender();
  } else if (calendarState.fc) {
    if (step > 0) calendarState.fc.next(); else calendarState.fc.prev();
    // datesSet callback updates focusDate, title, ribbons, hash
  }
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
      rerender();
    });
    host.append(el('label', { class: 'calendar-filter-row' },
      input,
      keycapEl(project.slug, project.name),
      el('span', { class: 'grow', text: project.name || project.slug, title: project.name || project.slug }),
      el('span', { class: `calendar-filter-status ${project.status}`, text: plainLabel(project.status) })));
  }
  host.scrollTop = scrollTop;
}

// ---- FullCalendar: hour grid only ----

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

function pad(minutes) {
  const h = String(Math.floor(minutes / 60)).padStart(2, '0');
  const m = String(minutes % 60).padStart(2, '0');
  return `${h}:${m}:00`;
}

function minutesLabel(minutes) {
  const h = Math.floor(minutes / 60), m = String(minutes % 60).padStart(2, '0');
  const ampm = h >= 12 ? 'PM' : 'AM';
  const h12 = ((h + 11) % 12) + 1;
  return `${h12}:${m} ${ampm}`;
}

function blockEvents(project) {
  const color = colorFor(project.slug);
  return (project.work_blocks || []).map((block) => ({
    start: `${block.date}T${pad(block.start)}`,
    end: `${block.date}T${pad(block.end)}`,
    title: project.name || project.slug,
    backgroundColor: color, borderColor: color,
    extendedProps: { project: project.slug, color, kind: 'block', events: block.events },
  }));
}

function toPreview(project, file) {
  if (file) navigate('preview', { project, file });
}

function eventContent(arg) {
  const p = arg.event.extendedProps;
  if (p.kind !== 'block') return true;
  const wrap = el('div', { class: 'af-block-inner' },
    el('div', { class: 'af-block-title' },
      keycapEl(p.project, arg.event.title),
      el('span', { text: `${arg.event.title} · ${p.events.length}` })));
  const col = el('div', { class: 'af-block-dots' });
  for (const ev of p.events) {
    const bdot = el('span', { class: 'af-bdot' });
    attachPopover(bdot, () => popContent(plainLabel(ev.label), [{
      text: minutesLabel(ev.time),
      onClick: ev.file ? () => toPreview(p.project, ev.file) : null,
    }]));
    col.append(bdot);
  }
  wrap.append(col);
  return { domNodes: [wrap] };
}

function fcWindow() {
  const view = calendarState.fc.view;
  return {
    start: view.activeStart.toISOString().slice(0, 10),
    end: view.activeEnd.toISOString().slice(0, 10),
    title: view.title,
  };
}

function renderRibbonHead(projects) {
  const head = document.getElementById('calendar-ribbons-head');
  const rows = document.getElementById('calendar-ribbons-rows');
  if (!calendarState.ribbons || !calendarState.fc) {
    head.hidden = true;
    return;
  }
  head.hidden = false;
  const win = fcWindow();
  renderRibbons(rows, projects, win.start, win.end, opts());
  // align the rows with FullCalendar's time-axis gutter and scrollbar pad
  const fcHost = document.getElementById('calendar-fc');
  const axis = fcHost.querySelector('.fc-timegrid-axis');
  const scroller = fcHost.querySelector('.fc-scroller-liquid-absolute') || fcHost.querySelector('.fc-scroller');
  const gutter = axis ? Math.round(axis.getBoundingClientRect().width) : 58;
  const sbw = scroller ? scroller.offsetWidth - scroller.clientWidth : 0;
  head.style.setProperty('--rib-gutter', `${gutter}px`);
  head.style.setProperty('--rib-scrollpad', `${sbw}px`);
}

async function renderTimeGrid(projects) {
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
  if (!calendarState.fc) {
    calendarState.fc = new FC.Calendar(host, {
      initialView: calendarState.view,
      initialDate: calendarState.focusDate || undefined,
      headerToolbar: false,
      allDaySlot: false,
      nowIndicator: true,
      height: '100%',
      expandRows: false,
      scrollTime: '08:00:00',
      scrollTimeReset: false,
      businessHours: { daysOfWeek: [0, 1, 2, 3, 4, 5, 6], startTime: '08:00', endTime: '17:00' },
      dayHeaderFormat: { weekday: 'short', day: 'numeric' },
      eventContent,
      eventDidMount: (info) => {
        if (info.event.extendedProps.kind === 'block') {
          info.el.style.setProperty('--pc', info.event.extendedProps.color);
        }
      },
      datesSet: () => {
        const win = fcWindow();
        calendarState.focusDate = win.start;
        document.getElementById('calendar-title').textContent = win.title;
        renderRibbonHead(currentSelection());
        writeCalendarHash();
      },
    });
    calendarState.fc.render();
  }
  const fc = calendarState.fc;
  if (fc.view.type !== calendarState.view) {
    fc.changeView(calendarState.view);
    fc.scrollToTime('08:00:00');
  }
  fc.removeAllEvents();
  for (const ev of projects.flatMap(blockEvents)) fc.addEvent(ev);
  fc.updateSize();
  if (host.dataset.justShown) {
    fc.scrollToTime('08:00:00');
    delete host.dataset.justShown;
  }
  document.getElementById('calendar-title').textContent = fc.view.title;
  renderRibbonHead(projects);
}

// ---- Custom month view: mockup week rows (date strip + ribbons) ----

function monthTitle(anchor) {
  return anchor.toLocaleDateString('en-US', { month: 'long', year: 'numeric', timeZone: 'UTC' });
}

function renderMonth(projects) {
  const host = document.getElementById('calendar-month');
  const anchorStr = calendarState.focusDate || todayIso();
  const anchor = new Date(`${anchorStr.slice(0, 7)}-01T00:00:00Z`);
  document.getElementById('calendar-title').textContent = monthTitle(anchor);

  const monthIdx = anchor.getUTCMonth();
  // back up to the Sunday on/before the 1st
  const first = new Date(anchor.getTime() - anchor.getUTCDay() * DAY_MS);
  const today = todayIso();

  host.replaceChildren();
  const dow = el('div', { class: 'cal-month-dow' });
  for (const d of ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']) {
    dow.append(el('span', { text: d }));
  }
  host.append(dow);

  for (let w = 0; ; w++) {
    const weekStart = new Date(first.getTime() + w * 7 * DAY_MS);
    if (w > 0 && weekStart.getUTCMonth() !== monthIdx && weekStart > anchor) break;
    if (w > 5) break;
    const weekEnd = new Date(weekStart.getTime() + 7 * DAY_MS);
    const row = el('div', { class: 'cal-month-week' });
    const dates = el('div', { class: 'cal-month-dates' });
    let todayIdx = -1;
    for (let d = 0; d < 7; d++) {
      const cell = new Date(weekStart.getTime() + d * DAY_MS);
      const iso = cell.toISOString().slice(0, 10);
      if (iso === today) todayIdx = d;
      const num = el('span', { text: String(cell.getUTCDate()) });
      const cellEl = el('div', {
        class: `cal-month-date${cell.getUTCMonth() !== monthIdx ? ' other' : ''}${iso === today ? ' today' : ''}`,
      }, num);
      dates.append(cellEl);
    }
    // Outlook-style fill on the whole today column, behind content
    if (todayIdx >= 0) {
      row.append(el('div', {
        class: 'cal-month-today-wash',
        style: `left:${(todayIdx / 7) * 100}%;width:${100 / 7}%`,
      }));
    }
    row.append(dates);
    const ribbons = el('div', { class: 'cal-month-ribbons' });
    renderRibbons(ribbons, projects, weekStart.toISOString().slice(0, 10), weekEnd.toISOString().slice(0, 10), opts());
    row.append(ribbons);
    host.append(row);
  }
}

// ---- Timeline ----

function renderTimelineView(projects) {
  const months = calendarState.window;
  document.getElementById('calendar-title').textContent =
    months === 'all' ? 'All history' : `Last ${months} month${months === '1' ? '' : 's'}`;
  renderTimeline(document.getElementById('calendar-board'), projects,
    { ...opts(), windowMonths: months === 'all' ? null : Number(months) });
}

function currentSelection() {
  const all = calendarState.snapshot?.timeline_projects || [];
  return all.filter((p) => calendarState.selected.has(p.slug));
}

export { calendarState };

export function renderCalendar(snapshot) {
  calendarState.snapshot = snapshot;
  const allProjects = snapshot.timeline_projects || [];
  syncSelections(allProjects);
  renderFilters(allProjects);
  const selected = currentSelection();

  const view = calendarState.view;
  const isGrid = view.startsWith('timeGrid');
  const isMonth = view === 'dayGridMonth';
  const isTimeline = view === 'timeline';
  const fcEl = document.getElementById('calendar-fc');
  if (isGrid && fcEl.hidden) fcEl.dataset.justShown = '1';
  fcEl.hidden = !isGrid;
  document.getElementById('calendar-ribbons-head').hidden = !isGrid || !calendarState.ribbons;
  document.getElementById('calendar-month').hidden = !isMonth;
  document.getElementById('calendar-board').hidden = !isTimeline;
  // nav stays for timeline (window selector); prev/next/today are grid+month only
  for (const id of ['calendar-prev', 'calendar-next', 'calendar-today-btn']) {
    document.getElementById(id).hidden = isTimeline;
  }
  document.getElementById('calendar-window').hidden = !isTimeline;

  if (isTimeline) renderTimelineView(selected);
  else if (isMonth) renderMonth(selected);
  else renderTimeGrid(selected);
}
