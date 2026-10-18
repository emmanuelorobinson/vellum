import { cssVar } from "@vellum/ui-tokens";

/** Who made a change or is here. Agents are rounded squares, people are circles, everywhere (AGENTS.md). */
export interface ActorView {
  id: string;
  kind: "agent" | "person";
  /** What the label says: "Teal", "Claude Code", a person's name. */
  name: string;
  /** An agent hue name from the tokens (`teal`, `saffron`). Ignored for people. */
  hue?: string;
  /** "working", "idle": said in words in the label, never by colour alone. */
  state?: string;
}

export function ActorMarker({
  actor,
  size = 16,
  outlined = false,
}: {
  actor: ActorView;
  size?: number;
  /** A 1 px ring in the panel colour, for markers that sit on rows. */
  outlined?: boolean;
}) {
  const agent = actor.kind === "agent";
  const label = actor.state ? `${actor.name} · ${actor.state}` : actor.name;
  return (
    <span
      className="vc-marker"
      data-actor={actor.kind}
      data-hue={agent ? actor.hue : undefined}
      data-outlined={outlined ? "" : undefined}
      role="img"
      aria-label={label}
      title={label}
      style={{
        width: size,
        height: size,
        borderRadius: agent ? size * 0.3 : "50%",
        background: agent && actor.hue ? `var(--agent-${actor.hue})` : cssVar("chrome/secondary"),
      }}
    />
  );
}
