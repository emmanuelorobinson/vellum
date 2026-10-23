import { brand } from "@vellum/config";
import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

/**
 * The mark pairs the two shapes of the product's visual language: a circle for a person, a rounded square for an
 * agent. Colours come from the shared tokens, so the mark follows light and dark appearance.
 */
function Mark() {
  return (
    <svg className="docs-mark" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <rect x="2" y="2" width="14" height="14" rx="4" className="docs-mark-agent" />
      <circle cx="16" cy="16" r="6" className="docs-mark-person" />
    </svg>
  );
}

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <span className="docs-wordmark">
          <Mark />
          {brand.name}
          <span> docs</span>
        </span>
      ),
    },
    links: [
      { text: "Start", url: "/docs/getting-started" },
      { text: "Use", url: "/docs/using-vellum" },
      { text: "Agents", url: "/docs/agents" },
      { text: "Reference", url: "/docs/reference" },
      { text: "Availability", url: "/docs/getting-started/availability" },
    ],
  };
}
