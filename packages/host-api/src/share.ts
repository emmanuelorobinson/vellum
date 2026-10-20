/** Public share progress only. Credentials and upload buffers stay in the daemon. */
export type ShareAction = "check" | "create" | "status" | "retry" | "delete";
export interface ShareRequest {
  action: ShareAction;
  id?: string;
  shareAnyway?: boolean;
}
export interface ShareState {
  state: "not-shared" | "checking" | "uploading" | "shared" | "changed" | "failed";
  id?: string;
  url?: string;
  progress?: number;
  missing?: string[];
  snapshot?: { at: number; incomplete: boolean };
  message?: string;
}
export interface SharePort {
  request(request: ShareRequest): Promise<ShareState>;
  copy(text: string): Promise<void>;
  open(url: string): Promise<void>;
}
export function shareRequest(v: unknown): v is ShareRequest {
  if (!v || typeof v !== "object" || Array.isArray(v)) return false;
  const r = v as ShareRequest;
  return (
    Object.keys(r).every((k) => ["action", "id", "shareAnyway"].includes(k)) &&
    ["check", "create", "status", "retry", "delete"].includes(r.action) &&
    (r.id === undefined || /^[A-Za-z0-9_-]{24,128}$/.test(r.id)) &&
    (r.shareAnyway === undefined || typeof r.shareAnyway === "boolean")
  );
}
export function shareState(value: unknown): ShareState {
  if (!value || typeof value !== "object") throw Error("Sharing unavailable");
  const v = value as ShareState;
  if (
    !["not-shared", "checking", "uploading", "shared", "changed", "failed"].includes(v.state) ||
    (v.id !== undefined && !/^[A-Za-z0-9_-]{24,128}$/.test(v.id))
  )
    throw Error("Sharing unavailable");
  let url: string | undefined;
  if (v.url) {
    if (
      !/^(?:https:\/\/[A-Za-z0-9.-]+(?::[0-9]{1,5})?|http:\/\/(?:localhost|127\.0\.0\.1|\[::1\])(?::[0-9]{1,5})?)\/share\/[A-Za-z0-9_-]{24,128}$/.test(
        v.url,
      )
    )
      throw Error("Sharing unavailable");
    url = v.url;
  }

  if (v.state === "shared" && !url) throw Error("Sharing unavailable");
  return {
    state: v.state,
    ...(v.id ? { id: v.id } : {}),
    ...(url ? { url } : {}),
    ...(typeof v.progress === "number" && Number.isFinite(v.progress)
      ? { progress: Math.max(0, Math.min(1, v.progress)) }
      : {}),
    ...(Array.isArray(v.missing)
      ? {
          missing: v.missing
            .filter((p) => typeof p === "string" && p.length <= 1024)
            .slice(0, 2048),
        }
      : {}),
    ...(v.snapshot && Number.isSafeInteger(v.snapshot.at)
      ? { snapshot: { at: v.snapshot.at, incomplete: v.snapshot.incomplete === true } }
      : {}),
    ...(v.state === "failed"
      ? { message: "Upload failed. Retry to resume, or check your account and connection." }
      : {}),
  };
}
