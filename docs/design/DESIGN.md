# Vellum — Design Direction (v2: agent-first)

> **Vellum** is a working codename. Replace the name and wordmark when the brand is final; nothing else in this brief depends on the name.

## Purpose and direction

Vellum is an open-source design canvas that lives next to code, where **agents do most of the drawing and the person directs**. A person opens a `.vellum` file in VS Code, Cursor or Windsurf (or the desktop app), describes what they want — or points at a spec like `DESIGN.md` — and watches one or several agents plan and build the screens live. They steer by talking, selecting and pointing, then refine by hand with a complete vector toolset. Designs are plain files in the repo, versioned with Git, with tokens synced to CSS.

This brief covers the **editor** (one design shared by the IDE extension and desktop app), the **desktop app shell** (macOS first; Windows and Linux share the layout), the **web share viewer and account pages** (Pro), and the **CLI's output style**.

The product loop is:

**Describe → plan → watch → steer → refine → implement → commit.**

The design direction is **a director's table**. The conversation with your agents sits on the left, always open; the canvas fills the window and shows their work arriving; precise hand tools wait on the right for when you want to take over. The chrome is neutral and quiet so the only colourful, moving things are the design and the agents building it.

**The agent panel is home. Blue means "selected". Violet means "component". Agent colours mean "an agent is here".** Amber is reserved for "needs you" (a plan to approve, a conflict) and red for errors and destructive actions.

The signature product details are the **Agent Panel** (sessions, turns, plans and the Step Timeline), the **Agent Presence** layer on the canvas (planned frames, cursors, leases) and the **Change Ledger** (what changed, by whom, reversible). Hand-editing tools should feel familiar and calm.

## Product principles

1. **Describe first, draw second.** The agent panel is the default view and the composer is always one keystroke away (⌘L). Hand tools are complete, but they're the refinement layer, not the front door.
2. **Show the work as it happens.** Big requests become a visible plan before anything is built. Planned frames appear on the canvas as placeholders before content arrives. Every turn carries its steps, with thumbnails of what was designed.
3. **Steer, don't restart.** You can interrupt and redirect mid-turn, edit a plan before it runs, select layers and ask about just those, or drop a quick prompt right next to the selection.
4. **Every agent is the same kind of thing.** Built-in agents and external coding agents (Claude Code, Cursor, Codex) appear in the same session list, on the same canvas, in the same ledger, with the same Stop.
5. **Undo is yours.** ⌘Z undoes your own last action, never an agent's. Agent work is undone per turn (from the message) or per change (from the ledger).
6. **You can always take the wheel.** You can edit anywhere at any time, including inside a frame an agent is working on; your edits win. Stopping one agent or pausing all of them is one click.
7. **Honest about cost, models and the machine.** The engine, model, parallel count and per-turn usage are always visible. When the daemon is offline, a key fails or a font is missing, say so plainly with what still works.
8. **It's a file.** The file name, save state and Git branch are always visible. "Saved" means written to disk. Agent conversations are stored on your computer, never in the design file.
9. **Agents are tools, not characters.** External agents are named by their client ("Claude Code"); built-in agents are named by their colour ("Teal"). No faces, moods, emoji or fake typing.

## Visual system

### Appearance

The editor ships **light, dark and high-contrast** appearances from day one.

- **In the IDE**, the editor follows the host theme's *kind* (light, dark, high-contrast light, high-contrast dark) with Vellum's own tokens. A setting, **Match editor colours**, maps surface and text tokens to the host theme for people who want the extension to disappear into their theme.
- **On desktop**, it follows the system setting with an in-app override.
- **The canvas backdrop** is a per-document token (saved in the file), so designers can judge work against light and dark backdrops.

### Color

| Role | Light | Dark | Use |
|---|---|---|---|
| Canvas backdrop | #EDEDEB | #18191C | Area behind frames (per-document override) |
| Panel | #FFFFFF | #222327 | Floating panels: agent panel, tool rail, properties, top bars |
| Raised | #FFFFFF | #2B2D32 | Menus, popovers, dialogs, the inline prompt (light uses shadow; dark the lighter fill) |
| Sunken | #F3F3F1 | #1B1C1F | Composer, inputs, step rows, code/spec blocks, value fields |
| Primary text / action | #17181B | #ECEDEF | Labels, values, primary buttons |
| Secondary text | #5D626B | #A3A7AE | Section headers, metadata, model names |
| Tertiary text | #7B8089 | #7E838B | Placeholders, quiet labels; never essential information |
| Inverse text | #FFFFFF | #17181B | Text on primary buttons, your message bubbles |
| Your message | #17181B | #ECEDEF | Fill of your bubbles in the agent panel (ink, not colour) |
| Hover row | #F1F1EF | #2A2C31 | Pointer hover in lists |
| Selected row | #E8EEFF | #23304F | Selected layer, active session, active list item |
| Separator | #E6E6E2 | #313339 | Hairlines between panel sections |
| Border | #D8D8D3 | #3A3D44 | Inputs, outlined buttons, panel outline (dark) |
| Focus ring | #2F6BFF | #6E97FF | 2px ring with 1px offset on every focusable control |
| **Selection** | #2F6BFF | #4C86FF | Selection outlines, handles, canvas hover outlines. Never used for buttons. |
| Selection tint | #E8EEFF | #23304F | Marquee fill (at 20% on canvas), selected rows |
| **Component** | #7A4FD8 | #A88BF2 | Component and instance outlines, icons and labels only. Main components get a 1.5px solid outline and a filled component icon; instances a 1px outline and the instance icon, so the two are told apart by shape and label, not only by shade. |
| Variable chip | #F0EBFC / text #5B3BB0 | #2E2645 / text #C9B8F7 | Values bound to a variable |
| Needs you | #8A5A00 on #FFF3D6 | #F2B84B on #2E2410 | Plans waiting for approval, conflicts, missing fonts, unsaved |
| Error | #B42318 on #FCEEEE | #F97066 on #2D1414 | Failed steps, rejected writes, destructive actions |
| Success | #18794E | #4CC38A | Step done, saved, export done (icon and text only, never a fill) |

**Agent colours.** Each active agent gets one of eight hues for its marker, cursor, lease outline, planned-frame placeholders and session dot. Built-in agents are named by their hue ("Teal", "Saffron"); external agents keep their client's name and are assigned a hue. The hues deliberately avoid the selection blue and component violet:

| Hue | Light | Dark |
|---|---|---|
| Teal | #0F8F86 | #3CC4B4 |
| Green | #2E8A4E | #5CC97F |
| Saffron | #B7791F | #F0B451 |
| Orange | #C9541D | #F2884F |
| Rose | #C23B5E | #F07393 |
| Magenta | #A63A8F | #E07BCB |
| Lime | #6F8A12 | #B5D44A |
| Sand | #8A6A45 | #CDA97B |

Cursors and name tags carry a 1px outline in the panel colour so they stay legible over any design. Colour is never the only signal: every cursor, lease, placeholder and session has a text label.

**Agents are rounded squares, humans are circles.** Agent markers everywhere (session list, messages, cursors, ledger, presence) are rounded squares (radius 30% of size); human collaborators are circles. This must stay consistent so an agent is never mistaken for a teammate.

### Typography

