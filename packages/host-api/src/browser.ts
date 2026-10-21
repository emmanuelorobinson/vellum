/** Browser pixels and computed layout cross this port; remote pages never enter the editor DOM. */
declare const URL: {
  new (
    url: string,
  ): {
    protocol: string;
    username: string;
    password: string;
    origin: string;
    pathname: string;
    search: string;
    hash: string;
  };
};
export interface BrowserRequest {
  url: string;
  width: number;
  height: number;
  origins?: readonly string[];
  selector?: string;
  scroll?: readonly [number, number];
  zoom?: number;
  snapshot?: boolean;
}
export interface DomNode {
  kind: "frame" | "text" | "image";
  name: string;
  selector?: string;
  parent: number | null;
  x: number;
  y: number;
  width: number;
  height: number;
  style: Record<string, string>;
  text?: string;
  image?: string;
}
export interface DomSnapshot {
  version: 1;
  width: number;
  height: number;
  nodes: readonly DomNode[];
  notes: readonly string[];
}
export interface BrowserCapture {
  png: string;
  width: number;
  height: number;
  dom?: DomSnapshot;
}
export interface BrowserPort {
  capture(request: BrowserRequest): Promise<BrowserCapture>;
  /** A live page session. Disposing stops updates and destroys its isolated browser. */
  watch?(
    request: BrowserRequest,
    frame: (capture: BrowserCapture) => void,
    failed: () => void,
  ): Promise<{ dispose(): void }>;
}
export function browserOrigins(request: BrowserRequest): readonly string[] {
  const parse = (raw: string) => {
    const url = new URL(raw);
    if (!["http:", "https:"].includes(url.protocol) || url.username || url.password)
      throw Error("Use an HTTP(S) address without credentials.");
    return url;
  };
  const origins = new Set([parse(request.url).origin]);
  if ((request.origins?.length ?? 0) > 16) throw Error("Too many origins.");
  for (const raw of request.origins ?? []) {
    const url = parse(raw);
    if (url.pathname !== "/" || url.search || url.hash)
      throw Error("Use an origin without a path.");
    origins.add(url.origin);
  }
  return [...origins];
}
export function browserRequest(input: BrowserRequest): BrowserRequest {
  if (
    !input ||
    typeof input.url !== "string" ||
    input.url.length > 8192 ||
    ![input.width, input.height].every((n) => Number.isInteger(n) && n >= 1 && n <= 4096) ||
    !Number.isFinite(input.zoom ?? 1) ||
    (input.zoom ?? 1) < 0.1 ||
    (input.zoom ?? 1) > 4 ||
    (input.selector !== undefined &&
      (typeof input.selector !== "string" || !input.selector || input.selector.length > 2048)) ||
    (input.scroll !== undefined &&
      (input.scroll.length !== 2 ||
        input.scroll.some((n) => !Number.isFinite(n) || n < 0 || n > 1_000_000)))
  )
    throw Error("Invalid browser request.");
  browserOrigins(input);
  return {
    ...input,
    origins: [...(input.origins ?? [])],
    scroll: input.scroll ?? [0, 0],
    zoom: input.zoom ?? 1,
  };
}
