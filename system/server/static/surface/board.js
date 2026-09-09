// Board tab: work in flight across projects, read from workspace/board.md by
// /api/board and joined to the live session roster. Visual identity comes from
// the shell's helpers (keycapEl, colorFor, el) and its .region / .dash-header
// components; every size scales off --dash-zoom exactly like the Dashboard tab.
// The card is the jump to its chat: the whole card links to the VS Code session.

import { keycapEl, el } from './api.js?v=5';
import { colorFor } from './ribbons.js?v=7';

const LANES = ['Queued', 'In progress', 'Needs you', 'Done'];
const VSCODE_OPEN = 'vscode://anthropic.claude-code/open?session=';
const ZOOM_KEY = 'af-board-zoom-v1';
const ARCHIVE_KEY = 'af-board-archive-v1';

const boardState = { model: null, archive: false };

function plain(value) {
  return String(value || '').replace(/[-_]+/g, ' ').trim();
}

function ageLabel(hours) {
  if (hours === null || hours === undefined || Number.isNaN(hours)) return '';
  if (hours < 1) return 'now';
  if (hours < 48) return `${hours}h`;
  return `${Math.round(hours / 24)}d`;
}

function stateClass(state) {
  return String(state || 'unknown').toLowerCase().replace(/\s+/g, '_');
}

// ---------- cards ----------

function cardEl(card, names) {
  const sid = card.session_id || null;
  const state = card.state || 'unknown';
  const title = `${card.id} · @${card.owner}${card.since ? ` · since ${card.since}` : ''}`;
  const attrs = { class: 'bcard', style: `--pc:${colorFor(card.project)}`, title };
  const node = sid ? el('a', { ...attrs, href: `${VSCODE_OPEN}${encodeURIComponent(sid)}` }) : el('div', attrs);

  node.append(el('div', { class: 'row' },
    el('span', { class: 'deliv', text: plain(card.deliverable) }),
    el('span', { class: `state ${stateClass(state)}`, text: plain(state) })));
  if (card.ask) node.append(el('div', { class: 'ask', text: card.ask === 'review' ? 'review & approve' : 'answer needed' }));
  if (card.reason) node.append(el('div', { class: 'reason', text: card.reason }));

  const meta = el('div', { class: 'meta' },
    el('span', { text: ageLabel(card.age_hours) }),
    el('span', { text: card.by === 'human' ? 'you' : (card.by || '') }),
    card.model ? el('span', { text: card.model, title: 'model agreed for this card' }) : null);
  if (card.ask === 'review' && card.receipt) {
    meta.append(el('a', {
      href: `/workspace/board/${card.receipt}`, text: 'receipt', target: '_blank',
      onclick: (e) => e.stopPropagation(),
    }));
  }
  meta.append(el('span', { class: 'jump', text: sid ? 'open chat ↗' : '' }));
  node.append(meta);
  return node;
}

function archiveCardEl(card, names) {
  const node = el('div', { class: 'bcard archived', style: `--pc:${colorFor(card.project)}`, title: card.id });
  node.append(el('div', { class: 'row' },
    el('span', { class: 'deliv', text: plain(card.deliverable) }),
    el('span', { class: `state ${card.outcome}`, text: card.outcome })));
  if (card.note) node.append(el('div', { class: 'reason', text: card.note }));
  node.append(el('div', { class: 'meta' },
    el('span', { text: card.closed_at ? new Date(card.closed_at).toLocaleDateString([], { month: 'short', day: 'numeric' }) : '' }),
    el('span', { text: card.by === 'human' ? 'you' : (card.by || '') })));
  return node;
}

function groupByProject(cards) {
  const groups = new Map();
  for (const card of cards) {
    if (!groups.has(card.project)) groups.set(card.project, []);
    groups.get(card.project).push(card);
  }
  return groups;
}

