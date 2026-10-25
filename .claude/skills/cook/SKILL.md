---
name: cook
description: Recipe lookup. Prints the one recipe block from the groceries recipes.md whose heading matches the dish named, verbatim.
disable-model-invocation: true
---

Read `workspace/life/systems/groceries/recipes.md`. Never edit it.

A block starts at a level-two heading (a line beginning `## `) and runs to the line before the next `## ` heading, or to the end of the file.

Match `$ARGUMENTS` against the heading text after `## `, case-insensitive substring: `pork chops` finds `## 5. Oven pork chops with carrots and potatoes`.

- **One match**: print that block verbatim, heading included, and nothing else.
- **No match, or no argument**: print every `## ` heading, one per line, and nothing else.
- **More than one match**: print the matching headings, one per line, and nothing else.
