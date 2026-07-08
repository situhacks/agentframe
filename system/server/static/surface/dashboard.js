// Dashboard: Attention, All Active Projects, Recent Activity.
// Each region scrolls internally; the page never scrolls. Activity pages
// lazily via cursor and never yanks the operator's scroll position.

import { getJSON, keycapEl, basename, el } from './api.js';
import { navigate } from './app.js';

const ZOOM_KEY = 'af-dashboard-zoom-v1';
const DEFAULT_ZOOM = 90;
const MIN_ZOOM = 60;
const MAX_ZOOM = 125;
const ZOOM_STEP = 5;

const activityState = {
  items: [],
  nextCursor: null,
  loading: false,
  etag: null,
};

function previewAction(project, file, label = 'preview ->') {
  return el('button', {
    class: 'pv-act',
    text: label,
    onclick: (e) => {
      e.stopPropagation();
      navigate('preview', { project, file });
    },
  });
}

function projCell(slug, name) {
  return el('div', { class: 'proj-cell' }, keycapEl(slug, name), el('span', { class: 'pname', text: name || slug }));
}

function flowLabel(flow) {
  if (!flow) return '--';
  return String(flow).replace(/^marketing-/, '').replace(/-flow$/, '');
}

function clampZoom(value) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return DEFAULT_ZOOM;
  return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, parsed));
}

function applyZoom(value, { persist = true } = {}) {
  const zoom = clampZoom(value);
  const dashboard = document.getElementById('view-dashboard');
  dashboard.style.setProperty('--dash-zoom', String(zoom / 100));
  const input = document.getElementById('dash-zoom-input');
  if (input) input.value = String(zoom);
  if (persist) {
    try {
      localStorage.setItem(ZOOM_KEY, String(zoom));
    } catch { /* persistence is best-effort */ }
  }
  return zoom;
}

function fitZoom() {
  const dashboard = document.getElementById('view-dashboard');
  let best = MIN_ZOOM;
  for (let zoom = MAX_ZOOM; zoom >= MIN_ZOOM; zoom -= ZOOM_STEP) {
    applyZoom(zoom, { persist: false });
    const overflows = [...dashboard.querySelectorAll('.scroll')]
      .some((node) => node.scrollWidth > node.clientWidth + 1);
    if (!overflows) {
      best = zoom;
      break;
    }
  }
  applyZoom(best);
}

export function setupDashboardDensity() {
  let saved = DEFAULT_ZOOM;
  try {
    saved = localStorage.getItem(ZOOM_KEY) || DEFAULT_ZOOM;
  } catch { /* leave default */ }
  applyZoom(saved);
  document.getElementById('dash-zoom-out')?.addEventListener('click', () => {
    applyZoom(clampZoom(document.getElementById('dash-zoom-input')?.value) - ZOOM_STEP);
  });
  document.getElementById('dash-zoom-in')?.addEventListener('click', () => {
    applyZoom(clampZoom(document.getElementById('dash-zoom-input')?.value) + ZOOM_STEP);
  });
  document.getElementById('dash-zoom-input')?.addEventListener('change', (e) => applyZoom(e.target.value));
  document.getElementById('dash-zoom-fit')?.addEventListener('click', fitZoom);
}

function activityCountText() {
  const suffix = activityState.nextCursor === null ? '' : '; loads more on scroll';
  return `showing ${activityState.items.length}${suffix}`;
}

// ---------- attention ----------

function renderAttention(snap) {
  const region = document.getElementById('region-attention');
  const body = document.getElementById('attention-body');
  const scrollTop = body.scrollTop;
  const items = snap.attention;

  document.getElementById('attention-count').textContent =
    items.length ? `${items.length} open` : '';
  region.classList.toggle('live', items.length > 0);
  document.getElementById('view-dashboard').classList.toggle('attention-empty', !items.length);

  const table = el('table', { class: 'grid attention-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'Project' }), el('th', { text: 'When' }), el('th', { text: 'Kind' }),
    el('th', { text: 'Reminder' }))));
  const tbody = el('tbody');

  if (!items.length) {
    tbody.append(el('tr', { class: 'empty-row' }, el('td', { colspan: '4', text: 'no open attention items' })));
  }
  for (const item of items) {
    const reminderText = item.text.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1');
    const row = el('tr', {},
      el('td', { class: 'nowrap' }, projCell(item.project, item.project_name)),
      el('td', { class: 'mono nowrap', text: item.date || '--' }),
      el('td', {}, el('span', { class: 'state attention', text: item.kind ? `[${item.kind}]` : '' })),
      el('td', {},
        item.file
          ? el('span', { class: 'excerpt file-link', text: reminderText, title: item.file, onclick: () => navigate('preview', { project: item.project, file: item.file }) })
          : el('span', { class: 'excerpt', text: reminderText }),
        item.file ? previewAction(item.project, item.file) : null));
    if (!item.file) {
      row.addEventListener('click', () => row.classList.toggle('expanded'));
    }
    tbody.append(row);
  }
  table.append(tbody);
  body.replaceChildren(table);
  body.scrollTop = scrollTop;
}

// ---------- active projects ----------