Use **Inter** for UI (bundled, so extension and desktop match everywhere) and **JetBrains Mono** for anything to copy or compare exactly: file paths, node IDs, variable names, model names, CSS, CLI commands, hex values, usage numbers. For Arabic UI, fall back to **IBM Plex Sans Arabic**; for CJK, the platform's system CJK font.

The agent panel is a reading surface and uses prose sizes; the properties panel is a precision surface and uses dense sizes. Nothing is ever below 11px.

| Role | Size / line height | Weight | Use |
|---|---:|---:|---|
| Window title | 20 / 1.25 | 650 | Desktop home, first-open prompt heading |
| Dialog title | 15 / 1.3 | 600 | Dialogs, plan card title, empty states |
| Agent prose | 14 / 1.55 | 400 | Messages from you and agents, plan items |
| Agent meta | 12 / 1.4 | 500 | Agent name, model, step labels, turn footer |
| Panel section | 11 / 1.35 | 600 | Section headers in properties, sentence case |
| Panel body | 12 / 1.4 | 450 | Labels, values, layer names |
| Panel caption | 11 / 1.35 | 500 | Hints, metadata, units |
| Canvas label | 11 / 1.2 | 500 | Frame names, lease tabs, placeholder labels (screen-space, never scaled) |
| Mono value | 12 / 1.4 | 450 | Hex, IDs, paths, model names, usage numbers |
| Mono block | 12.5 / 1.5 | 450 | Spec, CSS and JSON blocks, step output |

Value fields use tabular figures; values stay attached to units ("16 px", "2×", "38k tokens"). Layer names truncate in the middle; session titles truncate at the end and show in full on hover.

### Spacing and shape

Use a **4-point grid**. Panels **float over the canvas** with an 8px inset from the window edges and each other, so the canvas runs full-bleed behind them. Agent panel padding is 16px; properties padding 12px; property rows 28px; layer rows 28px; step rows 36px.

| Token | Value | Use |
|---|---|---|
| radius-xs | 4px | Value fields, chips, layer row selection |
| radius-sm | 6px | Buttons, menu items, inputs, step rows |
| radius-md | 10px | Message bubbles, step thumbnails, popovers, code blocks |
| radius-lg | 14px | Floating panels, the composer, dialogs, plan card |
| radius-full | 999px | Human avatars, status dots, zoom pill, parallel/variants steppers |

Elevation: in light appearance, floating panels share one soft, low shadow; menus and dialogs a slightly stronger one. In dark appearance, panels use the Panel fill plus a 1px Border outline instead of shadows. Never nest cards more than one level inside a panel.

### Icons and motion

Use **Lucide** line icons at 1.5px stroke, 16px in panels, 18px in the tool rail. Layer-type icons: frame (frame), auto-layout frame (rows-3 / columns-3), text (type), rectangle (square), ellipse (circle), vector (pen-tool), image (image), group (group), component and instance (component / diamond, in Component violet), boolean (combine), icon (shapes), script (code), slot (dotted square), note (sticky-note), context (scroll-text), prompt (message-square-play), embed (app-window). Step icons: read file (file-text), read guidelines (book-open), plan (list-checks), claim frame (lock), design (layout-template), screenshot check (scan-eye), tokens (palette), search layers (search), run script (code), check layout (ruler), insert icon or image (image-plus), permission (shield-question), external command (square-terminal). Never sparkles, never robot faces.

Motion only clarifies state, and direct manipulation is never animated:
- Pan, zoom, drag, resize and selection respond on the next frame with no easing.
- **Planned frames** appear as placeholders with a slow shimmer in the agent's hue (2s cycle) until the agent starts filling them; static under reduced motion.
- **Inserted nodes** fade in over 120ms with a one-time 600ms tint in the agent's hue. Under reduced motion: no fade, a static 1s outline.
- **Agent cursors** glide over 100ms; under reduced motion they jump.
- **Streaming text** appears as it arrives, with no artificial typing delay.
- **Working steps** show a slow rotating ring (2.4s per turn), static under reduced motion.
- Panels and popovers fade in over 120ms; dialogs 160ms fade + 8px rise; toasts slide 8px.
- No celebrations, confetti or bouncing. Completion is a check and a factual line.

## Signature component: Agent Panel

The Agent Panel is the editor's home: a floating panel on the left (360px, resizable 320–520) that opens by default for every document. Its first tab is **Agent**; the others are **Layers**, **Assets** and **History** (the Change Ledger).

### Session bar

At the top of the Agent tab:
- A **session switcher**: the current session's title (generated from the first request, renameable), its agent marker, and a state dot (working, needs you, done, failed). The dropdown lists every session for this document, newest first, grouped **Running**, **Needs you**, **Recent**. External agents connected over MCP appear here as sessions too ("Claude Code · external").
- **New session** button. Each session is an independent conversation with its own agent(s); several can run at once on different frames.

### Turns

A session is a list of turns. Each turn is:
1. **Your message** (ink bubble, right-aligned) with any context chips you attached (files, frames, selection).
2. **Agent header**: marker, name and model in mono ("Teal · `deepseek/deepseek-v4.1-flash`"), or for parallel turns a stacked marker group ("3 agents").
3. **Agent prose**: short, factual, first person ("I've built the three onboarding screens. Step 3's chart uses placeholder data.").
4. **Step Timeline** (below).
5. **Turn footer** (mono caption): "Made 47 changes · 2 frames · 1 m 12 s · 38k tokens", with **Review changes** (opens History filtered to this turn), **Undo this turn** and, on the latest turn, **Retry**.

Agent prose and steps interleave in the order they happened, so the panel reads like a log of the work, not a summary written afterwards.

### Step Timeline

Every tool call is a **step row**: icon, plain-language verb and target ("Read `DESIGN.md`", "Read design-system guidelines", "Claimed frame Step 2", "Designed Step 2 · 31 layers", "Checked screenshot", "Updated 4 tokens"), state (working ring, check, warning, error) and duration. Rows are collapsed by default.

- **Designed** steps always show a **thumbnail** of the frame as it looked after the step. Clicking the thumbnail zooms the canvas to that frame and selects it.
- **Checked screenshot** steps show the image the agent looked at and its one-line verdict ("Fixed: button text overflowed").
- **Checked layout** steps list problems found and fixed ("2 problems · text overflow in Step 2 title fixed").
- **Ran script** steps (one `execute` call) show the number of changes and, when expanded, the script itself in a mono block with Copy, so nothing an agent did is hidden behind a single line.
- Expanding any row shows its details: targets in mono, created/changed layer counts, and raw output in a sunken mono block with Copy.
- Failed steps show the error in plain words with **Retry step**.
- Long turns group consecutive steps of the same kind ("Read 3 files") to keep the panel scannable.

### Plan card

When a request will produce more than one frame or more than ~20 changes, the agent first replies with a **Plan card** instead of building:
- Title ("Plan · 12 screens from DESIGN.md"), the list of frames it will create or change, each with a page and a one-line intent; items are editable, reorderable and removable.
- **Parallel** stepper (1–6 agents) with a mode: **Split** (agents divide the plan: "4 agents, about 3 screens each") or **Side by side** (each agent builds the whole request separately, for comparison; each lane can use its own model).
- Primary **Build**, secondary **Edit request**. While waiting, the card is marked **Needs you** (amber) and the session shows in the Needs you group.
- After Build, every planned frame appears on the canvas as a **placeholder** in its agent's hue, so you can see the whole layout before content arrives. Plan items tick off live as their frames finish.
- A setting lets people skip plans for requests under a size they choose; the default is to always plan multi-frame work.

