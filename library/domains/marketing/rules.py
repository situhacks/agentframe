"""Marketing domain rules — the domain-owned module the generic spine imports.

The host (system/af.py) imports this when `domain: marketing`, passes a `ctx`
namespace of its stdlib helpers, and calls the hooks below. This module names
nothing outside the marketing pack's concern and never imports af.py (decoupled
via ctx). Stdlib only.

Hooks the host calls:
  on_lock(ctx, cdir, dpath, rel, cfm) -> (cfm, notes)   # post-FINAL assembly
  check(ctx, cdir, cfm)               -> [issue, ...]    # post-counting reconciliation
  publish(ctx, cdir, args)            -> None            # the publish verb (marketing-only)
"""
import os
import re


POST_FINAL_SKELETON = """---
status: drafting
last_updated: {date}
---

# {title} — FINAL

Assembled from the post's locked ingredients per the project manifest.
"""


def _assemble_post_final(ctx, post_dir, locked_path, ingredient):
    pf = os.path.join(post_dir, "post-FINAL.md")
    title = os.path.basename(post_dir)
    if not os.path.isfile(pf):
        ctx.write(pf, POST_FINAL_SKELETON.format(date=ctx.today(), title=title))
    fm, body = ctx.split_fm(ctx.read(pf), "post-FINAL.md")
    _, src_body = ctx.split_fm(ctx.read(locked_path), locked_path)
    fname = os.path.basename(locked_path)
    heading = f"## {ingredient.replace('-', ' ').title()} (locked from {fname})"
    section = f"{heading}\n{src_body.strip()}\n"
    old = re.search(rf"^## .*\(locked from {re.escape(ingredient)}-v\d+\.md\)\n.*?(?=^## |\Z)", body, re.M | re.S)
    if old:
        body = body[:old.start()] + section + "\n" + body[old.end():]
    else:
        body = body.rstrip("\n") + "\n\n" + section
    fm = ctx.set_scalar(fm, "last_updated", ctx.today(), "post-FINAL.md")
    ctx.write(pf, ctx.join_fm(fm, body))
    return pf


def _post_complete(ctx, post_dir, ingredients):
    for ing in ingredients:
        ns = ctx.versions_in(post_dir, ing)
        if not ns:
            return False
        best = os.path.join(post_dir, f"{ing}-v{max(ns)}.md")
        if ctx.get_scalar(ctx.split_fm(ctx.read(best), best)[0], "status") != "locked":
            return False
    return True


def on_lock(ctx, cdir, dpath, rel, cfm):
    """Post-step after a deliverable locks: assemble the post-FINAL record when
    the locked file is a post ingredient. Returns (cfm, notes)."""
    notes = []
    norm = rel.replace("\\", "/")
    if not re.search(r"(^|/)posts/", norm):
        return cfm, notes
    post_dir = os.path.dirname(dpath)
    ing = re.match(r"(.+)-v\d+\.md$", os.path.basename(dpath)).group(1)
    _assemble_post_final(ctx, post_dir, dpath, ing)
    notes.append("post-FINAL.md updated")
    ings = ctx.manifest_ingredients(cfm)
    if ings and _post_complete(ctx, post_dir, ings):
        pf = os.path.join(post_dir, "post-FINAL.md")
        pfm, pbody = ctx.split_fm(ctx.read(pf), "post-FINAL.md")
        pfm = ctx.set_scalar(pfm, "status", "locked", "post-FINAL.md")
        pfm = ctx.set_scalar(pfm, "last_updated", ctx.today(), "post-FINAL.md")
        ctx.write(pf, ctx.join_fm(pfm, pbody))
        post_slug = os.path.basename(post_dir)
        if ctx.row_span(cfm, post_slug):
            cfm = ctx.row_set(cfm, post_slug, "status", "locked")
            cfm = ctx.row_set(cfm, post_slug, "last_updated", ctx.today())
        notes.append("all manifest ingredients locked — post-FINAL locked")
    return cfm, notes


def check(ctx, cdir, cfm):
    """Domain doctor checks: posts_published reconciliation."""
    issues = []
    rel = os.path.relpath(cdir, ctx.ROOT).replace("\\", "/")
    delivered_posts = sum(1 for s in ctx.all_rows(cfm)
                          if re.match(r"post-\d+$", s) and ctx.row_get(cfm, s, "status") == "delivered")
    declared = ctx.get_scalar(cfm, "posts_published")
    if declared is not None and declared.isdigit() and int(declared) != delivered_posts:
        issues.append(f"{rel}: posts_published={declared} but {delivered_posts} post rows are delivered")
    return issues


def publish(ctx, cdir, args):
    """The publish verb (marketing-only): record a post's delivered state."""
    cpath = os.path.join(cdir, "project.md")
    cfm, cbody = ctx.split_fm(ctx.read(cpath), "project.md")
    rel = ctx.row_get(cfm, args.post, "file") or ctx.die(f"tracker row '{args.post}' not found or has no file")
    if not rel.endswith("post-FINAL.md"):
        ctx.die(f"row '{args.post}' points at {rel}, not a post-FINAL.md — publish operates on the assembly record")
    pf = os.path.join(cdir, rel)
    pfm, pbody = ctx.split_fm(ctx.read(pf), rel)

    posted = args.posted_at or ctx.now_iso()
    pfm = ctx.set_scalar(pfm, "status", "delivered", rel)
    pfm = ctx.set_scalar(pfm, "last_updated", ctx.today(), rel)
    pfm = re.sub(r"\n(shipped_at:.*|published:(\n  .*)*|shipped_media:(\n  - .*)*)", "", pfm)
    block = [f"shipped_at: {ctx.today()}", "published:", f"  platform: {args.platform}",
             f"  url: {args.url}", f"  posted_at: {posted}"]
    if args.media:
        block.append("shipped_media:")
        block += [f"  - {m}" for m in args.media]
    pfm = pfm.rstrip("\n") + "\n" + "\n".join(block)
    ctx.write(pf, ctx.join_fm(pfm, pbody))

    cfm = ctx.row_set(cfm, args.post, "status", "delivered")
    cfm = ctx.row_set(cfm, args.post, "last_updated", ctx.today())
    delivered = sum(1 for s in ctx.all_rows(cfm)
                    if re.match(r"post-\d+$", s) and ctx.row_get(cfm, s, "status") == "delivered")
    cfm = ctx.set_scalar(cfm, "posts_published", str(delivered), "project.md")
    if ctx.get_scalar(cfm, "shipped_at") in (None, "null", ""):
        cfm = ctx.set_scalar(cfm, "shipped_at", ctx.today(), "project.md")
    cfm = ctx.touch_lifecycle(cfm)
    ctx.write(cpath, ctx.join_fm(cfm, cbody))
    ctx.append_activity(cdir, f"post_published: {args.post} → {args.url}")

    print(f"af publish: {args.post} -> delivered ({args.url}); posts_published={delivered}")
    print("\nJudgment checklist (agent + operator):")
    print("  [ ] Shipped copy reconciled: if the live post differs materially from the locked")
    print("      ingredient, write the next ingredient version with the as-shipped text, re-lock,")
    print("      refresh its post-FINAL section — and run the voice publish/back-fill fallback")
    print("  [ ] shipped_media recorded for every asset that actually shipped (--media)")
    print("  [ ] Performance capture scheduled (~14 days after posted_at, per composio-notes.md)")
