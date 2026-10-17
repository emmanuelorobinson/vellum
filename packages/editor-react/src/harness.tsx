import { FakeEngineClient } from "@vellum/engine-client";
import { FakeDaemonClient, NoopSyncClient, NoopTelemetry } from "@vellum/host-api";
import { FakeHost } from "@vellum/host-fake";
import type { Mode } from "@vellum/ui-tokens";
import type { ComponentType } from "react";
import fixture from "../fixtures/components.json";
import "./harness.css";

export interface ComponentCase {
  id: string;
  componentId: string;
  name: string;
  variant: string;
  mode: Mode;
  frameId: string | null;
  nodeId: string | null;
  reference: "available" | "missing";
  box: { x: number; y: number; width: number; height: number };
  status: string;
}
export const componentCases = fixture.cases as ComponentCase[];
export const componentBoards = fixture.boards;
export const componentSourceHash = fixture.source_sha256;
export function createHarnessServices() {
  const host = new FakeHost();
  const engine = new FakeEngineClient();
  const daemon = new FakeDaemonClient();
  const sync = new NoopSyncClient();
  const telemetry = new NoopTelemetry();
  return {
    host,
    engine,
    daemon,
    sync,
    telemetry,
    dispose() {
      host.dispose();
      engine.dispose();
      daemon.dispose();
      sync.dispose();
      telemetry.dispose();
    },
  };
}
export type HarnessServices = ReturnType<typeof createHarnessServices>;
export interface SpecimenProps {
  specimen: ComponentCase;
  services: HarnessServices;
}
const implementations = new Map<string, ComponentType<SpecimenProps>>();
/** Later component prompts opt in here; an unbuilt specimen can never pass a pixel comparison. */
export function registerHarnessComponent(
  componentId: string,
  component: ComponentType<SpecimenProps>,
) {
  if (
    !componentCases.some((c) => c.componentId === componentId) ||
    implementations.has(componentId)
  )
    throw Error("Unknown or duplicate harness component");
  implementations.set(componentId, component);
  return {
    dispose() {
      if (implementations.get(componentId) === component) implementations.delete(componentId);
    },
  };
}
export function componentGoldens() {
  const ready = componentCases.filter((c) => implementations.has(c.componentId)).length;
  return { ready, pending: componentCases.length - ready };
}
export function HarnessSpecimen({ specimen, services }: SpecimenProps) {
  const Component = implementations.get(specimen.componentId);
  return (
    <div
      data-appearance={specimen.mode}
      data-canvas-appearance={specimen.mode}
      className="component-harness"
      data-component-case={specimen.id}
    >
      {Component ? (
        <div
          data-component-golden={specimen.id}
          data-golden-status="ready"
          style={{ width: specimen.box.width, height: specimen.box.height }}
        >
          <Component specimen={specimen} services={services} />
        </div>
      ) : (
        <div data-golden-status="pending">
          <p>Pending component implementation</p>
          <p>
            {specimen.name} · {specimen.variant} · {specimen.mode}
          </p>
        </div>
      )}
    </div>
  );
}