### Composer

Pinned to the bottom of the Agent tab and always visible there, even while scrolling:
- Multi-line field with placeholder "Describe what to design or change…". Enter sends, Shift+Enter adds a line.
- **Context chips** above the field, added automatically and removable: the current selection ("3 layers in Step 2"), the current page, and anything attached. **@** mentions files in the workspace (`@DESIGN.md`), frames, components and variables. Dragging a file or image onto the composer attaches it.
- **Footer controls:** attach (+), **Parallel** (1×–6×, with Split or Side by side), **Explore** (off, or 2–6 successive variants that vary **Layout** or **Style** after the first result), **Engine and model** picker (with reasoning effort where the model supports it), a **usage** meter, and Send — which becomes **Stop** while any agent in this session is working. With several agents running, Stop offers **Stop this agent** and **Stop all**.
- **Skills:** typing **/** at the start of the field opens the skills menu — built-in skills, the design system's guidelines and any `SKILL.md` folders the person added (**Add skill…**). A chosen skill appears as a chip and is sent with the request.
- **Usage meter:** a small ring showing how full the model's context is. Clicking it opens a popover: tokens in this session by kind (conversation, files, screenshots), and **Estimated cost at API rates**, always labelled as an estimate. Installed engines on a subscription show usage against their limit instead of cost.
- When layers are selected, the field's placeholder changes to "Change the selected layers…" so the scope of a request is always obvious.

### Inline prompt

For quick edits without moving your eyes from the canvas: with a selection, press **⌘K** on desktop (**⌥K** in the IDE, where ⌘K is reserved) or click **Ask** in the selection toolbar. A compact composer appears beside the selection (Raised surface, 360px). Its turn runs in the current session and appears in the Agent panel like any other turn; the inline prompt shows only a progress line and **Undo**.

### External agents

Sessions from Claude Code, Cursor, Codex or Gemini CLI show their **steps** (every MCP call) with the same Step Timeline, thumbnails and Undo, but the conversation itself lives in that tool. The composer is replaced by a line: "Chat with Claude Code in its own window · Stop", and the session header shows the client name, its colour and how long it's been connected.

### Permission card

When an agent wants to do something outside the canvas — read a file in the repo, write a file (tokens, a script, exported HTML), or run a command — the turn pauses on a **Permission card** in the Agent panel:
- Agent marker and a plain sentence ("Teal wants to read `DESIGN.md`", "Claude Code wants to run `pnpm build`"), with the exact path or command in mono.
- **Allow once**, **Allow for this session**, **Deny**. Denying tells the agent why it can't proceed.
- The session dot turns amber (Needs you) and, if the app is in the background, an OS notification says the agent is waiting.
- Canvas edits never ask: they're visible and undoable. Settings can pre-allow repo reads; "Skip all permission prompts" exists, is off by default, and while on shows a persistent amber line above the composer.

### Background notifications

When a turn finishes, fails or needs permission while the editor isn't focused, an OS notification names the session and the outcome ("Onboarding screens · 12 of 12 built"). Controlled in Settings → Agents.

## Signature component: Agent Presence

Everything that shows agents on the canvas:

1. **Presence stack** (top-right bar): rounded-square markers for agents active in this document. Hover shows name, engine and state ("Teal · working · Step 2"). Click opens the presence menu: Follow, Open session, Stop this agent, **Pause all agents**.
2. **Placeholders**: planned frames drawn at their final position and size as dashed outlines in the agent's hue with a label ("Planned · Settings · Saffron"). They fill in as the agent works and become normal frames when done.
3. **Cursor and name tag**: an arrow in the agent's hue with a rounded-square tag carrying the name and a short task line while working ("Teal · Building Step 2").
4. **Lease outline and tab**: a frame an agent is working on gets a 1.5px solid outline in its hue and a tab above its top-left corner: marker, name, state ("Working · 14 changes") and **Stop**. Stop ends that agent's work on that frame and blocks its further writes there until resumed.
5. **Your edits win**: editing inside a leased frame is always allowed; the tab shows "You're editing here too" and the agent is told which layers you changed.
6. **Rejected writes** show as a red count on the lease tab ("2 changes rejected") linking to the History entry, so failures are never silent.
7. **Paused state**: a slim amber banner at the top of the canvas — "Agents paused · their changes are being rejected" — with Resume.

## Signature component: Change Ledger (History tab)

The History tab is the cross-actor record of every change since the file was opened.

- **One entry per batch.** Each agent tool call, or each of your editing bursts (grouped by a 30-second idle gap), is an entry: actor marker and name, plain summary ("Inserted 24 layers in Onboarding / Step 2", "Changed `color/primary` in Light and Dark", "You: resized 3 layers"), time and change count. Agent entries link back to their turn in the Agent tab.
- **External changes** (Git checkout, merge, direct file edits) appear with a file icon: "File changed on disk · 14 changes applied".
- **Expand** an entry for each operation: verb, layer name and node ID in mono, and before → after values ("fill: `#F3F3F1` → `{color/surface}`").
- **Actions:** Select affected layers, Zoom to, **Undo this change** (when nothing later depends on it) or **Revert** (applies inverse changes as a new attributed entry), Copy as JSON.
- **Filters:** by actor (You, each agent, Teammates, File), by session and by page.
- Footer: "History since this file was opened · Full history is in Git."

## Layout and navigation

### Editor workspace

The canvas fills the window. Everything else floats above it:

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ╭ ☰  juniper.vellum · Saved ╮                ╭ ■■ Agents · Share · ⚙ ╮ │
│ ╭───────────────────╮ ╭─╮                                ╭───────────╮ │
│ │ Agent│Layers│Assets│ │▸│                                │ Design    │ │
│ │ │History          │ │□│                                │ Inspect   │ │
│ │ session ▾  + New  │ │✎│          CANVAS (full-bleed)   │           │ │
│ │                   │ │T│                                │ properties│ │
│ │ turns · steps ·   │ │#│                                │           │ │
│ │ thumbnails · plan │ │✋│                                │           │ │
│ │                   │ ╰─╯                                │           │ │
│ │ ╭───────────────╮ │                                    │           │ │
│ │ │ composer      │ │                                    │           │ │
│ │ ╰───────────────╯ │                                    ╰───────────╯ │
│ ╰───────────────────╯   ⎇ feat/onboarding · Live · 3 agents   ╭ 50% ╮   │
└──────────────────────────────────────────────────────────────────────────┘
```

1. **Document bar** (top-left, floating): menu, file name with save state ("Saved", "Unsaved", "Saving…"), and on desktop the document tabs.
2. **Agent panel** (left, floating, 360px): Agent · Layers · Assets · History. Agent is the default tab for every document; the panel remembers the last tab per document.
3. **Tool rail** (vertical, floating, docked to the agent panel's right edge, 44px wide): Move V, Frame F, Rectangle R (with Ellipse, Line, Polygon under its menu), Pen P, Text T, Image, Hand H, and at the bottom **Ask** (inline prompt) and keyboard shortcuts.
4. **Canvas**: full-bleed, with frame names, placeholders, cursors, leases and measurement labels on ⌥-hover.
5. **Right panel** (floating, 280px, resizable 260–400): **Design** and **Inspect** tabs. It **appears only when something is selected** (or document properties are opened), so the default view is agent panel plus canvas.
6. **Top-right bar** (floating): presence stack and **Agents** (presence menu and connected clients), **Share** (Pro, signed in), **Preview** (play the prototype links), settings.
7. **Bottom strip** (floating, centred, quiet): Git branch with dirty marker, connection mode (**Live · 3 agents**, **Headless only**, **Daemon offline**), renderer note when relevant, and the zoom pill at bottom-right.

**Responsive rules.** Minimum editor width is **960px**. Below 1200px, the right panel overlays the canvas. Below 1040px, the agent panel narrows to 320px; below 960px (IDE side-by-side), it collapses to a 44px strip whose icon re-opens it as an overlay, and the composer remains reachable through ⌘L. **Tab** hides everything except the canvas; **⌘\\** toggles panels.

### Desktop app shell

- **Home window** (before a file is open): a large composer ("What do you want to design?") with folder picker, then recent files with thumbnails and recent folders.
- **Editor windows**: one window per folder, document tabs in the document bar.
- **Menu bar** (macOS): File / Edit / View / Object / Arrange / Agents / Help; Windows and Linux show them in the document bar menu.

### Keyboard

⌘ and ⌥ on macOS map to Ctrl and Alt on Windows and Linux. Canvas shortcuts work only when the canvas has focus.

**Agents**

| Action | Desktop | In the IDE |
|---|---|---|
| Focus composer | ⌘L | ⌘L |
| Inline prompt on selection | ⌘K | ⌥K (⌘K is a VS Code chord prefix) |
| Send / new line | Enter / ⇧Enter | same |
| Previous / next prompt in history | ↑ / ↓ in an empty composer | same |
| Stop current session · Pause all agents | ⌘. · ⇧⌘. | same |
| New session · next / previous session | ⌥⌘N · ⌃Tab / ⌃⇧Tab | ⌥⌘N · ⌥] / ⌥[ (⌃Tab is VS Code's) |
| Switch panel tab (Agent, Layers, Assets, History) | ⌥1–⌥4 | same |
| Command palette | ⌘/ | ⌘/ |

**Tools:** V move · K scale · F frame · R rectangle · O ellipse · L line · P pen · T text · N note · H hand (or hold Space) · hold Z zoom (click in, ⌥-click out, drag a region).

**Edit:** ⌘C / ⌘X / ⌘V · ⇧⌘V paste over selection (replaces it, keeping position) · ⇧⌘C copy as PNG · ⌥⌘C copy as HTML · ⌘D duplicate · Delete · arrows nudge 1 px, ⇧ arrows 10 px (amounts in settings); inside an auto-layout, arrows reorder · ⌘G group · ⇧⌘G ungroup · ⌥⌘G wrap in frame · ⇧A add auto-layout · ⌥⇧A remove auto-layout · ⇧H / ⇧V flip · ⌘Z / ⇧⌘Z undo / redo (your own changes only).

**Arrange:** ⌥A / ⌥D / ⌥W / ⌥S align left / right / top / bottom · ⌥H / ⌥V align horizontal / vertical centres · ⌃⌥H / ⌃⌥V distribute · ⌘[ / ⌘] backward / forward · [ / ] to back / front.

**Select:** click · ⌘-click deepest layer · ⌘-drag marquee inside frames · ⇧-click add · ⌘A all · Enter children · ⇧Enter parent · Esc clear.

**Components:** ⌥⌘K make component / turn back into a layer · ⌥⌘X detach instance · ⌥⇧R replace inside instance.

**Path editing** (after Enter on a shape): P pen · V select points · X cut segment · ⌘A all points · ⌘J connect points · double-click a point to toggle smooth / corner · ⌘-drag a point for symmetric handles · ⌥-drag a handle to break symmetry · ⌘-drag a segment to bend it (⌥⌘ keeps neighbours fixed) · ⌘-double-click a segment to straighten · ⌥-click a segment to add a point · Delete removes points, segments or handles · Enter or Esc to finish.

**View:** ⌘scroll or pinch zoom · ⇧scroll pan sideways · middle-drag pan · ⌘+ / ⌘− zoom · ⌘0 100% · ⇧1 fit all · ⇧2 fit selection · ⌘' pixel grid · ⇧⌘' snap to pixel grid · Tab hide all panels · ⌘\\ toggle panels.

**Present** (slides pages): ⌘Enter present · ← → or Space / Enter to move · Esc to stop.

**Files (desktop):** ⌘S save · ⇧⌘S save as · ⌘O open · ⇧⌘N new file · ⌘N home (recents, drafts, design systems). In the IDE, the IDE's own file commands apply.

**Host conflicts:** when the canvas or a Vellum panel has focus, Vellum's shortcuts win; elsewhere VS Code's win. Vellum never overrides ⌘P, ⇧⌘P, ⌘B, ⌘J (outside path editing), ⌘W or ⌘S (⌘S saves through the host). Every shortcut is listed, searchable and rebindable in Settings, with host conflicts marked.

## Screen requirements

Editor screens are numbered C01–C38, desktop shell A01–A06, web W01–W03, system and edge states S01–S16. Design each in light and dark; C03, C05 and C10 also in high contrast. Use the example project throughout (see Content).

### Editor: agent-first core

**C01 — First open.** A new `.vellum` file. The canvas is empty; the Agent panel is open with a large, centred-in-panel prompt: heading "What are we designing?", the composer with suggestion chips — **Design from a spec** (opens an @ file picker filtered to `.md` files), **Start from a design system** (3 bundled systems with swatches), **Pick a visual style** (a small grid of styles with a type sample and palette each, for when there's no design system yet), **Import tokens from CSS**, **Recreate a screen from my code** (@ file picker for components) — and a quiet secondary row: "Or draw by hand: press F for a frame". A line under the composer shows the engine and model, with **Connect your coding agent** if none is connected.
**Route:** create a file → C01 → C04 (plan) or C03.

**C02 — Connect agents.** Dialog listing detected clients (Claude Code, Codex — which also covers the ChatGPT desktop app and the Codex IDE extension — Cursor, Windsurf, VS Code Copilot, Gemini CLI, Antigravity, OpenCode, Kiro, Claude Desktop) with state: Connected, Ready to connect, Not detected. **Connect** writes the MCP config entry for that client and shows exactly what was written and where (path in mono); **Disconnect** removes only that entry. **Manual setup** shows the config snippet with Copy. After connecting: "Claude Code connected · 18 tools available".
**Route:** C01, Agents menu or palette → C02.

**C03 — Agent workspace (building).** Sam has sent "Design the onboarding screens in @DESIGN.md using Starter Minimal". The Agent panel shows the turn in progress: Teal's prose, steps (Read `DESIGN.md` ✓, Read guidelines ✓, Claimed frame Welcome ✓, **Designed Welcome · 28 layers** with thumbnail ✓, Designing Step 1 with a working ring). Canvas: Welcome finished, Step 1 being filled with Teal's cursor and lease tab, Steps 2–3 as Teal placeholders. Right panel hidden (nothing selected). Composer shows **Stop**.
**Route:** after sending a request.

**C04 — Plan card.** Sam asks for "all the screens in @DESIGN.md". The agent's reply is a Plan card: "Plan · 12 screens from DESIGN.md", items grouped by page (Onboarding 4, Settings 3, Paywall 2, Components 3), each editable; Parallel stepper at 4 ("4 agents, about 3 screens each"); **Build** and Edit request. Amber "Needs you" on the session dot.
**Route:** multi-frame request → C04 → C05.

**C05 — Parallel build.** After Build with 4 agents: canvas zoomed to fit, showing all 12 placeholders arranged by page, four agents (Teal, Saffron, Rose, Green) each with a lease and cursor, 5 frames already filled. Agent panel shows a **parallel turn**: a stacked header "4 agents", a progress line "5 of 12 screens done", and four collapsible lanes, one per agent, each with its own Step Timeline. The plan card above ticks items off live. Presence stack shows four.
**Route:** C04 Build.

**C06 — Sessions and external agents.** Session switcher open: Running (Onboarding screens · 4 agents; Claude Code · external), Needs you (Paywall variants), Recent (Token cleanup). Second state: the **Claude Code external session** selected — its steps with thumbnails and the composer replaced by "Chat with Claude Code in its own window · Stop".
**Route:** session switcher.

**C07 — Alternatives.** Two ways to get options, shown as two states. **Side by side:** Sam asked for "a paywall" with Parallel 3× Side by side, choosing a different model for the third lane; three frames labelled A, B, C sit side by side under a group tag, each with its agent's marker and model. **Explore:** Sam asked for "a paywall" with Explore 4 · Style; the first result and three successive style variants appear in a row, each labelled with what changed ("Warmer palette, serif headings"). In both, the Agent panel turn shows the thumbnails with **Keep** per option and a **Keep B, discard others** primary; hovering a thumbnail highlights its frame.
**Route:** request with Parallel Side by side or Explore.

**C08 — Steering with selection.** Sam selects a card and two buttons in Step 2. The selection toolbar shows **Ask**; the inline prompt is open beside the selection with "Make these feel less crowded" typed. The Agent panel composer shows the selection chip "3 layers in Step 2". Second state: the inline prompt after sending, showing a progress line and Undo, with the changed layers tinting.
**Route:** select layers → Ask or ⌥K/⌘K.

**C09 — Turn details and undo.** A finished turn with the footer "Made 47 changes · 2 frames · 1 m 12 s · 38k tokens", a **Checked screenshot** step expanded (image plus verdict "Fixed: CTA text overflowed at 375 px"), a failed step with Retry step, and the **Undo this turn** confirmation popover ("Removes 47 changes from Teal's turn. Your edits since then are kept.").
**Route:** expand steps, Undo this turn.

**C10 — History.** History tab full height: entries from four agents, Sam and one external file change; one entry expanded with operations and before → after values; actions visible; filtered by session "Onboarding screens". Second state: Revert confirmation explaining that later changes depend on this one.
**Route:** ⌥4 or Review changes.

**C11 — Engine and model.** Picker opened from the composer footer. Two groups: **Built-in** (provider and model in mono, e.g. OpenRouter `deepseek/deepseek-v4.1-flash`, Anthropic, OpenAI, Google, DeepSeek, xAI, Local), and **Your installed agents** (Claude Code, Codex — runs that tool on your computer under your own account, shown with "Installed · signed in"). Each option shows capabilities as words: Vision, Fast, Large context. A chevron beside a model opens **Reasoning effort** (Low, Medium, High) where the model supports it. A note warns when switching provider mid-session means the new model won't see earlier turns. **Add provider** opens setup: a provider list (OpenRouter, Anthropic, OpenAI, Google Gemini, DeepSeek, xAI, Moonshot / Kimi, Together, Fireworks, Z.AI, OpenCode, AWS Bedrock, Google Vertex, Microsoft Foundry, Local, **Custom endpoint** with its API style: Chat Completions, Responses or Anthropic Messages), then the API key field (masked, paste, "Stored in your system keychain. Never written to your files or sent to us."), Test and continue. Each configured provider shows a status in words — Connected, API key set, Signed in, Configured outside Vellum (not tested), Failed — with **Re-check** and **Clear key**.
**Route:** composer footer → picker.

**C12 — Notes for agents and guidelines.** Right panel Design tab with a frame selected, showing the **Notes for agents** section at the top: free-text instructions stored with the frame in the file ("Copy under 10 words per line. Use the illustration slot, don't draw one.") with a line "Agents read these before changing this frame". Second state: the design system's **Guidelines** editor (opened from Assets), a readable Markdown editor of the rules every agent reads first.
**Route:** select a frame; Assets → design system → Guidelines.

### Editor: refining by hand

**C13 — Frame with auto-layout selected.** Right panel Design tab: Notes for agents (collapsed when empty), Position, Layout (Freeform / Row / Column / Grid, gap, padding linked/unlinked, alignment matrix, Hug / Fill / Fixed per axis), Appearance (opacity, radius, clip), Fill, Stroke, Effects, Export. Bound values show the Variable chip ("`{space/4}` 16"). Selection toolbar above the frame with **Ask**.
**Route:** select a frame.

**C14 — Text editing.** Text layer in edit mode; Text section with style picker, font, weight, size, line height, letter spacing, alignment and resizing. Missing-font state in amber.
**Route:** double-click text.

**C15 — Vector editing.** Pen tool editing a path: anchors, handles, active point; point coordinates and corner radius in the right panel; a "Done" pill.
**Route:** select vector, Enter.

**C16 — Components and variants.** A main component selected: solid violet outline, filled component icon in its label, its properties and variants. Second state: an instance selected — thinner violet outline with the instance icon, overrides listed with Reset, **Go to main component** (or **Open library** when it comes from one), **Replace inside instance**, **Detach** (nested instances and variable links stay linked). Third state: the Components list in Assets with search, click or drag to insert.
**Route:** select a component or instance.

**C17 — Variables.** Large dialog: collections (Color, Space, Radius, Type), variables as rows and **modes as columns** (Light, Dark), inline editing, aliases shown as Variable chips. Toolbar: New variable, New mode, Import from CSS, Export, **Ask agent** (opens the composer with the variables as context).
**Route:** Assets → Variables or palette.

**C18 — Layers and Assets tabs.** Layers: pages list, layer tree with type icons, a small agent marker beside layers created in the last 10 seconds, visibility and lock on hover. Assets: search, then sections for document components (drag to canvas), libraries, **Icons** (Lucide, Material Symbols, Phosphor, Feather and the project's own SVG folder; search, choose set and weight, drag in as vectors), **Styles** (visual styles agents can apply) and starter design systems with "Add to document" and "Guidelines".
**Route:** ⌥2, ⌥3.

### Editor: code and files

**C19 — Inspect.** Right panel Inspect tab for a frame: spec (layout, sizing), tokens used with resolved values per mode, components used with props, and a code preview with a **Stack** selector (CSS, Tailwind, shadcn, React Native). Primary: **Copy prompt for your coding agent**; secondary: **Implement with…** (sends the frame to an installed agent such as Claude Code to write the code in your repo). A **Code** section exports deterministic **HTML + Tailwind** or **HTML + CSS**: Copy HTML, Export HTML, and a settings popover (include a full HTML page scaffold; inline assets or write them as files).
**Route:** Inspect tab.

**C20 — Token sync.** Dialog for Import or Export: file path (mono), format (CSS variables, Tailwind v4, Design Tokens JSON), mode mapping (Light → `:root`, Dark → `.dark`) and a diff preview of added, changed and removed tokens. Apply / Write file; the toast names the file written.
**Route:** Variables dialog or palette.

**C21 — Export.** Export section plus a multi-frame export dialog: format (PNG, JPEG, WEBP, SVG, PDF; PDF puts each frame on its own page in selection order), scale (1×, 2×, 3×), suffix, destination folder, preview strip of file names in mono. Right-click **Copy as PNG** puts an image on the clipboard.
**Route:** Export section or ⇧⌘E.

**C22 — File changed on disk.** Banner: "juniper.vellum changed on disk (git checkout) · 14 changes applied · View in History". Variant: merge conflict node with an amber dashed outline and tag "Conflict · 2 versions", resolver popover (Ours / Theirs previews: Keep ours, Keep theirs, Keep both, **Ask agent to merge**).
**Route:** external change detected.

### Editor: utilities

**C23 — Command palette.** ⌘/ overlay searching actions, layers, pages, components, variables and sessions; the first result is always "Ask the agent: <your query>" so any palette search can become a prompt.
**Route:** global.

**C24 — Context menu.** Right-click on layers: **Ask about this…** and **Redesign this…** at the top, then Copy / Paste; Group, Frame selection, Add auto-layout; Create component; arrange; Copy as CSS, Copy node ID; Delete at the bottom. Shortcuts right-aligned in mono.
**Route:** right-click.

**C25 — Editor settings.** Appearance (Follow editor theme / Light / Dark / High contrast; Match editor colours), Agents (default engine, model and effort, default Parallel and mode, plan threshold, let external agents start helper agents, notifications, auto-open History on external agent edits, insert highlight), Permissions (repo reads, file writes, commands: Ask or Allow; engine auto mode where supported; Skip all with its warning), Skills (added `SKILL.md` folders with Remove), Assets (icon sets, stock image provider and key, image generation model), Canvas (backdrop for this document; show pixel grid; snap to pixel grid; snap to objects; scroll wheel zooms; invert zoom direction; small and big nudge amounts, also used for path points; animations and effects on/off to save battery), Renderer (Automatic / Basic), Language, Telemetry (**off by default**, with exactly what is sent if on), Keyboard shortcuts with host conflicts marked.
**Route:** menu or palette.

### Editor: agent tooling and content

**C26 — Skills menu.** The composer with **/** typed: a menu grouped Built-in (Design a screen, Build a design system, Recreate from code, Make a chart with a script), Design system (Starter Minimal guidelines) and Yours (two added skills with their folder paths in mono), plus **Add skill…** at the bottom. Second state: a chosen skill as a chip above the field.
**Route:** type / in the composer.

