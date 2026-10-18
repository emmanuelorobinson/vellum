import { Tooltip } from "@base-ui/react/tooltip";
import type { ReactElement } from "react";
import { usePortalContainer } from "./portal";

/** The design's tool tooltip: the name, then the shortcut in mono. */
export function Tip({
  name,
  shortcut,
  children,
  side = "right",
}: {
  name: string;
  shortcut?: string | null;
  children: ReactElement;
  side?: "top" | "right" | "bottom" | "left";
}) {
  const container = usePortalContainer();
  return (
    <Tooltip.Root>
      <Tooltip.Trigger render={children} />
      <Tooltip.Portal container={container}>
        <Tooltip.Positioner side={side} sideOffset={8}>
          <Tooltip.Popup className="vc-tooltip">
            <span>{name}</span>
            {shortcut ? <span className="vc-tooltip__key">{shortcut}</span> : null}
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
export const TipProvider = Tooltip.Provider;
