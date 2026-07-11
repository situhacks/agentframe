// Preview: project rail + IDE-style editor groups (Dockview) + viewers.
// Loaded lazily by app.js so the Dashboard never depends on the CDN.

import { createDockview } from 'https://cdn.jsdelivr.net/npm/dockview-core@4.13.1/+esm';
import { getJSON, postJSON, keycapEl, basename, el } from './api.js';
import { renderViewer } from './viewers.js';

const LAYOUT_KEY = 'af-surface-layout-v1';
const PDF_SCALE_KEY = 'af-preview-pdf-scale-v1';
const PDF_SCALE_EVENT = 'agentframe:pdf-scale';
const PDF_SCALE_DEFAULT = 0.55;
const PDF_SCALE_MIN = 0.25;
const PDF_SCALE_MAX = 1.5;

const rail = {
  projects: [],
  selected: null,
  groups: [],
  untrackedCount: 0,
  details: new Map(), // group slug -> {versions, exports}
  expanded: new Set(),
  untrackedOpen: false,
  untrackedFiles: null,
  filterClass: 'media',
  filterType: null,
  flatFiles: [],
  flatCacheKey: null,
};

let dockview = null;
let saveTimer = null;

// ---------- context menu ----------

let menuEl = null;
function closeMenu() {
  menuEl?.remove();
  menuEl = null;
}
function showMenu(x, y, items) {
  closeMenu();
  menuEl = el('div', { class: 'ctx-menu' });
  for (const item of items) {
    if (item === '---') { menuEl.append(el('hr')); continue; }
    if (!item) continue;
    const btn = el('button', { text: item.label, onclick: () => { closeMenu(); item.action(); } });
    if (item.disabled) btn.disabled = true;
    menuEl.append(btn);
  }
  document.body.append(menuEl);
  const rect = menuEl.getBoundingClientRect();
  menuEl.style.left = `${Math.min(x, window.innerWidth - rect.width - 6)}px`;
  menuEl.style.top = `${Math.min(y, window.innerHeight - rect.height - 6)}px`;
  setTimeout(() => {
    window.addEventListener('mousedown', onAway, { once: true });
    window.addEventListener('keydown', onEsc);
  });
}
function onAway(e) { if (!menuEl?.contains(e.target)) closeMenu(); }
function onEsc(e) { if (e.key === 'Escape') { closeMenu(); window.removeEventListener('keydown', onEsc); } }

// ---------- helpers ----------

async function fileMeta(project, file) {
  const query = `project=${encodeURIComponent(project)}&file=${encodeURIComponent(file)}`;
  return getJSON(`/api/preview?${query}`);
}

async function copyPath(project, file) {
  try {
    const meta = await fileMeta(project, file);
    await navigator.clipboard.writeText(meta.os_path || file);
  } catch { /* clipboard denied — non-fatal */ }
}

function reveal(project, file) {
  postJSON('/api/reveal', { project, file }).catch(() => {});
}

function flowLabel(flow) {
  if (!flow) return '--';
  return String(flow).replace(/^marketing-/, '').replace(/-flow$/, '');
}

function isPdfLike(meta) {
  return ['pdf', 'office'].includes(meta?.type);
}

function clampPdfScale(scale) {
  return Math.max(PDF_SCALE_MIN, Math.min(PDF_SCALE_MAX, scale));
}

function storedPdfScale() {
  try {
    const parsed = Number.parseFloat(localStorage.getItem(PDF_SCALE_KEY));
    if (Number.isFinite(parsed)) return clampPdfScale(parsed);
  } catch { /* storage is best-effort */ }
  return PDF_SCALE_DEFAULT;
}

function storePdfScale(scale) {
  try {
    localStorage.setItem(PDF_SCALE_KEY, String(clampPdfScale(scale)));
  } catch { /* storage is best-effort */ }
}

// ---------- dockview panels ----------

class ViewerPanel {
  constructor() {
    this.element = el('div', { class: 'viewer-host' });
    this.scale = 0.82;
    this.viewerControl = null;
  }

