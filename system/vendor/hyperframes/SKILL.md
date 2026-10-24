---
name: hyperframes
description: Full HyperFrames video capability, including Studio, the CLI, the composition engine, and the upstream skill family (hyperframes, hyperframes-cli, hyperframes-registry, media-use, slideshow, figma, remotion-to-hyperframes).
---

# HyperFrames

Read [`source/skills/hyperframes/SKILL.md`](source/skills/hyperframes/SKILL.md) before any HyperFrames video task. It is the upstream router: it holds the render contract and the work loop, and it names the reference or sibling skill each kind of video needs.

AgentFrame owns project context, deliverable state, and export reconciliation through [`library/process/video-production.md`](../../../library/process/video-production.md). HyperFrames owns video craft, project files, Studio, and rendering.
