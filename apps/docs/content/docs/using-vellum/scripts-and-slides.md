---
title: "Scripts and slides"
description: "Script nodes that draw layers from a small JavaScript file, what a script can and cannot do, and presenting and exporting slides."
---

## Script nodes

A script node draws layers from a JavaScript file that sits beside the design. Charts, grids and repeated patterns are good
uses: the layers follow the script's inputs, and you change the inputs in the properties panel rather than editing the
layers by hand.

A script declares its inputs in a header comment. Each input has a type, a default and, for numbers, a range:

```js
/**
 * @schema 1
 * @input count: number(min=1,max=20,step=1) = 3
 * @input caption: string(multiline=true) = "Example caption"
 * @input enabled: boolean = true
 * @input tint: color = "#123456"
 * @input mode: enum("compact","wide") = "compact"
 * @input target: ref(kind="page") = "page"
 */
return Array.from({length: doc.inputs.count}, () => ({
  type: "rect", width: 10, height: 20,
  fills: [{type: "solid", color: doc.inputs.tint}],
}));
```

The script returns layers. It cannot change the document itself, and it cannot read files or reach the network. A script's
output is derived: it is shown and it is exported, but it is not saved into the file. When you want the output to be ordinary
layers you can edit, convert the script node. The conversion copies the current output into layers with new ids, in one change.

### What a script can and cannot do

- A script is a strict function body. It must `return` its result, and the result must be plain data.
- There is no network, no file access and no timer. There is no console. A script reports through its return value, not
  through a log.
- Use `Math.pow(x, y)` for powers. The exponent operator is refused.
- A script has a time limit, a call limit, a limit on the layers it creates, and a memory limit. Going over a limit fails the script
  and changes nothing.

Agents write scripts too, through `execute`. Their scripts follow the same rules, and one script is one change and one undo step.
The [script API](/docs/reference/script-api) page lists what a script can call.

## Slides

A slides page is a page whose top-level frames are the slides, in order. Add one with **New page**, choose **Slides**, and draw
frames. Notes, context blocks and prompts on a slides page are for editing and are never slides.

Reorder slides by dragging them, or with `Alt+↑` and `Alt+↓`. Each reorder is one undo step.

Presenting starts at the selected slide, or at the first one, with `Mod+Enter`. Move with the arrow keys, `Space` or `Enter`.
`Escape` ends the presentation. Presenting shows a picture of each slide from the export renderer, and it never shows a stale
picture as if it were the current one.

Export the slides as one PDF from the command line:

```sh
vellum export deck.vellum --format pdf --output slides.pdf
```

> **Availability.** The command-line PDF export works. The installed desktop and VS Code hosts do not yet present or save a live
> deck. See the [availability](/docs/getting-started/availability) page.
