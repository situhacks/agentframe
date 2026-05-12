# Browser Use Notes

Load this only when running this workflow with local browser-harness.

- Workfront React inputs may not behave cleanly with plain `type_text`; prefer framework-aware value setting such as `fill_input` or direct value-setting that dispatches `input` and `change` events.
- Workfront dropdown labels can split words across nested spans, e.g. `Customer Acq\nuisition` or `Attract New C\nustomers`. When selecting options, match compacted text rather than exact visible text.
- Treat these as browser-harness execution notes only. Do not copy them into `recipe.md` unless the workflow shape itself changes.
