import { Tooltip } from "@base-ui/react/tooltip";
import type { ReactElement } from "react";
import { useLocale } from "../accessibility/locale";
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
  const { t, direction } = useLocale();
  const container = usePortalContainer();
  return (
    <Tooltip.Root>
      <Tooltip.Trigger render={children} />
      <Tooltip.Portal container={container}>
        <Tooltip.Positioner
          side={
            direction === "rtl"
              ? side === "right"
                ? "left"
                : side === "left"
                  ? "right"
                  : side
              : side
          }
          sideOffset={8}
        >
          <Tooltip.Popup className="vc-tooltip">
            <span>{t(name)}</span>
            {shortcut ? <span className="vc-tooltip__key">{shortcut}</span> : null}
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
export const TipProvider = Tooltip.Provider;
