export const APP_URL = "vellum-app://editor/index.html";
export const CSP =
  "default-src 'none'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
export function trustedSender(event: {
  senderFrame: { url: string } | null;
  sender: { mainFrame: { url: string } };
}): boolean {
  return (
    event.senderFrame !== null &&
    event.senderFrame === event.sender.mainFrame &&
    event.senderFrame.url === APP_URL
  );
}
/** Asset names are build output, never routes into a project or native storage. */
export function assetPath(url: string): string | null {
  if (!url.startsWith("vellum-app://editor/") || /[\\%?#]/.test(url)) return null;
  const path = url.slice("vellum-app://editor/".length);
  return /^(index\.html|editor\.(js|css)|assets\/[a-zA-Z0-9_-]+\.(woff2|ttf|png|svg))$/.test(path)
    ? path
    : null;
}
