import { describe, expect, it } from "vitest";
import { parseRequest } from "../shared/bridge";
import { APP_URL, assetPath, CSP, trustedSender } from "./security";

describe("desktop renderer boundary", () => {
  it("trusts only the exact loaded main frame of a registered window", () => {
    const frame = { url: APP_URL };
    expect(trustedSender({ senderFrame: frame, sender: { mainFrame: frame } })).toBe(true);
    expect(trustedSender({ senderFrame: { url: APP_URL }, sender: { mainFrame: frame } })).toBe(
      false,
    );
    for (const url of [
      "file:///secret",
      "https://example.org",
      `${APP_URL}?other`,
      `${APP_URL}/evil`,
      "about:blank",
    ]) {
      const untrusted = { url };
      expect(trustedSender({ senderFrame: untrusted, sender: { mainFrame: untrusted } })).toBe(
        false,
      );
    }
    expect(trustedSender({ senderFrame: null, sender: { mainFrame: frame } })).toBe(false);
    expect(CSP).toContain("connect-src 'none'");
    // The document engine compiles WebAssembly; JavaScript eval stays off.
    expect(CSP).toContain("script-src 'self' 'wasm-unsafe-eval'");
    expect(CSP).not.toContain("'unsafe-eval'");
    expect(CSP).toContain("frame-src 'none'");
  });
  it("serves only bundled assets, with no filesystem or credential routes", () => {
    expect(assetPath("vellum-app://editor/index.html")).toBe("index.html");
    expect(assetPath("vellum-app://editor/assets/font-1.woff2")).toBe("assets/font-1.woff2");
    for (const url of [
      "file:///etc/passwd",
      "vellum-app://other/index.html",
      "vellum-app://editor/%2e%2e/secret",
      "vellum-app://editor/daemon.json",
      "vellum-app://editor/assets/%2fetc%2fpasswd",
      "vellum-app://editor/index.html?secret=1",
    ])
      expect(assetPath(url)).toBe(null);
  });
  it("parses an exact action allowlist, with no generic IPC, path, socket or write capability", () => {
    expect(parseRequest({ type: "activate", tab: "1" })).toEqual({ type: "activate", tab: "1" });
    for (const request of [
      null,
      [],
      { type: "read", path: "/secret" },
      { type: "save", bytes: [] },
      { type: "rpc", method: "execute" },
      { type: "activate", tab: "1", path: "/secret" },
      { type: "activate", tab: 1 },
      { type: "menu", category: "unknown" },
    ])
      expect(parseRequest(request)).toBe(null);
  });
});