  init(initParams) {
    this.api = initParams.api;
    const { project, file } = initParams.params;
    this.project = project;
    this.file = file;
    this.onPdfScale = (event) => {
      if (isPdfLike(this.meta) && event.detail?.source !== this) {
        this.setScale(event.detail.scale, { persist: false });
      }
    };
    window.addEventListener(PDF_SCALE_EVENT, this.onPdfScale);

    this.toolbar = el('div', { class: 'viewer-toolbar' },
      el('span', { class: 'path', text: file, title: file }));
    this.body = el('div', { class: 'viewer-body' });
    this.element.replaceChildren(this.toolbar, this.body);

    this.zoomOut = el('button', { text: '-', title: 'zoom out', onclick: () => this.adjustScale(-0.10) });
    this.zoomLabel = el('span', { class: 'viewer-zoom', text: '82%', title: 'viewer scale' });
    this.zoomIn = el('button', { text: '+', title: 'zoom in', onclick: () => this.adjustScale(0.10) });
    this.zoomReset = el('button', {
      text: '100%', title: 'actual size', onclick: () => this.setScale(1, { broadcast: true }),
    });
    this.zoomGroup = el('div', { class: 'viewer-zoom-group', hidden: '' },
      this.zoomOut, this.zoomLabel, this.zoomIn, this.zoomReset);
    this.toolbar.append(this.zoomGroup);

    const actions = [
      ['open original', () => this.meta && window.open(this.meta.url, '_blank')],
      ['reveal', () => reveal(project, file)],
      ['refresh', () => this.load()],
      ['copy path', () => copyPath(project, file)],
      ['close', () => this.api.close()],
    ];
    for (const [label, action] of actions) {
      this.toolbar.append(el('button', { text: label, title: label, onclick: action }));
    }

    this.body.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const img = this.body.querySelector('.viewer-center img');
      showMenu(e.clientX, e.clientY, [
        img && { label: 'Fit', action: () => img.classList.add('fit') },
        img && { label: 'Actual Size', action: () => img.classList.remove('fit') },
        img && '---',
        { label: 'Copy Path', action: () => copyPath(project, file) },
        { label: 'Open Original', action: () => this.meta && window.open(this.meta.url, '_blank') },
        { label: 'Reveal File', action: () => reveal(project, file) },
      ]);
    });

    this.load();
  }

  defaultScaleFor(meta) {
    if (isPdfLike(meta)) return storedPdfScale();
    if (meta?.type === 'html') return 0.82;
    return 1;
  }

  setScale(scale, { persist = true, broadcast = false } = {}) {
    this.scale = Math.max(0.25, Math.min(1.5, scale));
    if (this.viewerControl?.setScale) this.scale = this.viewerControl.setScale(this.scale);
    if (isPdfLike(this.meta) && persist) storePdfScale(this.scale);
    if (this.zoomLabel) this.zoomLabel.textContent = `${Math.round(this.scale * 100)}%`;
    if (isPdfLike(this.meta) && broadcast) {
      window.dispatchEvent(new CustomEvent(PDF_SCALE_EVENT, {
        detail: { scale: this.scale, source: this },
      }));
    }
  }

  adjustScale(delta) {
    this.setScale(this.scale + delta, { broadcast: true });
  }

  setZoomVisible(visible) {
    if (this.zoomGroup) this.zoomGroup.hidden = !visible;
  }

  async load() {
    try {
      this.meta = await fileMeta(this.project, this.file);
      this.scale = this.defaultScaleFor(this.meta);
      this.viewerControl = await renderViewer(this.meta, this.body, { scale: this.scale });
      this.setZoomVisible(Boolean(this.viewerControl?.zoomable));
      this.setScale(this.scale);
    } catch (err) {
      this.setZoomVisible(false);
      this.body.replaceChildren(el('div', { class: 'viewer-note' },
        el('b', { text: this.file }), document.createElement('br'),
        el('span', { class: 'warn', text: err.message })));
    }
  }

  dispose() {
    window.removeEventListener(PDF_SCALE_EVENT, this.onPdfScale);
  }
}

