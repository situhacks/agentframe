// Dashboard: Attention, All Active Projects, Recent Activity.
// Each region scrolls internally; the page never scrolls. Activity pages
// lazily via cursor and never yanks the operator's scroll position.

import { getJSON, keycapEl, el, navigate } from './api.js?v=5';

const ZOOM_KEY = 'af-dashboard-zoom-v1';
const DEFAULT_ZOOM = 90;
const MIN_ZOOM = 60;
const MAX_ZOOM = 125;
const ZOOM_STEP = 5;
const FIT_BASE_WIDTH = 1360;

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
      navigate('preview', file ? { project, file } : { project });
    },
  });
}

function projCell(slug, name) {
  const label = name || slug;
  return el('div', { class: 'proj-cell' },
    keycapEl(slug, name),
    el('span', {
      class: 'pname file-link',
      text: label,
      title: label,
      onclick: (e) => {
        e.stopPropagation();
        navigate('preview', { project: slug });
      },
    }));
}

function plainLabel(value) {
  if (!value) return '--';
  return String(value).replace(/[-_]+/g, ' ').trim();
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
  const widths = [...dashboard.querySelectorAll('.scroll')]
    .map((node) => node.clientWidth)
    .filter(Boolean);
  const available = widths.length ? Math.min(...widths) : dashboard.clientWidth;
  const calculated = Math.floor((available / FIT_BASE_WIDTH * 100) / ZOOM_STEP) * ZOOM_STEP;
  applyZoom(calculated);
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

function stripLinks(text) {
  return String(text || '').replace(/\[([^\]]*)\]\([^)]*\)/g, '$1');
}

function formatShortDate(value) {
  if (!value) return '--';
  const [year, month, day] = value.split('-').map(Number);
  if (!year || !month || !day) return value;
  return new Date(Date.UTC(year, month - 1, day))
    .toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' });
}

function attentionTiming(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(String(value || ''))) {
    return { className: 'unscheduled', label: 'unscheduled', dateLabel: '--' };
  }
  const [year, month, day] = value.split('-').map(Number);
  const now = new Date();
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const target = Date.UTC(year, month - 1, day);
  const delta = Math.round((target - today) / 86400000);
  if (delta < 0) return { className: 'overdue', label: `${Math.abs(delta)}d overdue`, dateLabel: formatShortDate(value) };
  if (delta === 0) return { className: 'today', label: 'today', dateLabel: formatShortDate(value) };
  if (delta <= 7) return { className: 'soon', label: `in ${delta}d`, dateLabel: formatShortDate(value) };
  return { className: 'upcoming', label: 'upcoming', dateLabel: formatShortDate(value) };
}

function attentionWhen(value) {
  const timing = attentionTiming(value);
  return el('div', { class: `attention-when ${timing.className}`, title: value || '' },
    el('span', { class: 'urgency-label', text: timing.label }),
    el('span', { class: 'urgency-date', text: timing.dateLabel }));
}

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
    const reminderText = stripLinks(item.text);
    const timing = attentionTiming(item.date);
    const row = el('tr', { class: `urgency-${timing.className}` },
      el('td', { class: 'nowrap' }, projCell(item.project, item.project_name)),
      el('td', { class: 'mono nowrap' }, attentionWhen(item.date)),
      el('td', {}, el('span', { class: 'state attention', text: item.kind ? `[${item.kind}]` : '' })),
      el('td', {},
        item.file
          ? el('span', { class: 'excerpt file-link', text: reminderText, title: item.file, onclick: () => navigate('preview', { project: item.project, file: item.file }) })
          : el('span', { class: 'excerpt', text: reminderText }),
        previewAction(item.project, item.file, item.file ? 'preview ->' : 'project ->')));
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

function workStateCell(project) {
  const current = project.current_deliverable;
  if (!current) return el('span', { class: 'state untracked', text: '[untracked]' });
  const status = current.review === 'pending' ? 'review pending' : plainLabel(current.status);
  const title = [current.job, current.file, current.last_updated].filter(Boolean).join(' · ');
  const canPreview = current.file && current.status !== 'not_started';
  const workAttrs = {
    class: `work-name${canPreview ? ' file-link' : ''}`,
    text: plainLabel(current.slug),
    title,
  };
  if (canPreview) workAttrs.onclick = () => navigate('preview', { project: project.slug, file: current.file });
  return el('div', { class: 'work-cell' },
    el('span', workAttrs),
    el('span', { class: `state work-status ${current.status || 'unknown'}`, text: `[${status || '--'}]` }),
    canPreview ? previewAction(project.slug, current.file) : null);
}

function nextAttentionCell(project) {
  const next = project.next_attention;
  if (!next) return el('span', { class: 'mono', text: '--' });
  const text = stripLinks(next.text);
  const label = `${next.kind ? `[${next.kind}] ` : ''}${text}`;
  const canPreview = Boolean(next.file);
  const nextAttrs = {
    class: `next-action${canPreview ? ' file-link' : ''}`,
    text: label,
    title: [text, next.date, next.file].filter(Boolean).join(' · '),
  };
  if (canPreview) nextAttrs.onclick = () => navigate('preview', { project: project.slug, file: next.file });
  return el('div', { class: 'next-cell' },
    el('span', nextAttrs),
    canPreview ? previewAction(project.slug, next.file) : null);
}

function renderProjects(snap) {
  const body = document.getElementById('projects-body');
  const scrollTop = body.scrollTop;

  const table = el('table', { class: 'grid projects-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'Project' }), el('th', { text: 'Domain' }),
    el('th', { text: 'Governance' }), el('th', { text: 'Updated' }),
    el('th', { text: 'Attn' }),
    el('th', { text: 'Work state' }),
    el('th', { text: 'Next' }))));
  const tbody = el('tbody');

  if (!snap.projects.length) {
    tbody.append(el('tr', { class: 'empty-row' }, el('td', { colspan: '7', text: 'no active projects' })));
  }
  for (const p of snap.projects) {
    const governanceDocs = Object.entries(p.governance || {})
      .filter(([, present]) => present)
      .map(([name]) => name)
      .join(', ');
    tbody.append(el('tr', {},
      el('td', { class: 'nowrap' }, projCell(p.slug, p.name)),
      el('td', { class: 'mono nowrap', text: plainLabel(p.domain), title: p.flow || '' }),
      el('td', {}, el('span', {
        class: `state ${p.governance_status}`,
        text: `[${p.governance_status}]`,
        title: governanceDocs || 'no governance files',
      })),
      el('td', { class: 'mono nowrap', text: p.last_updated_label || '--', title: p.last_updated || '' }),
      el('td', {}, p.attention_count
        ? el('span', { class: 'count-pink', text: String(p.attention_count) })
        : el('span', { class: 'mono', text: '0' })),
      el('td', {}, workStateCell(p)),
      el('td', {}, nextAttentionCell(p))));
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
    el('td', { class: 'mono nowrap' }, el('span', { class: 'event-label', text: plainLabel(entry.event), title: entry.event || '' })),
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
