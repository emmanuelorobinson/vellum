import { Menu } from "@base-ui/react/menu";
import { Icon } from "../icons/Icon";
import { ActorMarker, type ActorView } from "./ActorMarker";
import { Tip } from "./Tooltip";

export interface TopRightProps {
  right: number;
  /** Agents active in this document, shown as a stack of rounded squares. */
  agents: readonly ActorView[];
  /** Share is for signed-in Pro accounts; absent hides it. */
  onShare?: () => void;
  onPreview?: () => void;
  onSettings?: () => void;
  /** Pause all agents; absent while nothing can pause them. */
  onPauseAll?: () => void;
}

export function TopRightBar(p: TopRightProps) {
  return (
    <div
      className="vc-bar vc-topright"
      style={{ right: p.right }}
      role="toolbar"
      aria-label="Presence and sharing"
    >
      <Menu.Root>
        <Menu.Trigger className="vc-textbutton vc-topright__agents" aria-label="Agents">
          <span className="vc-stack">
            {p.agents.map((a) => (
              <ActorMarker key={a.id} actor={a} size={16} />
            ))}
          </span>
          <span>Agents</span>
        </Menu.Trigger>
        <Menu.Portal>
          <Menu.Positioner side="bottom" align="end" sideOffset={8}>
            <Menu.Popup className="vc-menu">
              {p.agents.length === 0 ? (
                <Menu.Item className="vc-menu__item" disabled>
                  No agents in this document
                </Menu.Item>
              ) : (
                p.agents.map((a) => (
                  <Menu.Item key={a.id} className="vc-menu__item vc-menu__item--icon" disabled>
                    <ActorMarker actor={a} size={16} />
                    <span>{a.name}</span>
                    <span className="vc-menu__key">{a.state ?? ""}</span>
                  </Menu.Item>
                ))
              )}
              <Menu.Separator className="vc-menu__separator" />
              <Menu.Item
                className="vc-menu__item"
                disabled={!p.onPauseAll}
                onClick={() => p.onPauseAll?.()}
              >
                Pause all agents
              </Menu.Item>
            </Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
      {p.onShare ? (
        <button type="button" className="vc-textbutton" onClick={p.onShare}>
          Share
        </button>
      ) : null}
      <Tip name="Preview" side="bottom">
        <button
          type="button"
          className="vc-iconbutton"
          aria-label="Preview prototype"
          aria-disabled={p.onPreview ? undefined : true}
          onClick={() => p.onPreview?.()}
        >
          <Icon name="play" />
        </button>
      </Tip>
      <Tip name="Settings" side="bottom">
        <button
          type="button"
          className="vc-iconbutton"
          aria-label="Editor settings"
          aria-disabled={p.onSettings ? undefined : true}
          onClick={() => p.onSettings?.()}
        >
          <Icon name="settings" />
        </button>
      </Tip>
    </div>
  );
}
