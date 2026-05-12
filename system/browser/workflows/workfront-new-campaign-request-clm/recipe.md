# Workfront New Campaign Request - CLM

workflow_id: workfront-new-campaign-request-clm
status: cursor_replay
app_url: https://deloittedigitalus.my.workfront.com/home/workspaces
approval_mode: human_review

## Job

Create a draft Workfront `New Campaign Request - CLM` request from any campaign or post context. Fill every field Cursor can infer from the source files. Leave non-inferable fields blank. Stop before `Submit` and report the outstanding fields so the marketer can review, finish, and submit manually.

## Source Context

Read the campaign and deliverable files supplied by the operator before opening Workfront. For AgentFrame Marketing posts, useful sources usually include:

- `workspace/campaigns/{campaign}/campaign.md` frontmatter for campaign name, post job, phase, and status.
- The target post's `copy-vF.md` for final copy, objective, CTA, and platform.
- The target post's `carousel-spec-vF.md`, `video-spec-vF.md`, or equivalent production artifact for format, asset requirements, and launch context.

Do not treat this workflow as Post 5-specific. Post 5 was only the demonstration case.

## Tool Routing

Use browser-harness in the controlled Edge Work Browser. Cursor remains the reasoning layer. Routine run history does not belong in this recipe.

## Inference Policy

Fill only values that can be inferred with reasonable confidence from the source context or stable workflow defaults.

- Request subject: infer from campaign/post name and asset type.
- Owner: use the operator if already prefilled or obvious from the Workfront session.
- Campaign type: infer from the campaign's goal and post role when possible. If uncertain, leave blank.
- Campaign description: summarize the post/campaign request in marketer-ready prose using the source files.
- Priority: use `Normal` unless source context says urgent.
- Feasibility: use `Medium` unless production complexity is clearly low or high.
- Customer Impact: use `Medium` unless the source clearly indicates a high-impact launch or a low-risk/internal request.
- Marketing Priority: infer from campaign goals. For inbound/thought-leadership/demand creation, `Attract New Customers` is usually the best fit.
- Go-Live Date, Campaign End Date, Budget: leave blank unless explicitly provided by the operator or source files.

If a required Workfront field cannot be inferred, stop and ask the operator in chat only if the run is explicitly approved for autonomous submission. In default human-review mode, leave the field blank and include it in the outstanding-fields note.

## Date And Budget Policy

Dates are not dropdown decisions. They are commitment fields, so do not invent them from "today" or from the demo's random selections.

- Go-Live Date: fill only when the source context has an explicit publish date, launch date, go-live date, scheduled date, or operator-provided target date.
- Campaign End Date: fill only when the source context has an explicit campaign end date, promotion end date, deadline window, or operator-provided end date. Do not auto-create a one-week campaign window for a single post.
- Already-published content: if the request is meant to represent work already published and the source has a reliable `published.posted_at`, use that as Go-Live Date only if the operator is asking for retroactive intake capture. Otherwise leave date fields blank.
- Budget: leave blank unless a budget is explicitly provided. Do not enter `$0` unless the operator says the request is zero-budget or Workfront requires a numeric value before draft save.

In human-review mode, blank date/budget fields should be reported as outstanding for the marketer. In autonomous mode, ask the operator before submitting if any of these fields are required.

## Dropdown Inventory And Selection Rules

Do not blindly reuse the demo selections. Open each dropdown, inspect the visible options, then choose the best option using the field title, the available choices, and the source campaign/post context. A frontier model should infer the fit from the request's actual marketing intent rather than follow rigid per-option rules.

### Campaign Type

Observed options:

- `Customer Acquisition`
- `Customer Retention`
- `Others`

### Priority

Observed options:

- `Critical`
- `Low`
- `Normal`
- `High`
- `Urgent`

### Feasibility

Observed options:

- `High`
- `Medium`
- `Low`

### Customer Impact

Observed options:

