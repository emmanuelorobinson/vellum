import { brand } from "@vellum/config";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import Link from "next/link";
import { baseOptions } from "../lib/layout";

const paths = [
  {
    title: "Build and open a design",
    description:
      "Build the command-line tool from source, create a design file in your repo, and open it in the editor.",
    href: "/docs/getting-started/install",
    detail: "From source",
  },
  {
    title: "Connect your agent",
    description:
      "Register the MCP server with Claude Code, Codex, Cursor and the other supported clients, then ask for a screen.",
    href: "/docs/getting-started/connect-an-agent",
    detail: "Ten clients",
  },
  {
    title: "Look up a command or tool",
    description:
      "The CLI commands, every MCP tool, the script API, the file format and the decision records.",
    href: "/docs/reference",
    detail: "Reference",
  },
];

const sections = [
  {
    title: "Getting started",
    description: "Install, make a first document, tour the canvas, and check what is available.",
    href: "/docs/getting-started",
  },
  {
    title: `Using ${brand.name}`,
    description:
      "Pages, layout, components, variables, text, images, imports, exports and sharing.",
    href: "/docs/using-vellum",
  },
  {
    title: "Agents",
    description:
      "Connect a client, let it work on a frame, run scripts, and keep changes permitted and undoable.",
    href: "/docs/agents",
  },
  {
    title: "Operations",
    description:
      "The daemon, the desktop and VS Code hosts, accounts, sync, security, releases and the runbooks.",
    href: "/docs/operations",
  },
  {
    title: "Reference",
    description:
      "The CLI, MCP tools, script API, file format, configuration, environment variables and ADRs.",
    href: "/docs/reference",
  },
  {
    title: "Architecture",
    description: "The document model, the core, the daemon, the clients and the quality gates.",
    href: "/docs/architecture",
  },
  {
    title: "Development",
    description: "The workspace, the commands, the verification gates, contributing and this site.",
    href: "/docs/development",
  },
];

export default function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main id="main-content" tabIndex={-1}>
        <section className="home-intro">
          <p className="eyebrow">Documentation</p>
          <h1>{brand.tagline}</h1>
          <p className="home-lede">
            {brand.name} is an open-source design canvas in your repo. You and your coding agents
            design the same files: frames, components, variables and pages, stored as plain JSON
            that diffs in Git.
          </p>
          <div className="home-actions">
            <Link href="/docs/getting-started/install" className="ink-button">
              Install from source
            </Link>
            <Link href="/docs/using-vellum" className="text-link">
              Read the canvas guide
            </Link>
          </div>
        </section>
        <aside className="release-note" aria-label="Release status">
          <span aria-hidden="true">△</span>
          <p>
            <strong>{brand.name} is pre-release.</strong> No signed public release is published, so
            the current paths build from source.{" "}
            <Link href="/docs/getting-started/availability">
              See what is ready and what still needs verification.
            </Link>
          </p>
        </aside>
        <section aria-labelledby="paths-title" className="home-paths">
          <div className="section-heading">
            <h2 id="paths-title">Find your starting point</h2>
            <span>Three ways in</span>
          </div>
          <div className="path-grid">
            {paths.map((path) => (
              <Link key={path.href} href={path.href} className="path-card">
                <strong>{path.title}</strong>
                <p>{path.description}</p>
                <span className="path-detail">{path.detail}</span>
              </Link>
            ))}
          </div>
        </section>
        <section aria-labelledby="sections-title" className="home-sections">
          <div className="section-heading">
            <h2 id="sections-title">Sections</h2>
            <span>{sections.length} sections</span>
          </div>
          <div className="path-grid">
            {sections.map((section) => (
              <Link key={section.href} href={section.href} className="path-card">
                <strong>{section.title}</strong>
                <p>{section.description}</p>
              </Link>
            ))}
          </div>
        </section>
      </main>
    </HomeLayout>
  );
}
