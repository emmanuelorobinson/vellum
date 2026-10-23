---
title: "Text and fonts"
description: "Text layers, how their box sizes to the text, and which fonts the document can use."
---

## Text layers

A text layer holds runs of text. Each run has its own font, size and fill, so one layer can mix a bold word with a regular
one. A text layer's box can size to its text, have a fixed width that wraps, or have a fixed width and height. The editor
and the agents use the same layout for the text, so an export matches the canvas.

Text supports links, underline, strikethrough, vertical alignment and justification. Text that overflows its box is one of
the layout problems the layout check reports. Fix it by giving the layer more room, or by letting it grow.

Give text a fill. A text layer without one is invisible, and an agent that writes text without a fill will see nothing on
the canvas.

## Fonts the document can use

{{brand}} bundles two fonts with the engine: Inter, the interface font, and Noto Color Emoji. Text in a document that uses
them needs nothing else installed.

A document can carry its own fonts. A TrueType, OpenType, WOFF or WOFF2 font that a host imports is stored beside the
document, so it travels with the design and loads before the bundled fonts. The `list_fonts` tool lists the families a
document can use: the bundled ones, the fonts stored with the document, and the families its text already uses.

A text run that names a family the engine does not have is drawn with a fallback font.

## System and web fonts

The engine can also use system fonts and cached web fonts that a host registers with it. Finding those fonts belongs to
the host. A font that the document names but that no host has registered is drawn with the fallback. So the same document
can draw differently on two computers that have different fonts. Check the export on the machine that will ship it.

## Checking text

Ask the layout check for text problems, or export the frame and look at it. An export draws text with the same font and
layout as the canvas, so a text box that overflows in the export overflowed on the canvas too.
