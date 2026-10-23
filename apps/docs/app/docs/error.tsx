"use client";

import Link from "next/link";

export default function ErrorPage({ reset }: { reset: () => void }) {
  return (
    <div className="route-message" role="alert">
      <h1>Documentation could not load</h1>
      <p>Try loading this page again, or return to the documentation index.</p>
      <button type="button" onClick={reset} className="ink-button">
        Try again
      </button>
      <Link href="/docs">Documentation index</Link>
    </div>
  );
}
