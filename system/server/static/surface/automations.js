// Managed automations: durable roster plus complete, paged run history.

import { getJSON, keycapEl, el, navigate } from './api.js?v=5';

const PAGE_SIZE = 50;
const receiptState = {
  items: [],
  nextCursor: null,
  total: 0,
  filter: 'all',
  loading: false,
  initialized: false,
  allHeadKey: null,
};

function plainLabel(value) {
  if (!value) return '--';
  return String(value).replace(/[-_]+/g, ' ').trim();
}

function projCell(slug, name) {
  const label = name || slug;
  return el('div', { class: 'proj-cell' },
    keycapEl(slug, name),
    el('span', {
      class: 'pname file-link', text: label, title: label,
      onclick: () => navigate('preview', { project: slug }),
    }));
}

function stateLabel(value, attention = false, label = null, title = '') {
  return el('span', {
    class: `state ${attention ? 'attention' : value || 'unknown'}`,
    text: `[${label || plainLabel(value)}]`,
    title,
  });
}

function humanAge(seconds) {
  if (seconds === null || seconds === undefined || Number.isNaN(seconds)) return '--';
  if (seconds < 60) return '<1m';
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}

function ageFromTime(value) {
  if (!value) return '--';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '--';
  return humanAge(Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000)));
}

function timeLabel(value) {
  if (!value) return '--';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString([], {
    month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
  });
}

function workerState(row) {
  if (row.runtime_state === 'offline') {
    return stateLabel('offline', false, 'not running', 'No fresh watcher heartbeat; neutral for on-demand --once runs.');
  }
  return stateLabel(row.runtime_state, false, null, row.current_task || '');
}

function lastRunCell(last) {
  if (!last) return el('span', { class: 'mono', text: '--' });
  const attention = ['blocked', 'failed'].includes(last.status);
  return el('div', { class: 'automation-last', title: [last.summary, timeLabel(last.time)].filter(Boolean).join(' · ') },
    stateLabel(last.status, attention),
    el('span', { class: 'mono automation-run-age', text: ageFromTime(last.time) }));
}

function issueCell(issues) {
  if (!issues?.length) return el('span', { class: 'mono', text: '--' });
  const label = issues.length === 1 ? plainLabel(issues[0]) : `${issues.length} issues`;
  return stateLabel(issues[0], true, label, issues.map(plainLabel).join(' · '));
}

function renderInventory(rows) {
  const body = document.getElementById('automations-body');
  const table = el('table', { class: 'grid automations-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'Automation' }), el('th', { text: 'Project' }),
    el('th', { text: 'Runs through' }), el('th', { text: 'Worker' }),
    el('th', { text: 'Queued' }), el('th', { text: 'Last run' }),
    el('th', { text: 'Health' }))));
  const tbody = el('tbody');
  if (!rows.length) {
    tbody.append(el('tr', { class: 'empty-row' },
      el('td', { colspan: '7', text: 'no managed automations declared or deployed' })));
  }
  for (const row of rows) {
    tbody.append(el('tr', {},
      el('td', { text: plainLabel(row.automation_id || row.deployment_id), title: row.job || '' }),
      el('td', {}, row.project ? projCell(row.project, row.project_name) : el('span', { text: '--' })),
      el('td', { class: 'mono', text: plainLabel(row.body_profile) }),
      el('td', {}, workerState(row)),
      el('td', {
        class: `mono${row.queued ? ' attention-text' : ''}`,
        text: row.queued ? `${row.queued} · ${humanAge(row.oldest_awaiting_seconds)}` : '0',
      }),
      el('td', {}, lastRunCell(row.last_result)),
      el('td', {}, issueCell(row.issues))));
  }
  table.append(tbody);
  body.replaceChildren(table);
  document.getElementById('automations-count').textContent = `${rows.length} total`;
}

function receiptKey(receipt) {
  if (!receipt) return '';
  return [receipt.deployment_id, receipt.task_id, receipt.status, receipt.time].join('|');
}

function receiptRow(receipt) {
  const row = el('tr', {},
    el('td', { class: 'mono nowrap', text: timeLabel(receipt.time), title: receipt.time || '' }),
    el('td', { text: plainLabel(receipt.automation_id || receipt.deployment_id) }),
    el('td', {}, receipt.project ? projCell(receipt.project, receipt.project_name) : el('span', { text: '--' })),
    el('td', {}, stateLabel(receipt.status, ['blocked', 'failed'].includes(receipt.status))),
    el('td', { class: 'mono', text: receipt.task_id || '--' }),
    el('td', {}, el('span', { class: 'excerpt', text: receipt.summary || '--' })));
  row.addEventListener('click', () => row.classList.toggle('expanded'));
  return row;
}

function receiptCountText() {
  const suffix = receiptState.nextCursor === null ? '' : '; loads more on scroll';
  return `showing ${receiptState.items.length} of ${receiptState.total}${suffix}`;
}