class AfTab {
  constructor() {
    this.element = el('div', { class: 'af-tab' });
  }

  init(initParams) {
    const api = initParams.api;
    const { project, file } = initParams.params;
    const title = el('span', { class: 'tab-title', text: api.title || basename(file || '') });
    api.onDidTitleChange?.(() => { title.textContent = api.title; });
    const close = el('button', {
      class: 'tab-close', text: 'x', title: 'close',
      onclick: (e) => { e.stopPropagation(); api.close(); },
    });
    this.element.append(title, close);
    this.element.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      e.stopPropagation();
      showMenu(e.clientX, e.clientY, [
        { label: 'Close', action: () => api.close() },
        { label: 'Close Others', action: () => closePanels((p) => p.id !== api.id) },
        { label: 'Close All', action: () => closePanels(() => true) },
        '---',
        { label: 'Open to Side', action: () => openFile(project, file, { direction: 'right' }) },
        { label: 'Move into New Group', action: () => moveToNewGroup(api.id) },
        '---',
        { label: 'Reveal File', action: () => reveal(project, file) },
        { label: 'Copy Path', action: () => copyPath(project, file) },
      ]);
    });
  }
}

function closePanels(predicate) {
  for (const panel of [...dockview.panels]) {
    if (predicate(panel)) panel.api.close();
  }
}

function moveToNewGroup(panelId) {
  const panel = dockview.panels.find((p) => p.id === panelId);
  if (!panel) return;
  try {
    const group = dockview.addGroup({ direction: 'right' });
    panel.api.moveTo({ group });
  } catch { /* layout op failed — leave panel where it is */ }
}

export function openFile(project, file, { direction = null, forceNew = false } = {}) {
  if (!dockview || !file) return;
  const baseId = `${project}::${file}`;
  const existing = dockview.panels.find((p) => p.id === baseId);
  if (existing && !direction && !forceNew) {
    existing.api.setActive();
    return;
  }
  let id = baseId;
  if (existing || forceNew) id = `${baseId}::${Date.now()}`;
  const options = {
    id,
    component: 'viewer',
    tabComponent: 'af',
    title: basename(file),
    params: { project, file },
  };
  if (direction) {
    const ref = dockview.activePanel;
    options.position = ref ? { referencePanel: ref.id, direction } : { direction };
  }
  dockview.addPanel(options);
}

// ---------- rail: projects ----------

async function renderProjects() {
  const host = document.getElementById('rail-projects');
  const data = await getJSON('/api/projects');
  rail.projects = data.projects;
  host.replaceChildren();
  for (const p of rail.projects) {
    const row = el('button', { class: 'rail-row', 'data-slug': p.slug },
      keycapEl(p.slug, p.name),
      el('span', { class: 'grow' },
        el('span', { text: p.name || p.slug }),
        el('span', {
          class: 'sub',
          text: `${flowLabel(p.flow)} / ${p.current_phase || '--'} / ${p.last_updated_label || '--'}`,
          title: p.last_updated || '',
        })),
      p.attention_count ? el('span', { class: 'count-pink', text: String(p.attention_count) }) : null);
    row.addEventListener('click', () => selectProject(p.slug));
    host.append(row);
  }
}

async function selectProject(slug) {
  if (rail.selected === slug) return;
  rail.selected = slug;
  rail.details = new Map();
  rail.expanded = new Set();
  rail.untrackedOpen = false;
  rail.untrackedFiles = null;
  rail.flatFiles = [];
  rail.flatCacheKey = null;
  for (const row of document.querySelectorAll('#rail-projects .rail-row')) {
    row.classList.toggle('selected', row.dataset.slug === slug);
  }
  const host = document.getElementById('rail-artifacts');
  host.replaceChildren(el('div', { class: 'rail-section-note', text: 'loading…' }));
  try {
    const data = await getJSON(`/api/projects/${encodeURIComponent(slug)}/artifacts`);
    rail.groups = data.groups;
    rail.untrackedCount = data.untracked_count;
    renderArtifacts();
  } catch (err) {
    host.replaceChildren(el('div', { class: 'rail-section-note', text: `failed: ${err.message}` }));
  }
}

