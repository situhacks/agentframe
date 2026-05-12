# Auth Handoff Site Index

Use this index to choose the smallest site note for the current auth surface. Match by visible host, app URL, or page copy. If several match, load the most specific note.

| Host or Surface | Site Note |
|---|---|
| `login.microsoftonline.com` | `sites/microsoft-work.md` |
| `login.microsoft.com` | `sites/microsoft-work.md` |
| `login.live.com` | `sites/microsoft-work.md` |
| `outlook.office.com` | `sites/microsoft-work.md` |
| `outlook.cloud.microsoft` | `sites/microsoft-work.md` |
| Deloitte work/school SSO page | `sites/deloitte-sso.md` |
| Deloitte-branded Microsoft prompt | `sites/deloitte-sso.md` |
| `deloittedigitalus.my.workfront.com` | `sites/workfront-login.md` |

If no note matches, use `recipe.md` only and stop earlier at `unknown_auth_state` rather than guessing.
