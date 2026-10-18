import { canMakeSlot, slotTargets } from "@vellum/editor-core";
import { useState } from "react";
import type { DesignContext } from "../design/context";

export function SlotSection({ ctx }: { ctx: DesignContext }) {
  const [adding, setAdding] = useState<string | null>(null);
  const item = ctx.design.single()?.item;
  if (!item || !ctx.document) return null;
  const slots = slotTargets(item, ctx.document),
    a = ctx.host.authoring,
    address = ctx.session.state.selection[0] as string;
  if (!slots.length) {
    return canMakeSlot(item, ctx.document) ? (
      <section className="vx-section">
        <button
          type="button"
          className="vx-button"
          disabled={ctx.disabled || !a}
          onClick={() => a?.makeSlot(address)}
        >
          Make slot
        </button>
      </section>
    ) : null;
  }
  return (
    <>
      {slots.map((slot) => (
        <section
          key={slot.id}
          className="vx-section vx-slot-section"
          data-section="slot"
          aria-label={`Slot ${slot.name}`}
        >
          <h3>
            {slot.componentName} · {slot.instance ? "Instance" : "Component"}
          </h3>
          {slots.length > 1 ? <p>{slot.name}</p> : null}
          <p className="vx-muted">
            {slot.instance
              ? "Insert a suggested component into this slot."
              : "A slot accepts child components in an instance."}
          </p>
          {slot.suggested.map((s) => (
            <div key={s.id} className="vx-suggestion">
              <span>
                {s.name}
                {!s.available ? " · Unavailable" : ""}
              </span>
              <button
                type="button"
                className="vx-button"
                aria-label={`Insert ${s.name}`}
                title={!slot.instance ? "Select an instance to insert" : undefined}
                disabled={ctx.disabled || !a || !slot.instance || !s.available}
                onClick={() => a?.insertSlot(address, slot.id, s.id)}
              >
                +
              </button>
            </div>
          ))}
          <button
            type="button"
            className="vx-button"
            disabled={ctx.disabled || !a || !!slot.instance}
            title={slot.instance ? "Edit suggestions in the component" : undefined}
            onClick={() => setAdding(adding === slot.id ? null : slot.id)}
          >
            Add suggestion
          </button>
          {adding === slot.id ? (
            <select
              aria-label="Suggested component"
              value=""
              onChange={(e) => {
                if (e.target.value)
                  a?.setSuggestions(address, slot.id, [
                    ...slot.suggested.map((s) => s.id),
                    e.target.value,
                  ]);
                setAdding(null);
              }}
            >
              <option value="">Choose a component…</option>
              {Object.entries(ctx.document?.components ?? {})
                .filter(([id]) => !slot.suggested.some((s) => s.id === id))
                .map(([id, c]) => (
                  <option key={id} value={id}>
                    {c.name}
                  </option>
                ))}
            </select>
          ) : null}
        </section>
      ))}
    </>
  );
}
