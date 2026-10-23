---
title: "Skills and styles"
description: "The instructions an agent reads before it designs, the visual style recipes it can load, and the order in which they apply."
---

An agent designs better when it reads the rules first. {{brand}} gives it two kinds of help: skills, which are instructions in
text, and styles, which are visual recipes it can load when there is no design system to follow.

## Skills

A skill is a file of instructions. The built-in skills are:

| Skill | What it covers |
| --- | --- |
| The format reference | The shape of a document, its nodes and their fields |
| The execute reference | The `doc` API and how a program is checked |
| Design a screen | The steps for a screen, from brief to checked frame |
| Build a design system | Variables, components and guidelines, in order |
| Make a chart with a script | A script node that draws a chart from inputs |
| Recreate from code | Turning an existing interface into frames, with its tokens |

`read_skill` returns a skill by its path, and `list_skills` lists what is available. A design system's own guidelines are skills
too, and so are the skill folders you have chosen to share with {{brand}}. Only folders you have explicitly allowed are read.

## Styles

A style is a recipe of palette, typography, radius and density. Six are built in: earth, editorial, high contrast, quiet, rounded
and technical. `get_style` lists them, or loads one by name. Loading a style does not change the document and does not install
any fonts.

A style is for a design that has no system yet. Use one when the brief names no brand, and the design needs a consistent starting
point.

## The order of precedence

When more than one source applies, the nearer one wins:

1. The variables the document already defines.
2. The design system's guidelines, if the document has a design system.
3. A style recipe, when neither of the above sets a value.

An agent that loads a style on a document with its own colour variables uses the variables, so a style never overrides the
document's own decisions.

## Writing your own guidance

A guidelines file for a design system is plain text. It can say which components to use for which job, which variables carry which
meaning, and what must not change. Keep it short. An agent reads all of it before it designs, and a long file costs time without
adding much.