**C27 — Permission card.** A turn paused on "Teal wants to read `DESIGN.md`" with Allow once / Allow for this session / Deny, the session dot amber in the switcher, and the usage popover open beside the composer (context ring, tokens by kind, "Estimated cost at API rates · $0.04").
**Route:** agent requests a file or command.

**C28 — Script node.** A script node on the canvas rendering a bar chart for Juniper's weekly stats. Right panel: **Script** section with the file path (`charts/weekly.js`, mono, Open in editor), the inputs from the file's header as controls (Values, Bar color bound to `$color/primary`, Gap), and **Convert to layers**. Second state: a script error shown on the node itself (red outline, message and line number) and in the panel with the code excerpt. Third state: an empty script node with "Choose a script file or ask the agent to write one".
**Route:** insert a script node from the Rectangle tool menu, or an agent creates one.

**C29 — Slots.** A `Card` component with a slot: the slot drawn with a dotted inner outline and a "Slot" label, its suggested components (`HabitRow`, `StatTile`) listed in the right panel with + to add more. Second state: an instance selected, its slot filled with two `HabitRow` instances, the panel offering the suggestions as one-click inserts.
**Route:** select an empty frame inside a component → Make slot.

**C30 — Images.** Image fill on a selected frame: tabs **Upload**, **Stock** (search results grid with attribution) and **Generate** (prompt field, model, Generate; v1.1). Second state (v1.1): an image layer selected with **Remove background** and **Convert to vectors**.
**Route:** Fill → Image, or the Insert menu.

