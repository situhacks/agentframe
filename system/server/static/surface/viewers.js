// Viewer adapters: markdown, HTML, image/SVG, PDF, video, Office (via
// server-side LibreOffice conversion), and a metadata fallback.

import { postJSON, el } from './api.js';

let markedModule = null;
async function getMarked() {
  if (!markedModule) {
    markedModule = await import('https://cdn.jsdelivr.net/npm/marked@12/+esm');
  }
  return markedModule;
}

function note(html) {
  const div = el('div', { class: 'viewer-note' });
  div.innerHTML = html;
  return div;
}

async function renderMarkdown(meta, body) {
  body.replaceChildren(note('loading&hellip;'));
  try {
    const [text, { marked }] = await Promise.all([
      fetch(meta.url, { cache: 'no-store' }).then((r) => r.text()),
      getMarked(),
    ]);
    const fmMatch = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
    const bodyText = fmMatch ? text.slice(fmMatch[0].length) : text;
    const container = el('div');
    if (fmMatch) {
      const details = el('details', { class: 'md-frontmatter' },
        el('summary', { text: 'frontmatter' }),
        el('pre', { text: fmMatch[1] }));
      container.append(details);
    }
    const doc = el('article', { class: 'md-doc' });
    doc.innerHTML = marked.parse(bodyText);
    container.append(doc);
    body.replaceChildren(container);
  } catch (err) {
    body.replaceChildren(note(`<span class="warn">failed to render markdown:</span> ${err.message}`));
  }
}

async function renderText(meta, body) {
  body.replaceChildren(note('loading&hellip;'));
  try {
    const text = await fetch(meta.url, { cache: 'no-store' }).then((r) => r.text());
    body.replaceChildren(el('pre', { class: 'plain-text', text }));
  } catch (err) {
    body.replaceChildren(note(`<span class="warn">failed to render text:</span> ${err.message}`));
  }
}

function withPdfFragment(url, scale = 0.55) {
  const base = String(url).split('#')[0];
  const zoom = Math.round(scale * 100);
  return `${base}#zoom=${zoom}&pagemode=none`;
}

function renderIframe(meta, body, options = {}) {
  const scale = options.scale || 0.85;
  const stage = el('div', { class: 'iframe-stage' });
  const nativePdfZoom = options.nativePdfZoom || meta.type === 'pdf';
  const frame = el('iframe', { src: nativePdfZoom ? withPdfFragment(meta.url, scale) : meta.url });

  if (nativePdfZoom) {
    stage.classList.add('native-pdf');
    stage.append(frame);
    body.replaceChildren(stage);

    function setScale(nextScale) {
      const clamped = Math.max(0.25, Math.min(1.5, nextScale));
      frame.src = withPdfFragment(meta.url, clamped);
      return clamped;
    }

    return { zoomable: true, setScale };
  }

  const scaled = el('div', { class: 'iframe-scale' });
  scaled.append(frame);
  stage.append(scaled);
  body.replaceChildren(stage);

  function setScale(nextScale) {
    const clamped = Math.max(0.35, Math.min(1.25, nextScale));
    stage.style.setProperty('--viewer-scale', String(clamped));
    return clamped;
  }

  setScale(scale);
  return { zoomable: true, setScale };
}

function renderImage(meta, body) {
  const img = el('img', { src: `${meta.url}?m=${meta.mtime || 0}`, class: 'fit' });
  const wrap = el('div', { class: 'viewer-center' }, img);
  img.addEventListener('click', () => img.classList.toggle('fit'));
  img.style.cursor = 'zoom-in';
  img.addEventListener('click', () => { img.style.cursor = img.classList.contains('fit') ? 'zoom-in' : 'zoom-out'; });
  body.replaceChildren(wrap);
}

function renderVideo(meta, body) {
  body.replaceChildren(el('video', { src: meta.url, controls: '' }));
}

async function renderOffice(meta, body) {
  body.replaceChildren(note(`converting <b>${meta.file}</b> with LibreOffice&hellip;`));
  try {
    const result = await postJSON('/api/convert', { project: meta.project, file: meta.file });
    return renderIframe({ ...meta, url: result.url, type: 'pdf' }, body, { scale: 0.55, nativePdfZoom: true });
  } catch (err) {
    body.replaceChildren(note(
      `<span class="warn">conversion unavailable:</span> ${err.message}<br><br>` +
      `<b>${meta.file}</b><br>size ${formatSize(meta.size)}`));
  }
}

function formatSize(bytes) {
  if (bytes == null) return '?';
  if (bytes > 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
  if (bytes > 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

function renderUnsupported(meta, body) {
  body.replaceChildren(note(
    `<b>${meta.file}</b><br>` +
    `no inline viewer for this file type<br>` +
    `size ${formatSize(meta.size)} — use <b>open original</b> or <b>reveal file</b> in the toolbar`));
}

export async function renderViewer(meta, body) {
  switch (meta.type) {
    case 'markdown': return renderMarkdown(meta, body);
    case 'text': return renderText(meta, body);
    case 'html': return renderIframe(meta, body, { scale: 0.82 });
    case 'pdf': return renderIframe(meta, body, { scale: 0.55, nativePdfZoom: true });
    case 'image': return renderImage(meta, body);
    case 'video': return renderVideo(meta, body);
    case 'office': return renderOffice(meta, body);
    default: return renderUnsupported(meta, body);
  }
}
