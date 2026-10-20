"use client";
import { brand } from "@vellum/config";
import {
  type SnapshotFrame,
  snapshotHost,
  type ViewerSnapshot,
} from "@vellum/host-browser/snapshot";
import type { Renderer } from "@vellum/renderer-api";
import { createRenderer, type ModuleFactory } from "@vellum/renderer-skia";
import { useEffect, useRef, useState } from "react";
import "./share.css";

function SnapshotCanvas({ frame, data }: { frame: SnapshotFrame; data: ViewerSnapshot }) {
  const ref = useRef<HTMLCanvasElement>(null),
    [error, setError] = useState(false);
  useEffect(() => {
    let live = true,
      renderer: Renderer | undefined;
    const host = snapshotHost(data.bundle, (t) => navigator.clipboard.writeText(t));
    const load = async () => {
      try {
        const moduleUrl = "/engine/vellum-editor.js";
        const module = (await import(/* webpackIgnore: true */ moduleUrl)) as {
          default: ModuleFactory;
        };
        const canvas = ref.current;
        if (!canvas || !live) return;
        const r = await createRenderer(canvas, module.default, {
          width: Math.ceil(frame.width),
          height: Math.ceil(frame.height),
          dpr: 1,
        });
        renderer = r;
        if (!live) {
          r.dispose();
          return;
        }
        const read = (path: string) => host.readFile(path);
        r.draw({
          displayList: await read(frame.displayList),
          fonts: await Promise.all(
            frame.fonts.map(async (f) => ({
              handle: BigInt(f.handle),
              index: f.index,
              bytes: await read(f.path),
            })),
          ),
          images: await Promise.all(
            frame.images.map(async (f) => ({ id: f.id, bytes: await read(f.path) })),
          ),
        });
      } catch {
        if (live) setError(true);
      }
    };
    void load();
    return () => {
      live = false;
      renderer?.dispose();
      host.dispose();
    };
  }, [frame, data]);
  return (
    <figure>
      <figcaption>{frame.name}</figcaption>
      {error ? (
        <p role="alert">Canvas unavailable. Download the snapshot to open it on your computer.</p>
      ) : (
        <canvas
          ref={ref}
          width={Math.ceil(frame.width)}
          height={Math.ceil(frame.height)}
          aria-label={frame.name}
        />
      )}
    </figure>
  );
}
export function ShareViewer({ id, data }: { id: string; data: ViewerSnapshot }) {
  const [selected, setSelected] = useState(0),
    [notice, setNotice] = useState("");
  const frame = data.bundle.frames[selected];
  const spec = frame?.spec;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(
        `Implement ${frame?.name ?? "this frame"} from this read-only snapshot.\n\n${JSON.stringify(spec, null, 2)}`,
      );
      setNotice("Copied implementation spec");
    } catch {
      setNotice("Copy unavailable");
    }
  };
  const download = `/v1/shares/${encodeURIComponent(id)}/download`;
  return (
    <main className="vv-shell">
      <header>
        <strong>{brand.name}</strong>
        <p>
          {data.snapshot.name}
          {data.snapshot.ownerName ? ` · ${data.snapshot.ownerName}` : ""} · Snapshot from{" "}
          {new Date(data.snapshot.at).toLocaleString(undefined, { timeZone: "UTC" })} UTC · View
          only
        </p>
        <button
          type="button"
          data-primary=""
          onClick={() => {
            window.location.href = `${brand.desktopProtocol}://share/${encodeURIComponent(id)}?origin=${encodeURIComponent(window.location.origin)}`;
          }}
        >
          Open in {brand.name}
        </button>
      </header>
      {data.snapshot.incomplete && (
        <p className="vv-notice">
          △ Incomplete snapshot · Missing: {data.snapshot.missing.join(", ")}
        </p>
      )}
      <div className="vv-body">
        <nav className="vv-frames" aria-label="Frames">
          <h3>Frames</h3>
          {data.bundle.frames.map((f, i) => (
            <button
              key={f.id}
              type="button"
              aria-pressed={i === selected}
              onClick={() => setSelected(i)}
            >
              {f.name}
            </button>
          ))}
        </nav>
        <section className="vv-canvas" aria-label="Read-only canvas">
          <a className="vv-button vv-download" href={download} download>
            Download ZIP
          </a>
          {frame ? (
            <SnapshotCanvas key={frame.id} frame={frame} data={data} />
          ) : (
            <p>
              {data.snapshot.incomplete
                ? "Some files needed to render this snapshot are missing."
                : "This snapshot has no visible frames."}
            </p>
          )}
        </section>
        <aside className="vv-inspect" aria-label="Inspect">
          <h2>Inspect</h2>
          <section>
            <h3>Implementation spec</h3>
            <pre>
              {spec
                ? JSON.stringify(spec.root ?? spec.layers ?? spec, null, 2)
                : "Select a frame to inspect its specification."}
            </pre>
          </section>
          <section>
            <h3>Tokens used</h3>
            <pre>{JSON.stringify(spec?.tokens ?? [], null, 2)}</pre>
          </section>
          <section>
            <h3>Components</h3>
            <pre>{JSON.stringify(spec?.components ?? [], null, 2)}</pre>
          </section>
          <section>
            <h3>Code reference</h3>
            <pre>{frame?.css ?? "Code unavailable"}</pre>
            <button
              type="button"
              disabled={!frame?.css}
              onClick={() =>
                void navigator.clipboard
                  .writeText(frame?.css ?? "")
                  .catch(() => setNotice("Copy unavailable"))
              }
            >
              Copy CSS
            </button>
          </section>
          <section>
            <button type="button" disabled={!spec} onClick={() => void copy()}>
              Copy prompt for your agent
            </button>
            <p role="status">{notice}</p>
          </section>
        </aside>
      </div>
    </main>
  );
}