// ---------- rail: artifacts ----------

function versionTag(g) {
  const parts = [];
  if (g.status) parts.push(g.status);
  if (g.version_count > 1) parts.push(`${g.version_count} versions`);
  if (g.types?.length) parts.push(g.types.join('/'));
  if (g.last_updated) parts.push(g.last_updated);
  if (g.archived) parts.push('archived');
  return parts.join(' / ');
}

function syncFilterControls() {
  for (const btn of document.querySelectorAll('#artifact-class-filters button')) {
    btn.classList.toggle('active', btn.dataset.class === rail.filterClass);
  }
  const typeHost = document.getElementById('artifact-type-filters');
  typeHost.hidden = rail.filterClass !== 'media';
  for (const btn of typeHost.querySelectorAll('button')) {
    btn.classList.toggle('active', btn.dataset.type === rail.filterType);
  }
}

async function setArtifactClass(nextClass) {
  if (rail.filterClass === nextClass) return;
  rail.filterClass = nextClass;
  rail.filterType = null;
  renderArtifacts();
}

async function setArtifactType(nextType) {
  rail.filterType = rail.filterType === nextType ? null : nextType;
  renderArtifacts();
}

function typeForPath(path) {
  const ext = String(path || '').split('.').pop()?.toLowerCase();
  if (['md', 'txt'].includes(ext)) return 'text';
  if (['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg'].includes(ext)) return 'image';
  if (ext === 'pdf') return 'pdf';
  if (['mp4', 'mov', 'webm'].includes(ext)) return 'video';
  if (ext === 'html') return 'html';
  if (['pptx', 'docx'].includes(ext)) return 'office';
  return 'unsupported';
}

function groupMatchesFilter(g) {
  if (rail.filterClass === 'all') return true;
  const types = (g.types || []).map((ext) => typeForPath(`x.${ext}`));
  if (rail.filterClass === 'text') return types.includes('text');
  if (rail.filterClass === 'media') {
    return types.some((type) => type !== 'text' && (!rail.filterType || type === rail.filterType));
  }
  return true;
}

function detailFilesForFilter(g, detail) {
  const versions = detail?.versions || [];
  const media = [
    ...(detail?.manifest_media || []),
    ...(detail?.exports || []),
    ...(detail?.folder_media || []),
  ];
  let files;
  if (rail.filterClass === 'text') {
    files = versions.filter((file) => typeForPath(file) === 'text');
  } else if (rail.filterClass === 'media') {
    files = media.filter((file) => {
      const type = typeForPath(file);
      return type !== 'text' && (!rail.filterType || type === rail.filterType);
    });
    if (g.current && typeForPath(g.current) !== 'text' && (!rail.filterType || typeForPath(g.current) === rail.filterType)) {
      files.unshift(g.current);
    }
  } else {
    files = [...versions, ...media];
  }
  return [...new Set(files)];
}

