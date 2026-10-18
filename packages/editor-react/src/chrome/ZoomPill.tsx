import { Menu } from "@base-ui/react/menu";
import { Icon } from "../icons/Icon";
import { usePortalContainer } from "./portal";

export interface ZoomItem {
  label: string;
  run(): void;
  enabled?: boolean;
  key?: string | null;
}

export function ZoomPill({
  x,
  y,
  label,
  items,
}: {
  x: number;
  y: number;
  label: string;
  items: readonly ZoomItem[];
}) {
  const container = usePortalContainer();
  return (
    <div className="vc-bar vc-zoom" style={{ left: x, top: y }}>
      <Menu.Root>
        <Menu.Trigger className="vc-zoom__button" aria-label={`Zoom ${label}`}>
          <span>{label}</span>
          <Icon name="chevron-down" size={10} />
        </Menu.Trigger>
        <Menu.Portal container={container}>
          <Menu.Positioner side="top" align="end" sideOffset={8}>
            <Menu.Popup className="vc-menu">
              {items.map((i) => (
                <Menu.Item
                  key={i.label}
                  className="vc-menu__item vc-menu__item--icon"
                  disabled={i.enabled === false}
                  onClick={i.run}
                >
                  <span>{i.label}</span>
                  <span className="vc-menu__key">{i.key ?? ""}</span>
                </Menu.Item>
              ))}
            </Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
    </div>
  );
}
