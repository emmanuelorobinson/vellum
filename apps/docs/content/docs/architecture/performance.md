---
title: "Performance"
description: "The performance budgets {{brand}} is measured against, the measurements taken so far and on what machine, and the budgets that are not yet met."
---

The architecture sets budgets for the things a person feels: how smoothly the canvas pans, how quickly a drag responds, how fast a large file
opens, and how quickly a batch of agent changes is applied and drawn.

## The budgets

| Target | Budget |
| --- | --- |
| Pan and zoom on a 5,000-node page | 60 frames per second, on an Apple M1 or a mid-range Windows laptop |
| Drag a layer | From input to the new frame in under 16 ms |
| Open a 2 MB file | Under 500 ms to the first paint |
| An agent batch of 200 nodes | Applied and drawn in under 100 ms |
| The WebAssembly module | About 2 MB compressed, with the core, the renderer and the text engine together |

The budgets are checked by the benchmark suite, which fails a run that regresses by more than ten per cent. Release builds only are measured.

## What has been measured

The architecture records the measurements taken so far, on an Apple M5 Pro only:

- The renderer spike panned 10,000 nodes at 232 frames per second.
- A batch of 200 nodes was planned in 13 ms and undone in 15 ms, on a document of 10,000 nodes.
- The module measured 1.95 MB compressed, before the script runtime's additional 0.4 MB.

These are good signs, not a claim that every budget is met on every machine.

## What is not met yet

The repository's open items record the budgets that are not met yet, and they are listed here so you can see them:

- Native pan, zoom, drag, the 200-operation batch and the Canvas 2D fallback exceed their budgets in the full pipeline. The idle memory footprint, the Skia drawing time and both module size gates pass.
- In the browser, a frame with any shadow or blur is drawn by the CPU across the whole viewport. A changed frame of the shapes fixture takes about 150 to 400 ms at 1280 by 720 pixels, and more at twice the pixel density. The fix is to draw those effects on the GPU, and that needs the visual goldens to pass first.

## Measuring

The performance commands are in the [commands](/docs/development/commands) page. A measurement reports the machine it ran on, and a result from one
machine is not presented as a result for another. The [performance and observability runbook](/docs/operations/runbooks/performance-observability)
describes the gates and the way they are interpreted.
