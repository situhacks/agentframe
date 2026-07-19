"""Marketing domain rules — the domain-owned module the generic spine imports.

The host (system/af.py) imports this when `domain: marketing`, passes a `ctx`
namespace of its stdlib helpers, and calls the hooks below. This module names
nothing outside the marketing pack's concern and never imports af.py (decoupled
via ctx). Stdlib only.

Hooks the host calls:
  on_draft(ctx, cdir, dpath, rel, cfm, parent_row) -> (cfm, notes)
  on_ready(ctx, cdir, dpath, rel, cfm) -> (cfm, notes)  # post-FINAL assembly
  publish(ctx, cdir, args)             -> None           # marketing publish receipt
"""
import os
import re


def _manifest_ingredients(cfm):
    m = re.search(r"^\s*ingredients:\s*\[(.*?)\]\s*$", cfm, re.M)
    if not m:
        return []
    return [i.strip() for i in m.group(1).split(",") if i.strip()]


# Tracker rows the dream pass moved out of project.md. Same row shape as the
# DELIVERABLES block; publish receipts derive totals across both locations.
ARCHIVE_REL = os.path.join("knowledge", "_archive", "deliverables-archive.md")


def _published_posts(ctx, fm):
    return sum(1 for s in ctx.all_rows(fm)
               if re.match(r"post-\d+$", s) and ctx.row_get(fm, s, "status") == "published")


def _archived_published_posts(ctx, cdir):
    p = os.path.join(cdir, ARCHIVE_REL)
    if not os.path.isfile(p):
        return 0
    afm, _ = ctx.split_fm(ctx.read(p), "deliverables-archive.md")
    return _published_posts(ctx, afm)


POST_FINAL_SKELETON = """---
status: drafting
last_updated: {date}
---

# {title} — FINAL

Assembled from the post's ready ingredients per the project manifest.
"""


def _assemble_post_final(ctx, post_dir, ready_path, ingredient):
    pf = os.path.join(post_dir, "post-FINAL.md")
    title = os.path.basename(post_dir)
    if not os.path.isfile(pf):
        ctx.write(pf, POST_FINAL_SKELETON.format(date=ctx.today(), title=title))
    fm, body = ctx.split_fm(ctx.read(pf), "post-FINAL.md")
    _, src_body = ctx.split_fm(ctx.read(ready_path), ready_path)
    fname = os.path.basename(ready_path)
    heading = f"## {ingredient.replace('-', ' ').title()} (ready from {fname})"
    section = f"{heading}\n{src_body.strip()}\n"
    old = re.search(rf"^## .*\(ready from {re.escape(ingredient)}-v\d+\.md\)\n.*?(?=^## |\Z)", body, re.M | re.S)
    if old:
        body = body[:old.start()] + section + "\n" + body[old.end():]
    else:
        body = body.rstrip("\n") + "\n\n" + section
    fm = ctx.set_scalar(fm, "last_updated", ctx.today(), "post-FINAL.md")
    ctx.write(pf, ctx.join_fm(fm, body))
    return pf


def on_draft(ctx, cdir, dpath, rel, cfm, parent_row):
    """Create the post assembly record when its first ingredient starts."""
    notes = []
    norm = rel.replace("\\", "/")
    if not re.search(r"(^|/)posts/", norm):
        return cfm, notes
    post_dir = os.path.dirname(dpath)
    pf = os.path.join(post_dir, "post-FINAL.md")
    if not os.path.isfile(pf):
        title = os.path.basename(post_dir)
        ctx.write(pf, POST_FINAL_SKELETON.format(date=ctx.today(), title=title))
        notes.append("post-FINAL.md created")
    if ctx.row_span(cfm, parent_row):
        pf_rel = os.path.relpath(pf, cdir).replace("\\", "/")
        cfm = ctx.row_set(cfm, parent_row, "file", pf_rel)
    return cfm, notes


def _post_complete(ctx, post_dir, ingredients, current_ready_path=None):
    for ing in ingredients:
        ns = ctx.versions_in(post_dir, ing)
        if not ns:
            return False
        best = os.path.join(post_dir, f"{ing}-v{max(ns)}.md")
        if current_ready_path and os.path.abspath(best) == os.path.abspath(current_ready_path):
            continue  # cmd_ready commits this head immediately after the hook returns
        if ctx.get_scalar(ctx.split_fm(ctx.read(best), best)[0], "status") != "ready":
            return False
    return True


