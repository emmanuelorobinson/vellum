# Design craft

How to make a design look considered, not just correct. Read it with `workflows/design-a-screen.md`. The
person's brief, the design system's guidelines and the document's variables outrank everything here; the numbers
below are defaults for when none of them says otherwise.

## Decide before you draw

- Write down, for each screen: its one job, its primary action, and three to five blocks of content in order of
  importance. If you cannot, the screen is not ready to draw.
- Use real, specific content: names, numbers, times, places, prices, with believable length differences between
  rows. Never "Lorem ipsum", "Item 1" or "Title". Keep copy short and concrete.
- Several screens of one product share one system: the same type scale, spacing, radii, colours, icon set and
  component set. Build the shared parts once, as components, then compose the screens from them.

## Hierarchy

- One focal point per screen. Make it larger, heavier or higher-contrast than everything else, not all three.
- At most three text sizes and one accent colour inside a block. Prefer weight contrast (600 against 400) to many
  sizes.
- Group by proximity: the space inside a group is at most two-thirds of the space between groups. Use space before
  dividers, and dividers before boxes.
- Align to one left edge per column. Centre only short, single lines (a title, a button label, an empty state).

## Type

- Mobile: body 16, secondary 14, captions and labels 11 to 12 (never below 11). Headings follow a scale of about
  1.25: 20, 24, 32, 40. Wide frames: body 16 to 18.
- Line height about 1.5 for body, 1.2 for headings. Tighten tracking on large display text (-1 % to -2 %).
- One family, or two at most (a display face and a text face). Sentence case. Left-aligned paragraphs, 45 to 75
  characters a line.
- Numbers that change or line up (times, distances, prices) read best in a mono face, or at least right-aligned.
- Set every text node to wrap inside a fixed or `fill` width. Do not let long text run past its frame.

## Spacing and shape

- Work on a 4 px grid; use 8, 12, 16, 24, 32, 48 between things. Keep the screen margin constant (16 to 24 on
  mobile, 24 to 40 on desktop) and the card padding constant (16).
- One radius scale (for example 8, 12, 16, full). A nested corner's radius is the outer radius minus the padding
  between them.
- Depth is quiet: borders of 1 px in a low-contrast neutral, or one soft shadow (y 4 to 12, blur 12 to 32, 8 to
  16 % opacity). Do not mix both on one surface, and do not stack shadows.

## Colour

- Neutral surfaces plus one accent. The accent marks the primary action and the one or two things that matter;
  keep it under about a tenth of the area. A second hue only for status.
- Contrast: body text 4.5:1 against its surface, large text and icons 3:1. Check text over images and over the
  accent colour, which is where it usually fails.
- Never carry meaning in colour alone: a status has a symbol and a word.
- Every colour comes from a variable. A dark theme is not an inverted light one: lift the surfaces in steps,
  soften the accent, and re-check contrast.

## Pictures and icons

- Use real pictures where the content is a picture (people, places, products): `fill_image` with a repository
  file or an allowed source. Do not hand-draw portraits or scenes from rectangles and ellipses; it reads as
  clip art. If no picture source is available, say so in your report and use a neutral, labelled placeholder
  frame, so the person can drop the picture in.
- Avatars are circles holding a picture. An image fill with `fit: "fill"` covers the shape and crops, so the
  picture is never stretched.
- Icons come from `search_icons` and `insert_icon`: one set, one stroke weight, 20 to 24 px in controls, sat on
  the text's optical centre, coloured with a variable. Do not use emoji or text characters as icons.

## Mobile screens (390 x 844 unless the brief says otherwise)

- Status bar at the top: a component with the time, signal and battery as icons, not as text characters. The top
  safe area is about 47; the home indicator and bottom safe area about 34.
- Touch targets at least 44 x 44. A primary action sits within thumb reach: full width at the bottom with a 16
  margin, or in the tab bar.
- A tab bar has three to five items, each an icon with a short label, the current one marked by weight and colour
  and also by shape (a filled icon or a bar).
- Lists show three to five rows that differ in length. Cards that repeat are one component with props.

## Web and desktop screens

- A 12-column grid with a content width of 1120 to 1280; section padding of 64 to 120 at the top and bottom;
  navigation 56 to 72 high.
- A hero has one headline, one line of support and one primary action. Everything else waits below.

## States

- Components that are interactive get their states as variants: default, hover or pressed, focus, disabled,
  and error where an input can fail. Draw the empty, loading or error state of any screen whose flow needs it.

## Building it in Vellum

- Use auto layout (`layout: "column"` or `"row"`, `gap`, `padding`, `fill` and `hug`) instead of positioning
  children by hand. Absolute positions only for badges and overlays.
- A line break inside text is a real newline character in the string. Never type the two characters backslash and
  `n`; they show up on the canvas as a visible `\n`.
- Name layers for what they are (`Card / Runner`, `Button / Primary`) so a later edit can find them.
- Lay several screens out side by side with `find_space`, with 80 to 120 between them and each frame named for
  its screen.

## Before you say you are done

Take a screenshot of each frame you changed and read it as a stranger would: where does the eye land first, is
anything misaligned by a few pixels, are all gaps from the scale, does any text touch an edge or wrap badly, is
any text hard to read, do pictures and icons look like one set, is any copy still placeholder. Run
`snapshot_layout` with `problems_only` and fix what it names. Fix first, then report; and report anything you
could not do (a missing picture source, a font that is not installed) in plain words.