**C31 — Import (v1.1).** Desktop only: an import panel with a built-in browser view loading `http://localhost:3000` (address in mono, device size picker), **Select an element** mode highlighting a card on the page, and **Import as layers** / **Screenshot**. Second state: Figma file import progress with a summary of what couldn't be converted (Figma layer paste works in v1 but without images; the summary says so). Third state: an **embed** node on the canvas showing the live localhost page at a phone size, with URL, device and zoom in the right panel.
**Route:** File → Import, or an agent's `browser` step.

**C32 — Slides (v1.1).** A page of kind Slides: a vertical slide strip with numbers, the canvas showing slide 3, **Present** in the top bar, and Export as PDF (slide order).
**Route:** New page → Slides.

### Editor: visual detail

**C33 — Fill and effects.** The fill editor open from a swatch, with type tabs **Solid**, **Gradient** (Linear, Radial, Angular, **Mesh**: grid size, points movable on canvas, point colours), **Image** (Fill, Fit, Stretch; crop handles on canvas; replace from file, URL or stock) and **Shader** (choose a shader file, its uniforms as controls, bindable to variables; **Examples…** opens the examples gallery; v1.1). Per paint: visibility, opacity, blend mode menu, remove. Second state: the Effects section with Drop shadow, Inner shadow, Layer blur, Background blur and Shader effect (v1.1) rows; and the Stroke section with per-side widths, Inside / Center / Outside, caps, joins and dashes.
**Route:** select a layer → Fill, Stroke or Effects.

