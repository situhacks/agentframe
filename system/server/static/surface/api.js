// Thin fetch helpers for the surface API.

export async function getJSON(path) {
  const resp = await fetch(path, { cache: 'no-store' });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw Object.assign(new Error(body.error || `${resp.status} on ${path}`), { status: resp.status });
  }
  return resp.json();
}

export async function postJSON(path, payload) {
  const resp = await fetch(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw Object.assign(new Error(body.error || `${resp.status} on ${path}`), { status: resp.status });
  }
  return resp.json();
}

// Deterministic keycap identity: initials + sampled Modern Dolch bucket.
const KEYCAP_HASH_SALT = 'modol'; // Keeps current five active projects spread across grey/teal/pink buckets.

export function keycap(slug, name) {
  const words = String(slug || name || '??').split(/[-_\s]+/).filter(Boolean);
  let initials;
  if (words.length >= 2) initials = words[0][0] + words[1][0];
  else initials = (words[0] || '??').slice(0, 2);
  let hash = 0;
  for (const ch of String(slug) + KEYCAP_HASH_SALT) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  return { initials: initials.toUpperCase(), colorClass: `c${hash % 4}` };
}

export function keycapEl(slug, name) {
  const { initials, colorClass } = keycap(slug, name);
  const el = document.createElement('span');
  el.className = `keycap ${colorClass}`;
  el.textContent = initials;
  el.title = name || slug;
  return el;
}

export function basename(path) {
  return String(path).split('/').pop();
}

export function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (key === 'class') node.className = value;
    else if (key === 'text') node.textContent = value;
    else if (key.startsWith('on')) node.addEventListener(key.slice(2), value);
    else if (value !== null && value !== undefined) node.setAttribute(key, value);
  }
  for (const child of children) {
    if (child === null || child === undefined) continue;
    node.append(child);
  }
  return node;
}
