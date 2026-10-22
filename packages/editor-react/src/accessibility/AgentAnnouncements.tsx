import { type AgentController, completionChanges } from "@vellum/editor-core";
import { useEffect, useRef, useState } from "react";
import { useStore } from "../hooks";
import { useLocale } from "./locale";

/** One stable region outside the tab: neither streaming tokens nor expanding stored history is news. */
export function AgentAnnouncements({ agent }: { agent: AgentController }) {
  useStore(agent.subscribe);
  const { t } = useLocale();
  const previous = useRef<{ session: string; states: Map<string, string> } | null>(null);
  const [message, setMessage] = useState({ text: "", serial: 0 });
  useEffect(() => {
    const session = agent.session()?.id;
    if (!session || agent.state.loading) {
      previous.current = null;
      return;
    }
    const states = new Map<string, string>(),
      labels = new Map<string, string>();
    for (const turn of agent.turns()) {
      if (!turn.external) {
        states.set(turn.key, turn.state);
        labels.set(turn.key, t(turn.state === "done" ? "Turn complete" : turn.label));
      }
      for (const item of turn.items) {
        const steps = item.type === "group" ? item.steps : item.type === "step" ? [item.step] : [];
        for (const step of steps) {
          const key = `${turn.key}/${step.key}`;
          states.set(key, step.state);
          labels.set(
            key,
            `${t(step.state === "failed" ? "Step failed" : step.state === "warning" ? "Step refused" : "Step complete")}: ${step.line}`,
          );
        }
      }
    }
    const changed = completionChanges(
      previous.current?.session === session ? previous.current.states : null,
      states,
    );
    previous.current = { session, states };
    if (changed.length)
      setMessage((m) => ({
        text: changed.map((key) => labels.get(key)).join(". "),
        serial: m.serial + 1,
      }));
  });
  return (
    <div className="vc-sr" role="status" aria-live="polite" aria-atomic="true" data-completions="">
      <span key={message.serial}>{message.text}</span>
    </div>
  );
}