**C34 — Themes.** The Variables dialog with two theme **axes**: Mode (Light, Dark) and Device (Phone, Tablet), shown as grouped columns; **+** beside a column adds a value, **+** beside the axis tabs adds an axis. Second state: a frame selected with the **Theme** section in the right panel ("Mode: Dark · Device: inherits Phone") and the canvas showing that frame in dark while its siblings stay light. A line explains that children inherit unless they set the same axis.
**Route:** Variables dialog; select a frame → Theme.

**C35 — Notes, context and prompts on the canvas.** Beside the Onboarding frames: a sticky **note** for people ("Check copy with Lena"), a **context** block for agents ("All onboarding screens: one primary action, max 2 lines of body text") with a small agent-marker icon showing it's read by agents, and a **prompt** node ("Make a tablet version of Step 2") with Run, the engine and model it will use, and its last run time. The Agent panel shows the context block as an automatic context chip when a nearby frame is selected.
**Route:** N for a note; Insert menu for context and prompt.

**C36 — Libraries.** Assets → Libraries: this document's imported libraries (with version date, Open, Remove), **Import library…** for a `*.lib.vellum` file, the bundled design systems, and for the current file **Turn this file into a library** with a confirmation that it can't be undone. Second state: the paste dialog from S15.
**Route:** Assets → Libraries.

**C37 — Custom fonts.** The font picker with search, sections for Recently used, This document's fonts, Google Fonts, System fonts; **Add font…** imports TTF, OTF, WOFF or WOFF2 into the document ("Stored with this file, so it travels with it"). **Manage fonts** lists custom fonts with where they're used and Remove. A missing-font row shows the substitute and **Replace…**, with a reminder to check text wrapping.
**Route:** Text section → font field.

