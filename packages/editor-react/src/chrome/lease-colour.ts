import type { CSSProperties } from "react";
/** The agent's assigned hue for its cursor, lease, placeholder and insert feedback. Unknown stays neutral. */
export const leaseColour = (hue?: string): CSSProperties =>
  ({ "--lease-hue": hue ? `var(--agent-${hue})` : "var(--chrome-secondary)" }) as CSSProperties;
