import type { Workspace } from "@vellum/editor-core";
import type { EngineClient, EngineSnapshot } from "@vellum/engine-client";
import { useEffect, useMemo, useReducer, useState, useSyncExternalStore } from "react";

/** Re-renders when `subscribe`'s store changes. The store itself is read in render. */
export function useStore(subscribe: (listener: () => void) => { dispose(): void }): void {
  const [, bump] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    const s = subscribe(bump);
    return () => s.dispose();
  }, [subscribe]);
}

export function useWorkspace(workspace: Workspace) {
  const state = useSyncExternalStore(
    (listener) => workspace.subscribe(listener).dispose,
    () => workspace.state,
  );
  const layout = useMemo(() => workspace.layout(), [workspace, state]);
  return { state, layout };
}

export function useEngineSnapshot(engine: EngineClient): EngineSnapshot {
  const [snapshot, setSnapshot] = useState(() => engine.snapshot());
  useEffect(() => {
    setSnapshot(engine.snapshot());
    const s = engine.subscribe(setSnapshot);
    return () => s.dispose();
  }, [engine]);
  return snapshot;
}
