import { brand } from "@vellum/config";
import { RootProvider } from "fumadocs-ui/provider/next";
import type { Metadata } from "next";
import localFont from "next/font/local";
import type { ReactNode } from "react";
import "./globals.css";

// The one copy of each font in the repository, read at build time. Nothing is fetched from a font host.
const inter = localFont({
  src: "../../../crates/adapters/text-parley/fonts/InterVariable.ttf",
  variable: "--font-inter",
  display: "swap",
  weight: "100 900",
});

const jetbrainsMono = localFont({
  src: "../../../packages/ui-tokens/fonts/JetBrainsMono-wght.ttf",
  variable: "--font-jetbrains-mono",
  display: "swap",
  weight: "100 800",
});

export const metadata: Metadata = {
  title: {
    default: `${brand.name} documentation`,
    template: `%s | ${brand.name} docs`,
  },
  description: `${brand.tagline} Install ${brand.name}, design with your agents, and run its daemon, CLI and MCP server.`,
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${jetbrainsMono.variable}`}
      suppressHydrationWarning
    >
      <body className="flex min-h-screen flex-col">
        <a href="#main-content" className="skip-link">
          Skip to content
        </a>
        {/* Fumadocs writes the appearance to data-appearance, which the shared token stylesheet reads. */}
        <RootProvider theme={{ attribute: "data-appearance" }}>{children}</RootProvider>
      </body>
    </html>
  );
}