- `High`
- `Medium`
- `Low`

### Marketing Priority

Observed options:

- `Attract New Customers`
- `Build The Brand`
- `Deepen Customer Relationships`

## Demonstrated Path

1. Open or reuse Work Browser Mode at `https://deloittedigitalus.my.workfront.com/home/workspaces`.
2. Observe/refresh once to check whether Workfront is already ready. If not authenticated, run `workflows/_shared/auth-handoff/recipe.md` with `sites/workfront-login.md` and return here only when the Workfront home page or Requests UI is ready.
3. Click `Main Menu`.
4. Click `Requests`.
5. Click `New request`.
6. In `Select a Request Type`, choose `New Campaign Request - CLM`.
7. Fill `Type request subject` from source context.
8. Confirm or select the owner if Workfront requires it.
9. Fill the `New Campaign Request - Delta` custom form fields using the inference policy.
10. Do not upload files unless the operator explicitly asks. File upload is outside the default recipe.
11. Stop before `Submit`.
12. Report: filled fields, outstanding blank fields, and "Please review and submit when done."

## Known Controls

- `Main Menu`: `button[data-testid="global-nav-drawer-toggle"]`
- `Requests`: `a[data-testid="request.plural"]`
- `New request`: `button[data-testid="new-request-button"]`
- `Select a Request Type`: `input[data-testid="queue-select-input"]`
- `New Campaign Request - CLM`: `div[data-testid="request-queue-option"]`
- Request subject: `input[data-testid="phoenix-input-name"]`
- Owner: `input[data-testid="phoenix-input-owner"]`
- Campaign Type: `input[data-testid="DE:Campaign type - SP-input"]`
- Campaign Description: `textarea[data-testid="DE:Campaign Description"]`
- Go-Live Date input: `input[data-testid="DE:Go-Live Date-date-input-input"]`
- Campaign End Date input: `input[data-testid="DE:Campaign End Date-date-time-input-input"]`
- Budget: `input[data-testid="DE:Budget-input"]`
- Priority: `input[data-testid="DE:Priority-input"]`
- Feasibility: `input[data-testid="DE:Feasibility_delta-input"]`
- Customer Impact: `input[data-testid="DE:Customer Impact - Delta-input"]`
- Marketing Priority: `input[data-testid="DE:Marketing Priority - Delta-input"]`
- Submit: `button[data-testid="request-submit-button"]`

Date controls were demonstrated through date-picker clicks, but dates are usually non-inferable. Leave them blank unless the operator provides dates.

When filling dates, target the field-specific input or date-picker wrapper. Do not use a generic `Open date picker` locator by label alone: Go-Live Date and Campaign End Date both expose the same visible button label. A generic locator can reopen the first date picker and fail to set Campaign End Date.

- For Go-Live Date, use `input[data-testid="DE:Go-Live Date-date-input-input"]` or the date trigger inside `field-DE:Go-Live Date`.
- For Campaign End Date, use `input[data-testid="DE:Campaign End Date-date-time-input-input"]` or the date trigger inside `field-DE:Campaign End Date`.
- After setting both dates, observe/read the form and verify the visible input values before submitting.

Dropdown option labels can remain mounted but hidden after prior selections. When selecting repeated labels like `Medium`, prefer the visible option inside the currently open dropdown, or use the demonstrated option selectors only after confirming the right dropdown is open.

## Human Gate

Default behavior is draft-only. Cursor must stop before `Submit` unless the operator explicitly authorizes autonomous submission for the current run or this workflow is later promoted and whitelisted.

If autonomous submission is authorized, Cursor must first ask the operator for any required fields it cannot infer, then execute the workflow, observe the completed request, and report the request name/status.

## Promotion Notes

Promotion is not earned yet. Keep Cursor in the loop while field inference is still being learned across campaign types. The recipe remains the source of truth for workflow memory and fallback when replay hits auth, selector drift, option drift, or missing input.