function laneEl(name, cards, names, { live = false, render = cardEl } = {}) {
  const body = el('div', { class: 'scroll' });
  if (!cards.length) body.append(el('div', { class: 'empty', text: 'empty' }));
  for (const [slug, items] of groupByProject(cards)) {
    const label = names[slug] || plain(slug);
    const group = el('div', { class: 'pgroup' },
      el('div', { class: 'proj-cell' }, keycapEl(slug, label), el('span', { class: 'pname', text: label, title: slug }),
        items.length > 1 ? el('span', { class: 'n', text: `${items.length}` }) : null));
    for (const card of items) group.append(render(card, names));
    body.append(group);
  }
  return el('section', { class: `region lane${live ? ' live' : ''}` },
    el('header', {}, el('span', { class: 'title', text: name }), el('span', { class: 'count', text: `${cards.length}` })),
    body);
}

// ---------- render ----------

export function renderBoard(model) {
  boardState.model = model;
  const lanesEl = document.getElementById('board-lanes');
  const metaEl = document.getElementById('board-meta');
  if (!lanesEl || !metaEl) return;
  lanesEl.replaceChildren();
  lanesEl.classList.toggle('with-archive', boardState.archive);

  if (!model || !model.exists) {
    lanesEl.append(el('div', { class: 'board-note' },
      el('b', { text: 'No board yet.' }),
      el('span', { text: ` Run ${model?.init_hint || 'python system/af.py board init'} in the vault, then add a card with af board add.` })));
    metaEl.textContent = '';
    document.getElementById('tab-board')?.classList.remove('live');
    return;
  }

  const names = model.projects || {};
  const byName = new Map((model.lanes || []).map((lane) => [lane.name, lane.cards || []]));
  for (const name of LANES) {
    lanesEl.append(laneEl(name, byName.get(name) || [], names, { live: name === 'Needs you' && (byName.get(name) || []).length > 0 }));
  }
  if (boardState.archive) {
    lanesEl.append(laneEl('Archive', model.archive || [], names, { render: archiveCardEl }));
  }
  if (model.errors && model.errors.length) {
    lanesEl.append(el('div', { class: 'board-errors', text: `board.md grammar: ${model.errors.join(' | ')}` }));
  }

  const waiting = model.waiting_on_you || 0;
  const bits = [`${model.open || 0} open`, `${waiting} waiting on you`];
  if (model.roster_available === false) bits.push('roster unavailable');
  if (model.issues && model.issues.length) bits.push(`${model.issues.length} flagged`);
  metaEl.textContent = bits.join(' · ');
  metaEl.title = (model.issues || []).join('\n');
  document.getElementById('tab-board')?.classList.toggle('live', waiting > 0);
}

// ---------- controls (same pattern as the Dashboard tab) ----------

function applyZoom(value, { persist = true } = {}) {
  const zoom = Math.max(60, Math.min(125, Number.parseInt(value, 10) || 90));
  document.getElementById('view-board')?.style.setProperty('--dash-zoom', String(zoom / 100));
  const input = document.getElementById('board-zoom-input');
  if (input) input.value = String(zoom);
  if (persist) { try { localStorage.setItem(ZOOM_KEY, String(zoom)); } catch { /* best effort */ } }
}

export function setupBoard() {
  let saved = 90;
  try { saved = localStorage.getItem(ZOOM_KEY) || 90; } catch { /* default */ }
  applyZoom(saved, { persist: false });
  const input = document.getElementById('board-zoom-input');
  document.getElementById('board-zoom-out')?.addEventListener('click', () => applyZoom(Number(input.value) - 5));
  document.getElementById('board-zoom-in')?.addEventListener('click', () => applyZoom(Number(input.value) + 5));
  input?.addEventListener('change', (e) => applyZoom(e.target.value));

  const archive = document.getElementById('board-archive');
  try { boardState.archive = localStorage.getItem(ARCHIVE_KEY) === '1'; } catch { /* default off */ }
  if (archive) {
    archive.checked = boardState.archive;
    archive.addEventListener('change', () => {
      boardState.archive = archive.checked;
      try { localStorage.setItem(ARCHIVE_KEY, boardState.archive ? '1' : '0'); } catch { /* best effort */ }
      renderBoard(boardState.model);
    });
  }
}
