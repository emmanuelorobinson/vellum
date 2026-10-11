# Vellum — Product Description

> **Vellum** is a working codename.
> **One-liner:** An open-source design canvas that lives in your repo, where you and your coding agents design side by side.

---

## 1. The problem

AI coding agents now write most of the UI code for many developers. Design hasn't caught up:

- **Design lives somewhere else.** Figma files sit in a vendor's cloud, out of reach of the agent writing the code. Handoff is still screenshots, links and copy-paste.
- **Agents design blind.** Asked for "a settings screen", an agent guesses in code, renders it, and iterates. It has no canvas to sketch on and no spatial picture of what it's building.
- **Design isn't versioned with code.** Roll back a branch and the design doesn't roll back with it.
- **The new tools are closed.** pen.dev (formerly Pencil) proved the model: a vector canvas inside the IDE, designs stored as JSON in the repo, and an MCP server so agents can edit them. But it's proprietary, its format is defined by one vendor, and pricing is "free for now".

Developers who design, and designers who code, want the pen.dev workflow on terms they control: an open format, an open-source editor, their own model, and collaboration they can host themselves.

## 2. The product

**Vellum is a vector design canvas that runs in your IDE, on your desktop and in the browser, with designs stored as plain, diffable files in your repo.**

You install the extension in VS Code, Cursor or Windsurf (or open the desktop app). You create `home.vellum` next to your code. You draw, or you tell your coding agent "design an onboarding flow in home.vellum", and watch it build the screens live on the canvas. Then you tell it to implement them. Tokens stay in sync with your CSS. When the code changes, the design can be updated from it.

The engine is a Rust core compiled to WebAssembly, so the same code runs in the IDE, the desktop app, the browser, the CLI and on servers.

### Positioning

|  | pen.dev | Figma | Penpot | **Vellum** |
|---|---|---|---|---|
| Where you design | IDE extension, desktop | Browser, desktop | Browser | **IDE, desktop, browser, headless CLI** |
| Where files live | `.pen` JSON in your repo | Figma's cloud | Penpot server | **Deterministic JSON in your repo** |
| Format | Vendor-defined | Proprietary | Open (SVG-based) | **Open, versioned, published spec** |
| Agent access | Local MCP server (app open), CLI | MCP server (Dev Mode) | Plugins | **MCP server and CLI, with the app open or closed** |
| Built-in agent | Integrated agent; Claude, Codex and other providers | Figma's AI | — | **Any model (OpenRouter, Anthropic, OpenAI, local) or your installed Claude Code / Codex** |
| Account needed for local use | Yes (agent, export, CLI) | Yes | Yes (server) | **No** |
| Multiplayer | Yes (vendor cloud) | Yes (vendor cloud) | Yes (self-hostable) | **Yes: our cloud or your own server** |
| Open source | ❌ | ❌ | ✅ | ✅ (core + editor) |

**Tagline options:**
- "Design where your code lives."
- "A canvas for you and your agents."
- "Open design files. Open editor. Your repo."

---

## 3. Who it's for

1. **Solo founders and indie developers.** They design their own products, already use Claude Code / Cursor / Codex, and want designs that agents can read and write.
2. **Design engineers and small product teams.** They want design versioned with code, tokens synced to CSS, and review in pull requests.
3. **Open-source projects.** They need design files contributors can open without a paid seat.
4. **Privacy-sensitive and regulated teams.** They need collaboration on their own servers (GCC, EU), not a US vendor's cloud.

**Beachhead:** groups 1 and 3. They already live in the IDE, they'll tolerate beta, and they'll talk about it on X, HN and GitHub.

---

## 4. Core concepts