**C38 — Scale.** K pressed with a card selected: a scale panel with anchor grid, factor field ("2" or "200%"), resulting W and H, and a note "Bound sizes become fixed values; instances get overrides." Corner handles scale proportionally; Enter applies and returns to Move.
**Route:** K, or Object → Scale.

### Desktop shell

**A01 — Welcome.** First launch: wordmark, "Design where your code lives.", a composer ("What do you want to design?") with a folder picker, and Open a folder / Connect your coding agent.
**Route:** first launch → editor with the prompt carried into C01.

**A02 — Home.** Composer at the top; then tabs **Recents** (files and drafts), **Drafts** (auto-saved documents not yet in a project, with Save As… to move one), **Design systems** (bundled libraries to start from) and recent folders. Grid view with thumbnails or list view sortable by Name, Last opened, Created; ⌘-click to multi-select; right-click Remove from recents (keeps the file) or Delete draft (permanent, confirms). Drop a `.vellum` file onto the window to open it. Each file shows folder path in mono, branch, last opened and a dot if a session is running. Empty state: "Open a folder to see its design files."
**Route:** app open without a document.

**A03 — Editor window with tabs.** C03 inside the desktop window with three document tabs, one Unsaved, one with a running-session dot.
**Route:** open files.

**A04 — Agents.** Settings page: daemon state, built-in engines and keys, installed agents detected, connected MCP clients with last activity (Reconnect, Remove), usage this week per engine (tokens), Pause all agents.
**Route:** Agents menu → Manage.

**A05 — Sign in (Pro).** What syncs ("Copies of files you choose to share, never your whole repo"); device-code variant for the CLI.
**Route:** Share or menu.

**A06 — Recovered changes.** After an unexpected quit, reopening a file shows a dialog: "Vellum found unsaved changes to juniper.vellum from 14:32", a before/after thumbnail strip of affected frames, and **Save** (to the original), **Save As…**, **Discard backup** (confirms) and **Cancel**. A line notes that recovery copies aren't history and Git is.
**Route:** reopen after a crash.

### Web (Pro)

**W01 — Share link viewer.** Read-only canvas: frame list, canvas, minimal top bar (name, owner, "Snapshot from 11 Oct, 14:32", View only, **Download** as a ZIP that opens in Vellum, Open in Vellum), Inspect for developers.
**W02 — Share dialog** (in the editor). States: **Not shared** (what will be uploaded — the file, its images, fonts, scripts and libraries — and Create share link); **Checking** (a missing-files warning listing paths, with Restore and try again or Share anyway, which marks the snapshot incomplete); **Uploading** (progress; it continues if the dialog is closed); **Shared** (link with Copy, "Snapshot from 14:32", Delete share link with a note that downloaded copies stay with their owners); **Changed since shared** ("New changes since 14:32" with **Update snapshot**); **Upload failed** (Retry). Invites by email with roles come with Team (v1.1).
**W03 — Account and plan.** Plans described by what they do, billing, devices and CLI sessions with Revoke, shared files.

### System and edge states

**S01 — Daemon offline.** Bottom strip "Daemon offline"; the composer is disabled with "Agents need the Vellum daemon · Restart"; hand editing and saving still work.
**S02 — Basic renderer.** "Basic renderer" note: effects are simplified on screen; exports unaffected.
**S03 — Missing fonts.** Amber count; popover listing fonts, usage, substitutes, Install from Google Fonts, Choose replacement.
**S04 — File from a newer version.** Read-only with banner "Made with a newer version of Vellum (format 3) · Update to edit". The composer is disabled with the same reason.
**S05 — File can't be read.** Error in plain words with line and column, Open as text, link to the format spec. No partial rendering.
**S06 — Large file.** Quiet suggestion to use the multi-file format; never blocks.
**S07 — Agent write rejected.** Lease tab count, History entry, and a failed step in the turn with the reason ("Frame leased by Saffron").
**S08 — Key or engine failed.** In the turn: "OpenRouter rejected the key · Update key", or "Claude Code isn't signed in · Open terminal to sign in". Other sessions are unaffected and the message says so.
**S09 — Usage limit reached.** For installed agents with plan limits: "Claude Code reached its usage limit · resets at 18:00 · Switch engine".
**S10 — Agent stuck.** A step running over 2 minutes shows "Still working on this step" with Stop and Retry step; never an endless spinner without words.
**S11 — Unsaved and closing.** Host dialog naming the file; if agents are running: "3 agents are still working. Stop them and close?"
**S12 — Empty states.** History ("Changes from you and your agents appear here"), Layers ("Ask the agent or press F to draw a frame"), Assets, Variables, Sessions (only in the switcher when a document has none). Each has one next step and no illustration.
**S14 — Missing library.** A document whose imported library path no longer resolves: Assets → Libraries shows it as missing with **Locate…**; instances keep rendering from the cached copy, marked with a small amber badge on their labels. After locating, a toast offers to save the document.
**S15 — Paste conflicts.** Pasting components from a library the document hasn't imported asks **Link to library** or **Paste as copies**; pasting variables whose names already exist asks per variable **Use existing** or **Add renamed**. A note lists anything not carried over (custom fonts, missing images).
**S16 — Agent needs sign-in.** An installed engine that isn't signed in: the engine row in C11 and the failed turn both say "Claude Code isn't signed in · Run `claude` in a terminal to sign in" with **Re-check**.

**S13 — Narrow editor.** C03 at 960px: agent panel at 320px, right panel as overlay; and at 760px (IDE split): agent panel collapsed to a strip, composer opened with ⌘L as an overlay.

## Terminal output (CLI)

- One line per meaningful result with a status glyph: `✓` done, `!` warning, `✗` error. Colour only reinforces the glyph and is off for non-TTY output or `NO_COLOR`.
- `vellum prompt` streams the same steps as the Agent panel, one per line: `✓ Designed Step 2 · 31 layers · 18 s`, ending with the turn footer line.
- Errors say what failed, why and what to do: `✗ juniper.vellum is format 3; this CLI reads up to format 2. Update with: npm i -g @vellum/cli`.
- `--json` on every command. No spinners in non-interactive output.

## States and interaction rules

Every flow accounts for: empty, loading, planning, waiting for plan approval, waiting for permission, working (single and parallel), streaming, alternatives pending a choice, script running or failed, done, failed step, stopped by you, paused, write rejected, external agent connected, engine or key failed, usage limit, daemon offline, read-only, external change, conflict, basic renderer and missing fonts.

**Agent panel rules:**
- Streaming must not make the panel jump: keep the reading position if the person has scrolled up, and show a "Latest" pill to jump down.
- Sending while an agent works asks: **Queue after this turn** (default) or **Stop and send now**.
- Stopping a turn keeps everything already built, marks the turn "Stopped by you", and offers Undo this turn.
- The composer keeps draft text per session when switching sessions.

Controls have visible default, hover, focus, pressed, selected, disabled, mixed ("Mixed" for multi-selection), loading, success and failure states. Value fields support scrubbing, arrow keys (±1, ⇧ ±10) and expressions ("16*2"); invalid input reverts with a brief message beside the field.

