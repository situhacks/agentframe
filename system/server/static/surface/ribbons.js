// Shared project-ribbon renderer — the visual contract is the approved mockup:
// each project intersecting the window gets one 20px row; a flat ~13% tint
// ghost track spans its life inside the window; hard solid segments (child
// spans, never gradients) mark days with logged work; deliverables cluster
// into white dots with a count badge sitting ON the ribbon; future
// commitments render as hollow squares. Popovers are one body-level floating
// card (position: fixed) so they never clip inside scroll containers or
// spawn scrollbars. Used by the week/day header strip, every month week-row,
// and the timeline.

import { el, keycapEl, navigate } from './api.js?v=5';

export const DAY_MS = 86400000;

export function colorFor(slug) {
  let hash = 0;
  for (const ch of String(slug)) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 45% 45%)`;
}

export function ghostTint(color) {
  return `color-mix(in srgb, ${color} 13%, transparent)`;
}

export function plainLabel(value) {
  return String(value || 'event').replace(/[-_]+/g, ' ').trim();
}

function dayOf(value) {
  const text = String(value || '').slice(0, 10);
  return /^\d{4}-\d{2}-\d{2}$/.test(text) ? text : null;
}

function dateUtc(dayStr) {
  return new Date(`${dayStr}T00:00:00Z`);
}

export function todayIso() {
  const now = new Date();
  return new Date(Date.UTC(now.getFullYear(), now.getMonth(), now.getDate())).toISOString().slice(0, 10);
}

// ---- floating popover card: one per page, fixed-position, hover-bridging ----

let floatCard = null;
let hideTimer = null;

function card() {
  if (floatCard) return floatCard;
  floatCard = el('div', { class: 'float-card' });
  floatCard.hidden = true;
  floatCard.addEventListener('mouseenter', () => clearTimeout(hideTimer));
  floatCard.addEventListener('mouseleave', hideCardSoon);
  document.body.append(floatCard);
  return floatCard;
}

function hideCardSoon() {
  clearTimeout(hideTimer);
  hideTimer = setTimeout(() => { if (floatCard) floatCard.hidden = true; }, 180);
}

function showCard(target, nodes) {
  clearTimeout(hideTimer);
  const c = card();
  c.replaceChildren(...nodes);
  c.hidden = false;
  const r = target.getBoundingClientRect();
  c.style.left = '0px';
  c.style.top = '0px';
  const cw = c.offsetWidth, ch = c.offsetHeight;
  let left = r.left + r.width / 2 - cw / 2;
  left = Math.max(8, Math.min(left, window.innerWidth - cw - 8));
  let top = r.bottom + 6;
  if (top + ch > window.innerHeight - 8) top = r.top - ch - 6;
  c.style.left = `${Math.round(left)}px`;
  c.style.top = `${Math.round(top)}px`;
}

// Attach a floating popover to `target`. `buildContent` returns child nodes.
export function attachPopover(target, buildContent) {
  target.addEventListener('mouseenter', () => showCard(target, buildContent()));
  target.addEventListener('mouseleave', hideCardSoon);
  target.addEventListener('focus', () => showCard(target, buildContent()));
  target.addEventListener('blur', hideCardSoon);
}

export function popContent(title, lines) {
  const nodes = [el('strong', { text: title })];
  for (const line of lines) {
    const row = el('span', { class: 'pop-line', text: line.text });
    if (line.onClick) {
      row.classList.add('pop-link');
      row.addEventListener('click', (e) => {
        e.stopPropagation();
        if (floatCard) floatCard.hidden = true;
        line.onClick();
      });
    }
    nodes.push(row);
  }
  return nodes;
}

// One clustered deliverable dot (white circle, project-color border, count badge).
export function deliverableDot(project, dayStr, items, leftPct, color) {
  const dot = el('button', {
    class: 'rib-dot',
    style: `left:calc(${leftPct}% - 7px);border-color:${color}`,
    'aria-label': `${items.length} deliverable(s) on ${dayStr}`,
  }, el('span', { text: items.length > 1 ? String(items.length) : '' }));
  attachPopover(dot, () => popContent(
    items.length > 1 ? `${items.length} deliverables · ${dayStr}` : `${plainLabel(items[0].slug)} · ${dayStr}`,
    items.map((item) => ({
      text: `● ${plainLabel(item.slug)}`,
      onClick: item.file ? () => navigate('preview', { project: project.slug, file: item.file }) : null,
    })),
  ));
  return dot;
}

// Render ribbon rows for every project intersecting [windowStart, windowEnd).
// windowStart/windowEnd: 'YYYY-MM-DD' (end exclusive). opts: {ghost, expandLabels, showFuture}.
export function renderRibbons(host, projects, windowStart, windowEnd, opts) {
  host.replaceChildren();
  const w0 = dateUtc(windowStart).getTime();
  const w1 = dateUtc(windowEnd).getTime();
  const today = todayIso();

  for (const project of projects) {
    const color = colorFor(project.slug);
    const created = dayOf(project.created_at) || dayOf(project.last_activity);
    if (!created) continue;
    const closed = dayOf(project.completed_at) || dayOf(project.cancelled_at);
    const projStart = dateUtc(created).getTime();
    const projEnd = (closed ? dateUtc(closed).getTime() : dateUtc(today).getTime()) + DAY_MS; // inclusive end day
    if (projStart >= w1 || projEnd <= w0) continue;

    const visStart = Math.max(projStart, w0);
    const visEnd = Math.min(projEnd, w1);
    const leftPct = ((visStart - w0) / (w1 - w0)) * 100;
    const widthPct = ((visEnd - visStart) / (w1 - w0)) * 100;
    const visDays = Math.max(1, Math.round((visEnd - visStart) / DAY_MS));

    const ribbon = el('div', {
      class: `rib${project.status === 'active' ? '' : ' done'}`,
      style: `margin-left:${leftPct}%;width:${widthPct}%;--pc:${color};background:${opts.ghost ? ghostTint(color) : color}`,
    });

    // hard solid segments for worked days (ghost mode only — solid mode is already full)
    const workedSet = new Set(project.worked_days || []);
    let labelOnSolid = !opts.ghost;
    if (opts.ghost) {
      for (const d of workedSet) {
        const t = dateUtc(d).getTime();
        if (t < visStart || t >= visEnd) continue;
        const segLeft = ((t - visStart) / (visEnd - visStart)) * 100;
        ribbon.append(el('span', {
          class: 'rib-seg',
          style: `left:${segLeft}%;width:${100 / visDays}%;background:${color}`,
        }));
        if (t === visStart) labelOnSolid = true;
      }
    }

    ribbon.append(el('span', { class: 'rib-name' },
      keycapEl(project.slug, project.name),
      el('span', {
        class: 'rib-name-text',
        style: `color:${labelOnSolid ? '#fff' : color}`,
        text: `${project.name || project.slug}${project.status === 'active' ? '' : ' ✓'}`,
      })));

    // clustered deliverable dots on the ribbon
    const byDay = {};
    for (const item of project.deliverables || []) {
      const d = dayOf(item.last_updated);
      if (!d) continue;
      const t = dateUtc(d).getTime();
      if (t < visStart || t >= visEnd) continue;
      (byDay[d] = byDay[d] || []).push(item);
    }
    for (const [d, items] of Object.entries(byDay)) {
      const t = dateUtc(d).getTime();
      const center = ((t + DAY_MS / 2 - visStart) / (visEnd - visStart)) * 100;
      ribbon.append(deliverableDot(project, d, items, center, color));
      if (opts.expandLabels) {
        ribbon.append(el('span', {
          class: 'rib-inline-label',
          style: `left:calc(${center}% + 10px);color:${color}`,
          text: items.length > 1 ? `${items.length} deliverables` : plainLabel(items[0].slug),
        }));
      }
    }

    // future commitments as hollow squares, with a dashed ghost continuation
    // from the ribbon's end so the square reads as part of this project's row
    if (opts.showFuture) {
      let maxFuture = 0;
      for (const item of project.attention || []) {
        const d = dayOf(item.date);
        if (!d || d <= today) continue;
        const t = dateUtc(d).getTime();
        if (t < w0 || t >= w1) continue;
        maxFuture = Math.max(maxFuture, t + DAY_MS);
        const center = ((t + DAY_MS / 2 - visStart) / (visEnd - visStart)) * 100;
        const sq = el('button', {
          class: 'rib-future',
          style: `left:calc(${center}% - 5px)`,
          'aria-label': `${item.kind || 'future commitment'}, ${d}`,
        });
        attachPopover(sq, () => popContent(
          `${plainLabel(item.kind || 'future commitment')} · ${d}`,
          [{ text: String(item.text || '') }],
        ));
        ribbon.append(sq);
      }
      if (maxFuture > visEnd) {
        const extWidth = ((Math.min(maxFuture, w1) - visEnd) / (visEnd - visStart)) * 100;
        ribbon.append(el('span', {
          class: 'rib-ext',
          style: `left:100%;width:${extWidth}%;border-color:${color}`,
        }));
      }
    }

    host.append(ribbon);
  }
}