function renderReceiptList() {
  const body = document.getElementById('automation-receipts-body');
  const table = el('table', { class: 'grid automation-receipts-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'When' }), el('th', { text: 'Automation' }),
    el('th', { text: 'Project' }), el('th', { text: 'Status' }),
    el('th', { text: 'Request' }), el('th', { text: 'Receipt summary' }))));
  const tbody = el('tbody', { id: 'automation-receipt-rows' });
  if (!receiptState.items.length) {
    tbody.append(el('tr', { class: 'empty-row' },
      el('td', { colspan: '6', text: receiptState.filter === 'issues' ? 'no failed or blocked runs' : 'no receipts yet' })));
  }
  for (const receipt of receiptState.items) tbody.append(receiptRow(receipt));
  table.append(tbody);
  body.replaceChildren(table);
  document.getElementById('automation-receipts-count').textContent = receiptCountText();
}

function appendReceiptRows(items) {
  const tbody = document.getElementById('automation-receipt-rows');
  tbody?.querySelector('.empty-row')?.remove();
  for (const receipt of items) tbody?.append(receiptRow(receipt));
  document.getElementById('automation-receipts-count').textContent = receiptCountText();
}

function setActiveFilter() {
  for (const button of document.querySelectorAll('#automation-receipt-filters button')) {
    button.classList.toggle('active', button.dataset.status === receiptState.filter);
  }
}

function clearNewActivityChip() {
  document.querySelector('#automation-receipts-header .chip')?.remove();
}

function showNewActivityChip() {
  const header = document.getElementById('automation-receipts-header');
  if (header.querySelector('.chip')) return;
  header.append(el('button', {
    class: 'chip',
    text: 'new runs - refresh',
    onclick: () => resetReceipts(receiptState.filter),
  }));
}

async function resetReceipts(filter = receiptState.filter) {
  if (receiptState.loading) return;
  receiptState.loading = true;
  receiptState.filter = filter;
  setActiveFilter();
  try {
    const page = await getJSON(`/api/automations/receipts?cursor=0&limit=${PAGE_SIZE}&status=${encodeURIComponent(filter)}`);
    receiptState.items = [...page.items];
    receiptState.nextCursor = page.next_cursor;
    receiptState.total = page.total;
    receiptState.initialized = true;
    renderReceiptList();
    document.getElementById('automation-receipts-body').scrollTop = 0;
    clearNewActivityChip();
  } finally {
    receiptState.loading = false;
  }
}

async function loadMoreReceipts() {
  if (receiptState.loading || receiptState.nextCursor === null) return;
  receiptState.loading = true;
  try {
    const page = await getJSON(`/api/automations/receipts?cursor=${receiptState.nextCursor}&limit=${PAGE_SIZE}&status=${encodeURIComponent(receiptState.filter)}`);
    receiptState.nextCursor = page.next_cursor;
    receiptState.total = page.total;
    receiptState.items.push(...page.items);
    appendReceiptRows(page.items);
  } finally {
    receiptState.loading = false;
  }
}

function applyReceiptHead(model) {
  const head = model.recent_receipts || [];
  const nextHeadKey = receiptKey(head[0]);
  const changed = receiptState.initialized && receiptState.allHeadKey !== nextHeadKey;
  receiptState.allHeadKey = nextHeadKey;

  if (!receiptState.initialized) {
    receiptState.items = [...head];
    receiptState.nextCursor = model.receipts_next_cursor ?? null;
    receiptState.total = model.receipts_total || 0;
    receiptState.initialized = true;
    renderReceiptList();
    return;
  }
  if (!changed) return;

  const body = document.getElementById('automation-receipts-body');
  if (body.scrollTop >= 40) {
    showNewActivityChip();
    return;
  }
  if (receiptState.filter === 'all') {
    receiptState.items = [...head];
    receiptState.nextCursor = model.receipts_next_cursor ?? null;
    receiptState.total = model.receipts_total || 0;
    renderReceiptList();
  } else {
    resetReceipts(receiptState.filter);
  }
}

for (const button of document.querySelectorAll('#automation-receipt-filters button')) {
  button.addEventListener('click', () => resetReceipts(button.dataset.status));
}

document.getElementById('automation-receipts-body').addEventListener('scroll', (event) => {
  const node = event.target;
  if (node.scrollTop + node.clientHeight > node.scrollHeight - 200) loadMoreReceipts();
});

export function renderAutomations(model) {
  const safe = model || { rows: [], recent_receipts: [], receipts_total: 0 };
  renderInventory(safe.rows || []);
  applyReceiptHead(safe);
  const meta = document.getElementById('automations-meta');
  meta.textContent = safe.heartbeat_at ? `watcher last seen ${timeLabel(safe.heartbeat_at)}` : 'watcher not observed';
  meta.title = safe.registry_path || 'no local deployment registry';
}