async function renderArtifacts() {
  const host = document.getElementById('rail-artifacts');
  syncFilterControls();
  const visibleGroups = rail.groups.filter(groupMatchesFilter);
  document.getElementById('rail-artifacts-count').textContent =
    visibleGroups.length ? `${visibleGroups.length} groups` : '';
  host.replaceChildren();

  if (!visibleGroups.length) {
    host.append(el('div', { class: 'rail-section-note', text: 'no matching tracked deliverables' }));
  }

  for (const g of visibleGroups) {
    const expanded = rail.expanded.has(g.slug);
    const row = el('button', { class: 'rail-row' },
      el('span', { class: 'chev', title: expanded ? 'hide versions' : 'show versions', text: expanded ? 'v' : '>' }),
      el('span', { class: 'grow' },
        el('span', { text: g.label }),
        el('span', { class: 'sub', text: versionTag(g) })),
      g.current ? el('span', { class: 'pv-act', text: 'open ->' }) : null);
    row.addEventListener('click', (e) => {
      if (e.target.closest('.chev')) {
        toggleGroup(g);
        return;
      }
      if (e.target.closest('.pv-act') || g.current) {
        openGroupPrimary(g);
        return;
      }
    });
    row.addEventListener('dblclick', (e) => {
      e.preventDefault();
      if (g.current) openGroupPrimary(g);
    });
    row.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      groupMenu(e, g);
    });
    host.append(row);
    if (expanded) renderGroupDetail(host, g);
  }

  // untracked escape hatch, collapsed by default
  if (rail.untrackedCount) {
    const row = el('button', { class: 'rail-row' },
      el('span', { class: 'chev', text: rail.untrackedOpen ? '▾' : '▸' }),
      el('span', { class: 'grow' },
        el('span', { text: 'See All Untracked Previewable Files' }),
        el('span', { class: 'sub', text: `${rail.untrackedCount} files` })));
    row.addEventListener('click', toggleUntracked);
    host.append(row);
    if (rail.untrackedOpen && rail.untrackedFiles) {
      for (const file of rail.untrackedFiles) {
        host.append(fileRow(file, { tag: '' }));
      }
    }
  }
}

function fileRow(file, { tag = '', current = false } = {}) {
  const row = el('button', { class: `rail-file${current ? ' current-file' : ''}`, title: file },
    el('span', { class: 'grow', text: basename(file) }),
    el('span', { class: 'tag', text: current ? 'current' : tag }));
  row.addEventListener('click', () => openFile(rail.selected, file));
  row.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    fileMenu(e, file);
  });
  return row;
}

async function toggleGroup(g) {
  if (rail.expanded.has(g.slug)) {
    rail.expanded.delete(g.slug);
    renderArtifacts();
    return;
  }
  rail.expanded.add(g.slug);
  await ensureGroupDetail(g);
  renderArtifacts();
}

async function ensureGroupDetail(g) {
  if (!rail.details.has(g.slug) && g.current) {
    try {
      const data = await getJSON(
        `/api/projects/${encodeURIComponent(rail.selected)}/artifacts/${encodeURIComponent(g.slug)}`);
      rail.details.set(g.slug, data.group);
    } catch {
      rail.details.set(g.slug, { versions: [], manifest_media: [], exports: [], folder_media: [] });
    }
  }
}

async function openGroupPrimary(g) {
  if (!g.current) return;
  if (rail.filterClass === 'media') {
    await ensureGroupDetail(g);
    const files = detailFilesForFilter(g, rail.details.get(g.slug));
    openFile(rail.selected, files[0] || g.current);
    return;
  }
  openFile(rail.selected, g.current);
}

function renderGroupDetail(host, g) {
  const detail = rail.details.get(g.slug);
  if (!detail) {
    host.append(el('div', { class: 'rail-section-note', text: g.current ? 'loading…' : 'no file yet' }));
    return;
  }
  const files = detailFilesForFilter(g, detail);
  for (const file of files) {
    const tag = file === g.current ? 'current' : typeForPath(file);
    host.append(fileRow(file, { current: file === g.current, tag }));
  }
  if (!files.length) {
    host.append(el('div', { class: 'rail-section-note', text: 'no matching files in group' }));
  }
}

async function toggleUntracked() {
  rail.untrackedOpen = !rail.untrackedOpen;
  if (rail.untrackedOpen && rail.untrackedFiles === null) {
    try {
      const data = await getJSON(
        `/api/projects/${encodeURIComponent(rail.selected)}/artifacts/untracked`);
      rail.untrackedFiles = data.untracked;
    } catch {
      rail.untrackedFiles = [];
    }
  }
  renderArtifacts();
}

// ---------- rail context menus ----------

