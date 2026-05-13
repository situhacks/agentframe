---
status: locked
last_updated: 2026-05-11T19:07:00-07:00
scope: copy-image-production
families:
  - post-copy
  - image-production
---

# Template Evolution Retro: Copy + Image Production

## Recommended Changes

### Clarify Image Production Records For Non-Model Paths

- **Action:** Keep as no-patch for this example.
- **Why:** The final image was an operator-cleaned asset, and the campaign record now names the final asset directly. That is enough for the example folder.
- **Target change:** No template change.
- **Current state:** no patch

### Make Single-Image Lock Checks First-Class In Post Copy

- **Action:** Keep as no-patch for this example.
- **Why:** The post copy, image prompt, and final asset are all locked and cross-referenced. The example demonstrates the intended behaviour without needing a template patch.
- **Target change:** No template change.
- **Current state:** no patch

## Already Applied

### Humanizer Gate Caught The Right Problem

- **Action:** Keep the existing humanizer gate.
- **Why:** The v1 to vF copy movement was mostly length, AI-polish, and sentence rhythm. The template already requires the humanizer pass, and the campaign activity shows it ran before lock.
- **Target change:** No template change.
- **Current state:** applied

## No Patch Needed

### Hook Variants Were Sufficiently Covered

- **Action:** Reject a new hook-options section.
- **Why:** The post-copy template already requires two to three hook variants and a recommended pick.
- **Target change:** No template change.
- **Current state:** no patch

### Paragraph Flow Was Execution, Not Template Shape

- **Action:** Do not patch the template for paragraph grouping.
- **Why:** The operator's feedback that one draft felt too short and jumpy was a writing-quality issue. The template already says the body should be flowing prose with no bullets.
- **Target change:** No template change.
- **Current state:** no patch

No template patches were applied from this example campaign.
