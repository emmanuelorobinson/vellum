---
title: "Frames and layout"
description: "Frames, auto-layout, groups, and the arrange tools for grouping, flipping, scaling, aligning and distributing layers."
---

## Frames

A frame is a container with a size and a fill. It clips its children to its edges when you ask it to. A screen is a
top-level frame on a canvas page, and a slide is a top-level frame on a slides page.

Draw a frame by pressing `F` and dragging, or ask an agent for one. Give it a name. Names are how people and the command
line refer to a frame, and an export of a frame needs its name or id.

## Auto-layout

A frame with auto-layout places its children in a row or a column, with a gap and padding. Each child can size itself to
its content (hug), fill the space its parent leaves (fill), or keep a fixed size. Changing the frame's size reflows the
children; a child that is fixed does not move.

Auto-layout follows flexbox and grid semantics. Use it for any group of things that should keep their spacing as the
design changes.

A fill on the main axis of a frame that hugs its content has no size to fill. The layout uses the fallback you gave it, or
hugs, and the layout check reports that it did.

## Layout problems

The layout check finds the problems an agent should fix before it stops:

- text that overflows its box
- children clipped by a clipping ancestor, including rounded and transformed clips
- overlapping siblings in a flow
- layers outside every frame
- a hug cycle that cannot be resolved

An agent reads the problems with `snapshot_layout` and fixes them. To run the check yourself, call the same tool from the
command line. Its `--help` lists the flag that limits the answer to problems:

```sh
vellum tool snapshot_layout --document design/app.vellum --json
```

## Groups and wrapping

- **Group** wraps layers in a group. A group has no fill and no size of its own; its size comes from its children.
- **Wrap in frame** wraps layers in a frame, which has a size and can have a fill.
- **Ungroup** puts a group's children where the group was, and removes the group.

Grouping keeps every layer where it appears on the canvas. The group sits at its children's corner, and the children are
moved to positions relative to it. Layers that follow a layout can be grouped only with each other, and layers at fixed
positions only with each other, because a group needs one or the other to find its place.

## Arranging layers

The arrange tools change layers' geometry as one batch:

| Action | What it does |
| --- | --- |
| Flip | Mirrors a layer about its own centre, horizontally or vertically |
| Scale | Scales a layer by a factor about an anchor on its own box. The anchor stays where it is |
| Align | Lines up layers at their left, right, top or bottom edge, or at their centres, within their collective bounds |
| Distribute | Spaces three or more layers with equal gaps. The outermost layers stay where they are |

Align and distribute need the layers to sit at fixed positions with fixed sizes. Layers that hug their content have no
fixed size to measure, so the change is refused and says which layer it cannot measure. Scale of a hug-sized layer is
not available through agents yet.

## Keyboard

The [keyboard reference](/docs/reference/keyboard-shortcuts) lists the defaults: grouping, ungrouping and wrapping,
flipping, aligning and distributing, the stacking order, and the auto-layout toggles.