function groupMenu(e, g) {
  const detail = rail.details.get(g.slug);
  const versions = detail?.versions || [];
  const latest = versions[versions.length - 1] || g.current;
  const previous = versions.length > 1 ? versions[versions.length - 2] : null;
  showMenu(e.clientX, e.clientY, [
    { label: 'Open', action: () => openFile(rail.selected, g.current), disabled: !g.current },
    { label: 'Open to Side', action: () => openFile(rail.selected, g.current, { direction: 'right' }), disabled: !g.current },
    { label: 'Open in New Tab', action: () => openFile(rail.selected, g.current, { forceNew: true }), disabled: !g.current },
    '---',
    { label: 'Open Latest', action: () => openFile(rail.selected, latest), disabled: !latest },
    { label: 'Open Previous Version', action: () => openFile(rail.selected, previous), disabled: !previous },
    { label: 'Compare With Current', action: () => { openFile(rail.selected, g.current); openFile(rail.selected, previous, { direction: 'right' }); }, disabled: !previous || !g.current },
    { label: 'Show Previous Versions', action: () => { if (!rail.expanded.has(g.slug)) toggleGroup(g); } },
    '---',
    { label: 'Reveal File', action: () => reveal(rail.selected, g.current), disabled: !g.current },
    { label: 'Copy Path', action: () => copyPath(rail.selected, g.current), disabled: !g.current },
  ]);
}

function fileMenu(e, file) {
  const dir = file.includes('/') ? file.slice(0, file.lastIndexOf('/')) : '';
  showMenu(e.clientX, e.clientY, [
    { label: 'Open', action: () => openFile(rail.selected, file) },
    { label: 'Open to Side', action: () => openFile(rail.selected, file, { direction: 'right' }) },
    { label: 'Open in New Tab', action: () => openFile(rail.selected, file, { forceNew: true }) },
    '---',
    { label: 'Reveal File', action: () => reveal(rail.selected, file) },
    { label: 'Copy Path', action: () => copyPath(rail.selected, file) },
    '---',
    dir && {
      label: 'Hide From Preview',
      action: async () => {
        if (!window.confirm(`Hide folder "${dir}" from preview?\nWrites a .preview-hide marker file.`)) return;
        await postJSON('/api/hide', { project: rail.selected, dir }).catch(() => {});
        rail.untrackedFiles = null;
        const slug = rail.selected;
        rail.selected = null;
        selectProject(slug);
      },
    },
  ]);
}

// ---------- mount + focus ----------

export async function mountPreview() {
  const area = document.getElementById('editor-area');
  for (const btn of document.querySelectorAll('#artifact-class-filters button')) {
    btn.addEventListener('click', () => setArtifactClass(btn.dataset.class));
  }
  for (const btn of document.querySelectorAll('#artifact-type-filters button')) {
    btn.addEventListener('click', () => setArtifactType(btn.dataset.type));
  }
  dockview = createDockview(area, {
    className: 'dockview-theme-af',
    createComponent: () => new ViewerPanel(),
    createTabComponent: () => new AfTab(),
  });

  dockview.onDidLayoutChange(() => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      try {
        localStorage.setItem(LAYOUT_KEY, JSON.stringify(dockview.toJSON()));
      } catch { /* quota / serialization — persistence is best-effort */ }
    }, 400);
  });

  dockview.onDidActivePanelChange((panel) => {
    const params = panel?.params;
    if (params?.project && params?.file) {
      history.replaceState(null, '',
        `#/preview?project=${encodeURIComponent(params.project)}&file=${encodeURIComponent(params.file)}`);
    }
  });

  const saved = localStorage.getItem(LAYOUT_KEY);
  if (saved) {
    try {
      dockview.fromJSON(JSON.parse(saved));
    } catch {
      localStorage.removeItem(LAYOUT_KEY);
    }
  }

  await renderProjects();
}

export async function focus(project, file) {
  await selectProject(project);
  if (file) openFile(project, file);
}
