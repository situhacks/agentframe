# Person profile

Shapes `workspace/life/people/<slug>.md` for anyone he knows (a work contact, a friend, an acquaintance) and `workspace/life/people/groups/<slug>.md` for a friend group. Every file is `private: true`. A work contact is promoted here at project closeout ([`project-flow.md`](../process/project-flow.md)); a friend gets a thin page the first time he names them. `af person add <slug> --name <n> [--group] [--met <where>] [--tag <t>]...` writes either shape and refuses a slug already used under `places/` or `people/`.

## Person

```yaml
---
private: true
name: <full name, or the name he uses>
type: person
title: <job title / organization>                 # work contacts
email: <address>                                   # work contacts
met: <where and when, e.g. "a friend's birthday, 2026-09">   # friends
groups: [<group slug>]                             # friends
profiles: [instagram @handle, linkedin <url>]      # public profiles only
aliases: [<other spellings>]
tags: [<interest or closeness ids>]                # registry ids only; af tag match first
---
```

`tags` carries ids from the registry ([`tags.md`](tags.md)), the people facets only (an interest, a context, a closeness): `af tag match "<phrase>" --facet <f>` per candidate and write what it returns; on `none`, `af tag propose <slug> --facet <f> --scope "<one line>"`. Nothing a field already holds (`groups`, `met`, `title`) is a tag; `af search --tag <id> --type person` filters on them.

Sections, present only when there is something to say:

- **Global Identity & Relationship** (work contacts): background, preferences, communication style, ongoing context.
- **Engagement History** (work contacts): dated roles and links to per-project overlays, `{YYYY-MM-DD} — {Project Name} (Role: {Role})` -> `workspace/projects/{slug}/knowledge/people/{person-slug}.md`.
- **What they do**, **Interests**, **How we met**, **Plans and places**, **Notes** (friends). Plans and places holds plans not yet a visit row; past outings live only as visit rows on place pages ([`place.md`](place.md)), found with `grep -r "[[<slug>]]" workspace/life/places/`.

## Group

```yaml
---
private: true
name: <the chat's name>
type: group
formed: <how, e.g. "a climbing class, 2026-09">
members: [<person slugs>]
---
```

Sections: **Members** (one line each, `[[slug]]` and what he knows), **Habits** (what they do together, how bills split, who organises), **Forming** (plans under discussion: what, when, where, who is in; a line is replaced when the plan settles into a visit row or dies). Outings are not copied here: grep `workspace/life/places/` for the group's slug.

## The line

- Sources: what he says, what a person states about themselves in a group chat, the person's own public profiles, and research he agrees with, cited by handle or source; never monitored.
- Record every fact from those sources, including home address, family, background, health, and sexuality, the same turn it comes up. More notes beat fewer: he corrects what is wrong, and a correction replaces the line. Health and sexuality come from what he says or agrees with, or what the person shows, never from your own guess. A work contact's facts still go to the project first, per [`knowledge-base.md`](../process/knowledge-base.md). A friend's words are summarised, never quoted at length; group-chat messages are never filed verbatim.
- A group chat is read only when he asks, as a normal page read of the recent window (scroll a few times, then read the page text); never a scripted bulk extraction, never on a schedule or at catch-up.
- `private: true` on every file: nothing from a person or group page goes into a tracked file.
