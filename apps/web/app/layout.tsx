import { brand } from "@vellum/config";
import type { ReactNode } from "react";

export const metadata = {
  title: brand.name,
  description: brand.tagline,
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
