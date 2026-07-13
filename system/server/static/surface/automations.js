// Managed automations pulse: requests in, terminal receipts out, exceptions visible.

import { keycapEl, el, navigate } from './api.js?v=5';

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

function stateLabel(value, attention = false) {
  return el('span', {
    class: `state ${attention ? 'attention' : value || 'unknown'}`,
    text: `[${plainLabel(value)}]`,
  });
}

function summaryCard(label, value, detail, tone = '') {
  return el('div', { class: `automation-summary-card ${tone}` },
    el('span', { class: 'summary-label', text: label }),
    el('strong', { text: String(value) }),
    el('span', { class: 'summary-detail', text: detail }));
}

function humanAge(seconds) {
  if (seconds === null || seconds === undefined) return '--';
  if (seconds < 60) return '<1m';
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}

function timeLabel(value) {
  if (!value) return '--';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString([], {
    month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
  });
}

function receiptCount(row) {
  return (row.today?.done || 0) + (row.today?.blocked || 0) + (row.today?.failed || 0);
}

function renderSummary(model) {
  const rows = model.rows || [];
  const requests = rows.reduce((sum, row) => sum + (row.requests_today || 0), 0);
  const receipts = rows.reduce((sum, row) => sum + receiptCount(row), 0);
  const done = rows.reduce((sum, row) => sum + (row.today?.done || 0), 0);
  const awaiting = rows.reduce((sum, row) => sum + (row.queued || 0), 0);
  const blocked = rows.reduce((sum, row) => sum + (row.today?.blocked || 0), 0);
  const failed = rows.reduce((sum, row) => sum + (row.today?.failed || 0), 0);
  document.getElementById('automations-summary').replaceChildren(
    summaryCard('Automations', rows.length, 'declared or deployed'),
    summaryCard('Requests today', requests, 'task files received'),
    summaryCard('Receipts today', receipts, `${done} done · ${blocked} blocked · ${failed} failed`, blocked || failed ? 'warn' : 'good'),
    summaryCard('Awaiting receipt', awaiting, 'queued or processing', awaiting ? 'warn' : 'good'),
    summaryCard('Exceptions today', blocked + failed, 'terminal receipts needing review', blocked || failed ? 'warn' : 'good'));
}

function renderAttention(model) {
  const attention = model.attention || [];
  const body = document.getElementById('automation-attention-body');
  const table = el('table', { class: 'grid attention-automation-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'When' }), el('th', { text: 'Automation' }),
    el('th', { text: 'Project' }), el('th', { text: 'Issue' }),
    el('th', { text: 'Request' }), el('th', { text: 'Summary' }))));
  const tbody = el('tbody');
  if (!attention.length) {
    tbody.append(el('tr', { class: 'empty-row good-empty' },
      el('td', { colspan: '6', text: 'every observed request has a terminal receipt and no runtime mismatch needs attention' })));
  }
  for (const item of attention) {
    tbody.append(el('tr', {},
      el('td', { class: 'mono nowrap', text: item.kind === 'unanswered' ? humanAge(item.age_seconds) : timeLabel(item.time) }),
      el('td', { text: plainLabel(item.automation_id || item.deployment_id) }),
      el('td', {}, item.project ? projCell(item.project, item.project_name) : el('span', { text: '--' })),
      el('td', {}, stateLabel(item.kind, true)),
      el('td', { class: 'mono', text: item.task_id || '--' }),
      el('td', { text: item.summary || '--' })));
  }
  table.append(tbody);
  body.replaceChildren(table);
  document.getElementById('automation-attention-count').textContent = attention.length ? `${attention.length} open` : 'clear';
}

function renderInventory(rows) {
  const body = document.getElementById('automations-body');
  const table = el('table', { class: 'grid automations-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'Automation' }), el('th', { text: 'Project' }),
    el('th', { text: 'Runs through' }), el('th', { text: 'Worker' }),
    el('th', { text: 'Requests' }), el('th', { text: 'Receipts' }),
    el('th', { text: 'Awaiting' }), el('th', { text: 'Last receipt' }))));
  const tbody = el('tbody');
  if (!rows.length) {
    tbody.append(el('tr', { class: 'empty-row' },
      el('td', { colspan: '8', text: 'no managed automations declared or deployed' })));
  }
  for (const row of rows) {
    const last = row.last_result;
    tbody.append(el('tr', {},
      el('td', { text: plainLabel(row.automation_id || row.deployment_id), title: row.job || '' }),
      el('td', {}, row.project ? projCell(row.project, row.project_name) : el('span', { text: '--' })),
      el('td', { class: 'mono', text: plainLabel(row.body_profile) }),
      el('td', {}, stateLabel(row.runtime_state, row.issues?.length)),
      el('td', { class: 'mono', text: String(row.requests_today || 0) }),
      el('td', { class: 'mono', text: String(receiptCount(row)) }),
      el('td', { class: `mono${row.queued ? ' attention-text' : ''}`, text: row.queued ? `${row.queued} · ${humanAge(row.oldest_awaiting_seconds)}` : '0' }),
      el('td', { class: 'automation-last', text: last ? `${plainLabel(last.status)}: ${last.summary || last.task_id}` : '--', title: last?.time || '' })));
  }
  table.append(tbody);
  body.replaceChildren(table);
  document.getElementById('automations-count').textContent = `${rows.length} total`;
}

function renderReceipts(receipts) {
  const body = document.getElementById('automation-receipts-body');
  const table = el('table', { class: 'grid automation-receipts-table' });
  table.append(el('thead', {}, el('tr', {},
    el('th', { text: 'When' }), el('th', { text: 'Automation' }),
    el('th', { text: 'Project' }), el('th', { text: 'Status' }),
    el('th', { text: 'Request' }), el('th', { text: 'Receipt summary' }))));
  const tbody = el('tbody');
  if (!receipts.length) {
    tbody.append(el('tr', { class: 'empty-row' }, el('td', { colspan: '6', text: 'no receipts yet' })));
  }
  for (const receipt of receipts) {
    tbody.append(el('tr', {},
      el('td', { class: 'mono nowrap', text: timeLabel(receipt.time) }),
      el('td', { text: plainLabel(receipt.automation_id || receipt.deployment_id) }),
      el('td', {}, receipt.project ? projCell(receipt.project, receipt.project_name) : el('span', { text: '--' })),
      el('td', {}, stateLabel(receipt.status, ['blocked', 'failed'].includes(receipt.status))),
      el('td', { class: 'mono', text: receipt.task_id || '--' }),
      el('td', { text: receipt.summary || '--' })));
  }
  table.append(tbody);
  body.replaceChildren(table);
  document.getElementById('automation-receipts-count').textContent = `latest ${receipts.length}`;
}

export function renderAutomations(model) {
  const safe = model || { rows: [], attention: [], recent_receipts: [] };
  renderSummary(safe);
  renderAttention(safe);
  renderInventory(safe.rows || []);
  renderReceipts(safe.recent_receipts || []);
  const meta = document.getElementById('automations-meta');
  const heartbeat = safe.heartbeat_at ? `worker heartbeat ${timeLabel(safe.heartbeat_at)}` : 'no worker heartbeat';
  meta.textContent = heartbeat;
  meta.title = safe.registry_path || 'no local deployment registry';
}
