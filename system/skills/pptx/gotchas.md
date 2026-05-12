# PPTX Rendering Gotchas

Skill-local quick fixes for recurring render failures. Keep entries in symptom -> fix form.

## 1) Windows headless Chrome crops HTML screenshots

Symptom: `--window-size=1920,522` still produces bottom whitespace/cutoff on Windows (`--headless=new`), while macOS/Linux renders cleanly.

Fix:
- Render with a vertical buffer (`renderHeight = targetHeight + 200`).
- Keep the source HTML body height at the target height.
- Screenshot at buffered height, then crop to target dimensions.
- On Windows, crop via PowerShell/System.Drawing; on other platforms use equivalent tooling.

Why: Windows headless reserves phantom browser-chrome height.

## 2) PNG distortion when mixed with native text in same PPT slot

Symptom: a fixed-aspect PNG gets stretched/squished when native PPT text in the same body region grows or wraps.

Fix:
- Pin PNG dimensions to source aspect ratio first.
- Place native text in a separate region below/beside the image.
- Never resize the PNG to free space for text.

Why: PNG aspect is fixed; native text reflows.