function renderProjects(snap) {
  const body = document.getElementById('projects-body');
  const scrollTop = body.scrollTop;

  const table = el('table', { class: 'grid projects-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'Project' }), el('th', { text: 'Flow' }),
    el('th', { text: 'Phase' }), el('th', { text: 'Updated' }),
    el('th', { text: 'Attn' }),
    el('th', { text: 'Latest' }),
    el('th', { text: 'Visibility' }))));
  const tbody = el('tbody');

  if (!snap.projects.length) {
    tbody.append(el('tr', { class: 'empty-row' }, el('td', { colspan: '7', text: 'no active projects' })));
  }
  for (const p of snap.projects) {
    const latest = p.latest_deliverable;
    tbody.append(el('tr', {},
      el('td', { class: 'nowrap' }, projCell(p.slug, p.name)),
      el('td', { class: 'mono nowrap', text: flowLabel(p.flow), title: p.flow || '' }),
      el('td', { class: 'mono nowrap', text: p.current_phase || '--' }),
      el('td', { class: 'mono nowrap', text: p.last_updated_label || '--', title: p.last_updated || '' }),
      el('td', {}, p.attention_count
        ? el('span', { class: 'count-pink', text: String(p.attention_count) })
        : el('span', { class: 'mono', text: '0' })),
      el('td', { class: 'mono' }, latest
        ? el('span', { class: 'file-link', text: basename(latest.file), title: `${latest.slug} - ${latest.last_updated || ''}`, onclick: () => navigate('preview', { project: p.slug, file: latest.file }) })
        : '--',
        latest ? previewAction(p.slug, latest.file) : null),
      el('td', {}, p.visibility ? el('span', { class: `state ${p.visibility}`, text: `[${p.visibility}]` }) : '')));
  }
  table.append(tbody);
  body.replaceChildren(table);
  body.scrollTop = scrollTop;
}

// ---------- recent activity ----------

function escapeHtml(text) {
  return String(text || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function inlineMarkdown(text) {
  return escapeHtml(text)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '$1');
}

function activityExcerpt(entry) {
  const raw = String(entry.text || '').trim();
  const lines = raw.split(/\r?\n/).filter((line) => line.trim());
  const html = (lines.length ? lines : [raw]).map((line) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('- ')) {
      return `<div class="activity-bullet">${inlineMarkdown(trimmed.slice(2))}</div>`;
    }
    return `<div class="activity-line">${inlineMarkdown(trimmed)}</div>`;
  }).join('');
  const node = el('div', { class: 'activity-md excerpt' });
  node.innerHTML = html;
  return node;
}

function activityRow(entry) {
  const row = el('tr', {},
    el('td', { class: 'mono nowrap', text: entry.time_label || entry.timestamp || '---', title: entry.timestamp || '' }),
    el('td', { class: 'nowrap' }, projCell(entry.project, entry.project_name)),
    el('td', { class: 'mono nowrap', text: entry.event || '--' }),
    el('td', {},
      activityExcerpt(entry),
      entry.file ? previewAction(entry.project, entry.file) : null));
  row.addEventListener('click', (e) => {
    if (e.target.closest('button')) return;
    row.classList.toggle('expanded');
  });
  return row;
}

function renderActivityList() {
  const body = document.getElementById('activity-body');
  const table = el('table', { class: 'grid activity-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'Time' }), el('th', { text: 'Project' }),
    el('th', { text: 'Event' }), el('th', { text: 'Excerpt' }))));
  const tbody = el('tbody', { id: 'activity-rows' });
  if (!activityState.items.length) {
    tbody.append(el('tr', { class: 'empty-row' }, el('td', { colspan: '4', text: 'no activity yet' })));
  }
  for (const entry of activityState.items) tbody.append(activityRow(entry));
  table.append(tbody);
  body.replaceChildren(table);
  document.getElementById('activity-count').textContent = activityCountText();
}

async function loadMoreActivity() {
  if (activityState.loading || activityState.nextCursor === null) return;
  activityState.loading = true;
  try {
    const page = await getJSON(`/api/activity?cursor=${activityState.nextCursor}&limit=50`);
    activityState.nextCursor = page.next_cursor;
    activityState.items.push(...page.items);
    const tbody = document.getElementById('activity-rows');
    for (const entry of page.items) tbody.append(activityRow(entry));
    document.getElementById('activity-count').textContent = activityCountText();
  } finally {
    activityState.loading = false;
  }
}

let latestSnap = null;

export function applyActivityUpdate(snap) {
  latestSnap = snap;
  const body = document.getElementById('activity-body');
  const header = document.querySelector('#region-activity > header');
  const firstRaw = activityState.items[0]?.raw;
  const newFirstRaw = snap.recent_activity.items[0]?.raw;
  const isFresh = activityState.etag === null;
  const unchanged = firstRaw === newFirstRaw && activityState.items.length >= snap.recent_activity.items.length;

  if (isFresh || body.scrollTop < 40) {
    activityState.items = [...snap.recent_activity.items];
    activityState.nextCursor = snap.recent_activity.next_cursor;
    activityState.etag = snap.etag;
    header.querySelector('.chip')?.remove();
    renderActivityList();
  } else if (!unchanged && !header.querySelector('.chip')) {
    const chip = el('button', { class: 'chip', text: 'new activity - refresh', onclick: () => {
      activityState.etag = null;
      applyActivityUpdate(latestSnap);
      body.scrollTop = 0;
    } });
    header.append(chip);
  }
}

document.getElementById('activity-body').addEventListener('scroll', (e) => {
  const node = e.target;
  if (node.scrollTop + node.clientHeight > node.scrollHeight - 200) loadMoreActivity();
});

// ---------- entry ----------

export function renderDashboard(snap) {
  renderAttention(snap);
  renderProjects(snap);
}
