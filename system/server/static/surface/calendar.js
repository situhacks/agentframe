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

// ---- FullCalendar (Task 5) + dots/popovers (Task 6) ----

function renderFullCalendar(_projects) {}

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
