import Link from "next/link";

export default function NotFound() {
  return (
    <main id="main-content" className="route-message" tabIndex={-1}>
      <p className="eyebrow">404</p>
      <h1>Page not found</h1>
      <p>This address does not have a documentation page.</p>
      <Link href="/docs" className="ink-button">
        Documentation index
      </Link>
      <Link href="/docs/getting-started/install">Install from source</Link>
    </main>
  );
}