| Concept | What it is |
|---|---|
| **Document** | A `.vellum` file. Deterministic JSON, one per feature or app area, committed to Git. |
| **Page** | A tab inside a document, holding an infinite canvas. |
| **Frame** | A container that can use auto-layout (flexbox/grid semantics). Screens are top-level frames. |
| **Node** | Anything on the canvas: frame, text, shape, vector path, image, component instance. |
| **Component** | A reusable node tree with props and variants. Instances can override properties. |
| **Library** | A shared document of components and variables, referenced by other documents. |
| **Variable** | A named value (color, number, string, boolean) with modes such as light/dark. Maps 1:1 to design tokens and CSS variables. |
| **Design system** | A library plus guidelines (written rules agents read before designing). |
| **Agent** | Any MCP client editing the document (Claude Code, Cursor, Codex, the built-in agent). It gets a name, a colour and a live cursor. |
| **Lease** | A frame an agent is currently working on, shown on the canvas, so several agents can work on different screens without colliding. |
| **Session** | A conversation with the built-in agent (or the record of an external agent's work) for one file, stored on your computer, never in the file. |
| **Skill** | Instructions agents read before working: built-in, from a design system's guidelines, or your own `SKILL.md`. |
| **Style** | A visual style (palette, type, radius, density) agents can apply when there's no design system yet. |
| **Slot** | An empty area inside a component that each instance fills with its own content, with suggested components. |
| **Script node** | A node drawn by a `.js` file beside the design (charts, grids, patterns), with inputs you tweak in the panel. |

---

## 5. Key experiences

### 5.1 Start in under a minute
1. Install "Vellum" from the VS Code Marketplace or Open VSX (or download the desktop app).
2. Create `design/app.vellum`. The canvas opens in an editor tab.
3. The extension offers to register its MCP server with the agents it detects (Claude Code, Cursor, Codex). One click.
4. Pick a starter design system or import tokens from your existing CSS / Tailwind config.

### 5.2 Design with your agent
- Ask in your agent's chat: "Design a 3-step onboarding flow in app.vellum using our tokens."
- The agent reads the design system guidelines, claims a frame, and builds it. You watch nodes appear in real time, with the agent's cursor and name on the canvas.
- Several agents (or several sessions of one agent) can work on different screens at once.
- The built-in agent panel does the same with a model of your choice, or with your installed Claude Code or Codex: plan first, then 1–6 agents in parallel (splitting the work, or each building its own alternative), with skills, permissions for anything outside the canvas, and per-turn usage and undo.
- Agents use one `execute` call to make many changes at once (a short script), or simple typed tools; either way every change is checked, attributed and undoable.

### 5.3 Design by hand
- A full vector editor: frames, auto-layout, constraints, pen tool, boolean operations, text styles, components and variants, prototyping links between frames.
- Keyboard shortcuts that match what designers expect from Figma.
- Paste from Figma (SVG and text/layout where possible) and import images.

### 5.4 Design to code, code to design
- **Design → code:** the agent reads a frame through MCP as a structured spec (layout, tokens, components) and writes the implementation in your stack (React + Tailwind, shadcn, React Native, SwiftUI).
- **Code → design:** the agent reads your existing components and recreates them on the canvas, so an existing app gets a design file without starting over.
- **Token sync:** variables export to CSS variables and Tailwind `@theme`, and import back. Deterministic, no AI needed.

### 5.5 Version and review with Git
- Files are pretty-printed with stable IDs and stable key order, so diffs are readable.
- A GitHub Action renders before/after images of changed frames and posts them on the pull request.
- Merge conflicts in `.vellum` files are resolved by a Git merge driver that understands the format.

### 5.6 Collaborate live
- Share a document with teammates for live multiplayer editing, comments and version history.
- Use our cloud, or run the open-source sync server on your own box.

### 5.7 Headless
- The `vellum` CLI creates, edits and exports documents without any app open: `vellum export app.vellum --frame Login --png --scale 2`.
- The same binary is the MCP server, so agents in CI or on servers can design too.

---

## 6. Business model

**Open core.** The editor, the file format, the CLI and the MCP server are free and open source. We charge for the cloud and for teams.

| Plan | Price (starting assumption) | Includes |
|---|---|---|
| **Free** | $0 | Editor (IDE, desktop, browser for local files), CLI, MCP server, unlimited local files, community design systems |
| **Pro** | ~$10/mo | Cloud sync across devices, share links and a web viewer, version history, hosted PR previews, premium design systems |
| **Team** | ~$12/user/mo | Live multiplayer, comments, shared libraries, roles, SSO later |
| **Self-hosted sync** | $0 (open source) | Run the sync server yourself. Paid support later. |

The built-in agent uses the user's own model key (BYO) or their own installed Claude Code or Codex subscription. We never mark up tokens in v1.

Future revenue: a design-system and template marketplace with a revenue share for creators, enterprise support, and possibly included hosted models for people who don't want to bring their own key (pen.dev offers this with its paid plans; never required in Vellum).

---

## 7. v1 scope

**In**
- Rust/WASM core: document model, auto-layout, text, hit-testing, undo/redo
- VS Code extension (VS Code, Cursor, Windsurf via Open VSX)
- Desktop app (macOS first, then Windows and Linux on x64 and ARM64), with home screen, drafts, crash recovery and auto-update
- Editor: frames, auto-layout, shapes (incl. arcs and polygons), pen tool, scale tool, text (incl. links, underline, vertical alignment), images with crop, gradients incl. mesh, blend modes, shadows and blurs, components + variants + slots, variables with aliases and theme axes (e.g. Light/Dark × Phone/Tablet) and per-frame themes, libraries, custom fonts carried with the file, layers panel, properties panel, full keyboard shortcut set
- Canvas notes for people, context blocks and prompt nodes for agents
- MCP server + CLI (one binary), working with the app open (live) or closed (headless)
- Agent panel: sessions, plans, Parallel (Split / Side by side), Explore variants, skills, styles, permissions, usage and estimated cost; engines: built-in (BYO model), installed Claude Code, installed Codex
- MCP tools covering everything pen.dev's server offers (see ARCHITECTURE §7.4), including `execute`, `get_app_state`, `read_skill`, `get_style`, `spawn_agents`
- Script nodes, slots, icon libraries, stock images
- HTML + Tailwind / HTML + CSS export
- Token import/export (CSS variables, Tailwind v4)
- 3 starter design systems
- PNG / JPEG / WEBP / SVG / PDF export
- Published file format spec (v1)
- Share snapshots (Pro): link, update, revoke, ZIP download
- One-click MCP setup for Claude Code, Codex/ChatGPT, Cursor, Windsurf, Copilot, Gemini CLI, Antigravity, OpenCode, Kiro and Claude Desktop; providers from OpenRouter and Anthropic (incl. Bedrock, Vertex, Foundry) to custom endpoints

**Out (for now)**
- Live multiplayer and comments (v1.1, with the Team plan)
- Browser editor for cloud files
- Prototyping and interactions beyond simple frame links
- Figma file import (beyond paste), web page import and live website embeds through a built-in browser (`browser` tool), AI image generation and editing, background removal and image tracing, shader fills and effects with an examples gallery, slides and presenting (all v1.1)
- Plugins API
- Mobile app (a read-only viewer later)

---

## 8. Risks and answers

| Risk | Answer |
|---|---|
| pen.dev or Figma ships the same features first | Being open source and self-hostable is the part they're unlikely to copy. Win the open-source and privacy-minded segment. |
| Building a vector editor is very hard | Rust core with proven libraries (Taffy, Parley, Skia), narrow v1 scope, hand-editing tools grown after agent tools. |
| Legal exposure from cloning a competitor | Clean-room: our own name, UI, file format and code. No reverse-engineering of their binaries or format. Interop importers only after legal review. |
| Agents produce mediocre designs | Guidelines docs, strong starter design systems, a `screenshot` tool so agents can check their own work, and BYO model so users pick quality. |
| Git conflicts in design files | Deterministic serialization, stable IDs, and a custom merge driver. |
| Canvas performance on large files | Rust core, R-tree spatial index, GPU rendering, tile caching. Benchmarks in CI. |

---

## 9. Success metrics

- **Activation:** % of installs that create a document and get an agent to edit it within 24 h.
- **Retention:** weekly active documents; documents with commits in 2+ different weeks.
- **Agent usage:** MCP tool calls per active user; % of frames created by agents vs by hand.
- **Conversion:** Free → Pro. Track "shared a link" and "opened on a second device" attempts.
- **Community:** GitHub stars, design systems shared, third-party tools reading the format.

---

## 10. Launch narrative

> pen.dev showed that design belongs in the repo, next to the agents writing your code. Vellum makes that open: an open format, an open-source editor, your own model and your own server. Tell your agent to design it, watch it happen, then tell it to ship it.

Launch channels: Show HN, r/webdev, r/reactjs, X (a demo of three agents designing three screens at once is the hero shot), the Claude Code and Cursor communities, and Product Hunt.
