"use client";
import { ShareViewer } from "@vellum/editor-react/share";
import type { ViewerSnapshot } from "@vellum/host-browser/snapshot";
import { use, useEffect, useState } from "react";
export default function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params),
    [data, setData] = useState<ViewerSnapshot | null>(null),
    [error, setError] = useState(false);
  useEffect(() => {
    let live = true;
    const controller = new AbortController();
    void fetch(`/v1/shares/${encodeURIComponent(id)}`, {
      cache: "no-store",
      signal: controller.signal,
    })
      .then(async (r) => {
        if (!r.ok) throw Error();
        const value = await r.json();
        if (live) setData(value);
      })
      .catch(() => {
        if (live) setError(true);
      });
    return () => {
      live = false;
      controller.abort();
    };
  }, [id]);
  return data ? (
    <ShareViewer id={id} data={data} />
  ) : (
    <main role="status">
      {error ? "Share unavailable. The owner may have deleted this link." : "Loading snapshot…"}
    </main>
  );
}
