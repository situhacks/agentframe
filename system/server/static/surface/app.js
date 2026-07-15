// AgentFrame Local: shell, hash router, snapshot poller, freshness light.
// Dashboard has no CDN dependencies; the Preview module (Dockview) loads
// lazily the first time the Preview tab opens.

import { getJSON, postJSON, navigate, parseHash } from './api.js?v=5';
import { renderDashboard, applyActivityUpdate, setupDashboardDensity } from './dashboard.js?v=7';
import { renderAutomations } from './automations.js?v=3';
import { renderCalendar, setupCalendar } from './calendar.js?v=7';

const POLL_MS = 12000;

const state = {
  etag: null,
  snapshot: null,
  failures: 0,
  route: 'dashboard',
  preview: null,
};

// ---------- freshness ----------

const freshnessEl = document.getElementById('freshness');
const freshnessText = document.getElementById('freshness-text');

function setFreshness(mode, text) {
  freshnessEl.className = `freshness ${mode}`;
  freshnessText.textContent = text;
}

function timeNow() {
  return new Date().toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
}

// ---------- snapshot polling ----------

async function poll({ force = false } = {}) {
  getJSON('/api/automations').then(renderAutomations).catch(() => {
    renderAutomations({ rows: [] });
  });
  try {
    const query = state.etag && !force ? `?etag=${encodeURIComponent(state.etag)}` : '';
    const snap = await getJSON(`/api/snapshot${query}`);
    state.failures = 0;
    if (snap.unchanged) {
      setFreshness('', `last updated ${timeNow()}`);
      return;
    }
    state.etag = snap.etag;
    state.snapshot = snap;
    renderDashboard(snap);
    renderCalendar(snap);
    applyActivityUpdate(snap);
    const dashMeta = document.getElementById('dash-meta');
    dashMeta.textContent = `${snap.projects.length} active projects`;
    dashMeta.title = snap.workspace_root;
    const lockup = document.querySelector('.lockup');
    if (lockup) lockup.title = snap.workspace_root;
    setFreshness('', `last updated ${timeNow()}`);
  } catch (err) {
    state.failures += 1;
    if (state.failures >= 2) {
      const retry = document.createElement('button');
      retry.textContent = 'refresh';
      retry.addEventListener('click', () => poll({ force: true }));
      freshnessEl.className = 'freshness stale';
      freshnessText.textContent = 'stale - server unreachable, ';
      freshnessText.append(retry);
    } else {
      setFreshness('manual', 'manual refresh - retrying');
    }
  }
}

// ---------- router ----------

async function applyRoute() {
  const { path, params } = parseHash();
  const route = path.startsWith('preview') ? 'preview'
    : path.startsWith('calendar') ? 'calendar'
      : path.startsWith('automations') ? 'automations' : 'dashboard';
  state.route = route;

  const calendarWasHidden = document.getElementById('view-calendar').hidden;
  document.getElementById('view-dashboard').hidden = route !== 'dashboard';
  document.getElementById('view-automations').hidden = route !== 'automations';
  document.getElementById('view-calendar').hidden = route !== 'calendar';
  document.getElementById('view-preview').hidden = route !== 'preview';
  // re-render on entry: width-dependent layout (timeline day detail) needs
  // the view visible to measure
  if (route === 'calendar' && calendarWasHidden && state.snapshot) renderCalendar(state.snapshot);
  document.getElementById('tab-dashboard').classList.toggle('active', route === 'dashboard');
  document.getElementById('tab-automations').classList.toggle('active', route === 'automations');
  document.getElementById('tab-calendar').classList.toggle('active', route === 'calendar');
  document.getElementById('tab-preview').classList.toggle('active', route === 'preview');

  if (route === 'preview') {
    if (!state.preview) {
      try {
        state.preview = await import('./preview.js?v=5');
        await state.preview.mountPreview();
      } catch (err) {
        state.preview = null;
        document.getElementById('editor-area').innerHTML =
          `<div class="viewer-note"><b>Preview failed to load.</b><br>` +
          `The tab layout library loads from cdn.jsdelivr.net - check the network connection and refresh.<br>` +
          `<span class="warn">${String(err.message || err)}</span></div>`;
        return;
      }
    }
    const project = params.get('project');
    const file = params.get('file');
    if (project) state.preview.focus(project, file);
  }
}

// ---------- boot ----------

for (const btn of document.querySelectorAll('.top-tabs button')) {
  btn.addEventListener('click', () => navigate(btn.dataset.route));
}
document.getElementById('refresh-snapshot').addEventListener('click', () => poll({ force: true }));
document.querySelector('.lockup')?.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  postJSON('/api/reveal-root', {}).catch(() => {});
});
window.addEventListener('hashchange', applyRoute);
window.addEventListener('agentframe:navigate', applyRoute);
window.addEventListener('focus', () => poll());

setupDashboardDensity();
setupCalendar();
setFreshness('manual', 'connecting...');
poll().then(applyRoute);
setInterval(poll, POLL_MS);