Everything is keyboard reachable with a visible focus ring. The canvas keyboard mode: Tab cycles layers within the selection's parent, Enter goes into a frame, Esc goes out, arrows nudge. The Layers tab is the canvas's accessible tree. Never rely on colour alone.

**Right-to-left UI.** In Arabic, panels, menus and the agent conversation mirror; **the canvas and design content never mirror**, and numeric value fields, code and paths stay left-to-right. Use logical properties throughout.

## Component library

- **Agent Panel**: tab bar, session bar and switcher (Running / Needs you / Recent), your message with context chips, agent header (single, parallel stacked), agent prose, Step row (working, done, warning, failed, grouped), Designed step with thumbnail, Screenshot-check step, Layout-check step, Ran-script step with script reveal, Plan card (editable items, Parallel stepper with mode, Build), parallel turn with lanes and progress line, Alternatives turn (Side by side and Explore) with Keep actions, turn footer with usage, Undo-this-turn popover, external session header and composer replacement, Latest pill.
- **Composer**: field, context chips (selection, page, file, frame, component, variable, skill), @ mention picker, / skills menu, attach, Parallel stepper with Split / Side by side, Explore control (Layout / Style, 2–6), Engine and model picker with effort, usage meter and popover, Send/Stop (Stop this agent / Stop all), queue-or-stop prompt; **Inline prompt** variant.
- **Permission card** (read file, write file, run command; allowed, denied, waiting) and the **Skip all** warning line.
- **Script node** (rendering, error, empty), **Script section** with generated input controls and Convert to layers; **Slot** marker and suggested-components list; **Icon picker**; **Image fill** tabs (Upload, Stock, Generate).
- **Paint editor** (solid, linear / radial / angular / mesh gradient, image with crop, shader), **blend mode menu**, **effect rows**, **stroke section** with per-side widths; **Theme section** and **theme axis columns**; **Scale panel**; **font picker** with custom fonts and **Manage fonts**.
- **Canvas nodes for collaboration**: sticky note, context block, prompt node with Run, embed (v1.1).
- **Library list** (imported, missing with Locate, bundled), **paste dialogs** (link or copies; variable conflicts), **Share dialog** states, **Recovered changes** dialog, **provider status row** (Connected, API key set, Signed in, Configured outside Vellum, Failed).
- **Agent Presence**: presence stack and menu, placeholder frame, cursor and name tag, lease outline and tab (working, idle, you're editing here too, rejected count), insert highlight, paused banner, follow badge, selection toolbar with Ask.
- **Change Ledger**: entry (agent, you, teammate, file, rejected), operation rows with before → after, actions, revert popover, filters.
- **Actor Marker**: rounded square (agents, 8 hues) and circle (humans), sizes 16/20/24/32; stacked group.
- **Floating chrome**: document bar, top-right bar, tool rail, bottom strip, zoom pill.
- **Layers row**, **property controls** (value field with scrub/expression/mixed/bound, padding, alignment matrix, segmented control, colour well and picker with variables tab, sizing menu, section header), **Notes for agents** field, **Variable chip**, **Variables table**, **component property row**.
- **Spec block**, **code block** with stack selector, **diff list**, **Guidelines editor**.
- **Command palette** (with "Ask the agent" row), **context menu**, **dialog**, **banner**, **toast**, **empty state**.
- **Desktop home**: composer, file card, folder row, document tabs.

## Product and design constraints

### Content and tone

- Consistent verbs: Build → Building → Built; Stop → Stopped by you; Pause all → Agents paused; Undo this turn → Undone by you; Keep → Kept variant B.
- Agents speak in first person, briefly and factually ("I've built Step 2. The chart uses placeholder data until you connect real numbers."). No exclamation marks, emoji, apologies for normal behaviour or claims of taste ("stunning", "beautiful").
- Built-in agents are named by hue ("Teal"); external agents by client ("Claude Code"). Never personas.
- System lines are third person and attributed ("Saffron's change rejected · frame leased by Teal").
- Say "your file", "on your computer", "in your repo". Only say "cloud" for things synced by Pro.
- Never claim agents produce correct or finished designs; where relevant, say "Review agent changes in History."
- One example project everywhere:

| Item | Example |
|---|---|
| Person | **Sam**, design engineer |
| Project | **Juniper**, a fictional habit-tracking app |
| Repo path | `~/code/juniper`, files `design/juniper.vellum` and `DESIGN.md` |
| Branch | `feat/onboarding` |
| Pages | Onboarding (Welcome, Step 1–3), Settings, Paywall, Components |
| Design system | Starter Minimal: `color/primary`, `color/surface`, `space/4`, `radius/md`; modes Light and Dark |
| Agents | Teal and Saffron (built-in, `deepseek/deepseek-v4.1-flash`), Rose and Green in the parallel build, Claude Code (external, Magenta) |
| Skills and scripts | Built-in skills; Sam's own `~/skills/juniper-voice/SKILL.md`; script `design/charts/weekly.js` |
| Dates | Plausible times in the second week of October 2026 |

- Never invent testimonials, usage statistics or customer logos.

### Imagery and assets

No photography or illustration in the product chrome. The only images are the person's designs, step thumbnails, screenshots agents took, file thumbnails and imported assets. Example designs must be generic, original UI for the fictional Juniper app. Vellum's own chrome must be an original design: do not reproduce another design tool's layout details, icons, labels or styling one-to-one.

### Build consistency

Centralise colour, type, spacing, radius, elevation, focus and motion as tokens with light, dark and high-contrast values, shared by editor, desktop shell and web viewer. Chrome tokens and canvas tokens (selection, component, agent hues, placeholders, canvas labels) are separate groups so the chrome can follow the host theme without changing canvas meaning. Use logical properties for RTL.

### Quality floor

- No UI text below 11px; panel body at 12px; agent prose at 14px with 1.55 line height.
- Text contrast at least 4.5:1 in all appearances, small text included; essential icons and focus rings at least 3:1.
- Cursors, lease tabs and placeholder labels legible over light, dark and busy fixture designs.
- Every state is labelled in text; nothing relies on colour alone.
- Editor layouts at 760×640 (IDE split), 960×640, 1280×800 and 1512×982, in the IDE webview and the desktop window.
- Respect keyboard focus, accessible names (agent turns and steps are announced as they complete, politely), OS text scaling for chrome, and reduced motion.
- Copy, state names and shortcut labels are identical across IDE, desktop, web and CLI.

## Deliverables

1. **Token sheet** (light, dark, high contrast; chrome and canvas groups) and **component library** covering every component above with all states.
2. **Editor screens** C01–C38 (C31, C32 and the shader parts of C33 are v1.1 and can follow later), **desktop** A01–A06, **web** W01–W03 and **edge states** S01–S16, light and dark (C03, C05, C10 also high contrast), at 1280×800, plus C03 and C05 at 1512×982 and S13 at 960×640 and 760×640.
3. **Prototype flows:** spec to screens (C01 → C04 → C05 → C10), steer with selection (C08 → C09 undo), alternatives (C07), skill to permission (C26 → C27), script chart (C28), external agent (C02 → C06), and design to code (C13 → C19 → Implement with…).
4. If producing HTML: a root `index.html` gallery linking every screen, shared `assets/vellum.css` for tokens and components, and one file per screen named `screen-C03-agent-building.html`, `screen-C05-parallel-build.html`, and so on.
