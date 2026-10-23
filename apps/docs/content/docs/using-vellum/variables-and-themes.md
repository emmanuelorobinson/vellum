---
title: "Variables and themes"
description: "Design tokens as variables: their types, aliases, theme axes such as light and dark, per-frame themes, and how they export to CSS and Tailwind."
---

A variable is a named value: a colour, a number, a string or a boolean. A layer does not hold the colour itself; it holds a
reference to the variable, so changing the variable changes every layer that uses it. Variables are the design's tokens, and
they map one to one onto the CSS variables and the Tailwind theme your code uses.

## Types

| Type | Used for | Example name |
| --- | --- | --- |
| Colour | Fills, strokes, text colour | `color/text` |
| Number | Spacing, radius, font size | `space/4` |
| String | A font family, a label | `font/body` |
| Boolean | Showing or hiding a layer | `show/icon` |

Names use slashes to group related tokens, such as `color/surface` and `color/surface-inverse`. A name is what agents and the
command line refer to a variable by, so a clear name matters more than a short one.

## Themes

A variable can hold one value per **theme**. A theme is an axis with a set of modes. A document might have one axis called
`Mode`, with the modes `light` and `dark`, so `color/surface` is white in light mode and near-black in dark mode. Axes are
independent: a document can have a second axis for the device, with `phone` and `tablet`, so each variable can take a value for
each combination.

A frame can carry its own theme. A frame set to dark shows its layers in the dark values, while the rest of the page stays in
light. Use this for a screen that must look different from the rest of the page.

## Aliases

An alias is a variable whose value is another variable. `color/card` can be an alias of `color/surface`, so changing the surface
changes the card too. Aliases keep a design system's layers in two levels: the palette, and the meaning each colour carries in
the interface.

## Binding and replacing

A layer's property can hold a value or a variable. Two tools make the link:

- `set_variables` adds, changes or removes variables. Send a name that exists and the variable changes. Send a new name and a new
  variable is made. The result lists the id of each one by name, and a property can be bound to that id.
- `replace_properties` changes every matching value in a scope in one change. It is the way to turn repeated `#111111` text
  colours into `color/text`. Run `find_properties` first to see which values a page already uses.

A variable that a layer still refers to cannot be removed. The change is refused, and nothing else in the batch is applied.

## Tokens in code

The variables export to the formats your code reads:

```sh
vellum tokens export design/app.vellum --format css-vars > src/tokens.css
vellum tokens export design/app.vellum --format tailwind-v4 > app/theme.css
vellum tokens export design/app.vellum --format dtcg-json > tokens.json
```

The import goes the other way. A token file's names are matched with the document's variables, a name the document already has
keeps its variable, and a new name becomes a new variable. `--dry-run` shows the change first, and `--remove-missing` removes the
variables the token file no longer has. The import is one change.

The [export formats](/docs/reference/export-formats) reference lists the token formats, and the
[exporting and handoff](/docs/using-vellum/exporting-and-handoff) page shows the commands in context.

## Checking the tokens

Ask for the variables with `get_variables`. It lists each variable's id, name and type, the value it has under the theme, and every
value it can take.