def on_ready(ctx, cdir, dpath, rel, cfm):
    """Post-step after a deliverable is ready: assemble the post-FINAL record
    when the ready file is a post ingredient. Returns (cfm, notes)."""
    notes = []
    norm = rel.replace("\\", "/")
    if not re.search(r"(^|/)posts/", norm):
        return cfm, notes
    post_dir = os.path.dirname(dpath)
    match = re.match(r"(.+)-v\d+\.md$", os.path.basename(dpath))
    if not match:
        if os.path.basename(dpath) == "post-FINAL.md":
            ings = _manifest_ingredients(cfm)
            if ings and not _post_complete(ctx, post_dir, ings):
                ctx.die(f"{rel}: ready requires every manifest ingredient to be ready")
        return cfm, notes
    ing = match.group(1)
    _assemble_post_final(ctx, post_dir, dpath, ing)
    notes.append("post-FINAL.md updated")
    ings = _manifest_ingredients(cfm)
    if ings and _post_complete(ctx, post_dir, ings, current_ready_path=dpath):
        pf = os.path.join(post_dir, "post-FINAL.md")
        pfm, pbody = ctx.split_fm(ctx.read(pf), "post-FINAL.md")
        pfm = ctx.set_scalar(pfm, "status", "ready", "post-FINAL.md")
        pfm = ctx.set_scalar(pfm, "last_updated", ctx.today(), "post-FINAL.md")
        ctx.write(pf, ctx.join_fm(pfm, pbody))
        post_slug = os.path.basename(post_dir)
        if ctx.row_span(cfm, post_slug):
            cfm = ctx.row_set(cfm, post_slug, "status", "ready")
            cfm = ctx.row_set(cfm, post_slug, "last_updated", ctx.today())
        notes.append("all manifest ingredients ready — post-FINAL ready")
    return cfm, notes


def handles_publish(ctx, cdir, args):
    """True only for marketing post assembly rows; other deliverables use the shared publisher."""
    cpath = os.path.join(cdir, "project.md")
    cfm, _ = ctx.split_fm(ctx.read(cpath), "project.md")
    target = args.deliverable.replace("\\", "/")
    rel = target if "/" in target else ctx.row_get(cfm, target, "file")
    return bool(rel and rel.endswith("post-FINAL.md"))


def publish(ctx, cdir, args):
    """Add the marketing receipt while applying the shared published state."""
    cpath = os.path.join(cdir, "project.md")
    cfm, cbody = ctx.split_fm(ctx.read(cpath), "project.md")
    target = args.deliverable.replace("\\", "/")
    if "/" in target:
        rel = target
        rows = [row for row in ctx.all_rows(cfm) if ctx.row_get(cfm, row, "file") == rel]
        post = rows[0] if len(rows) == 1 else None
        post or ctx.die(f"no unique tracker row points at '{rel}'")
    else:
        post = target
        rel = ctx.row_get(cfm, post, "file") or ctx.die(f"tracker row '{post}' not found or has no file")
    if not rel.endswith("post-FINAL.md"):
        ctx.die(f"row '{post}' points at {rel}, not a post-FINAL.md — publish operates on the assembly record")
    pf = os.path.join(cdir, rel)
    pfm, pbody = ctx.split_fm(ctx.read(pf), rel)
    status = ctx.get_scalar(pfm, "status")
    if status != "ready":
        ctx.die(f"{rel}: publish requires status ready (found {status or 'missing'}); run af ready first")
    if not args.url:
        ctx.die("marketing publish requires --url so the live receipt is reconstructable")

    posted = args.posted_at or ctx.now_iso()
    platform = args.platform or "linkedin"
    pfm = ctx.set_scalar(pfm, "status", "published", rel)
    pfm = ctx.set_scalar(pfm, "last_updated", ctx.today(), rel)
    pfm = re.sub(r"\n(shipped_at:.*|published:(\n  .*)*|shipped_media:(\n  - .*)*)", "", pfm)
    block = [f"shipped_at: {ctx.today()}", "published:", f"  platform: {platform}",
             f"  url: {args.url}", f"  posted_at: {posted}"]
    if args.media:
        block.append("shipped_media:")
        block += [f"  - {m}" for m in args.media]
    pfm = pfm.rstrip("\n") + "\n" + "\n".join(block)
    ctx.write(pf, ctx.join_fm(pfm, pbody))

    cfm = ctx.row_set(cfm, post, "status", "published")
    cfm = ctx.row_set(cfm, post, "last_updated", ctx.today())
    published = _published_posts(ctx, cfm) + _archived_published_posts(ctx, cdir)
    if ctx.get_scalar(cfm, "shipped_at") in (None, "null", ""):
        cfm = ctx.upsert_scalar(cfm, "shipped_at", ctx.today())
    cfm = ctx.touch_lifecycle(cfm)
    ctx.write(cpath, ctx.join_fm(cfm, cbody))
    ctx.append_activity(cdir, f"post_published: {post} → {args.url}")

    print(f"af publish: {post} -> published ({args.url}); published posts={published}")
    print("\nJudgment checklist (agent + operator):")
    print("  [ ] Published copy reconciled before this immutable receipt: if the live post differs")
    print("      materially, create a new tracked edition rather than mutating this published record")
    print("  [ ] shipped_media recorded for every asset that actually shipped (--media)")
    print("  [ ] Performance capture scheduled (~14 days after posted_at, per composio-notes.md)")
