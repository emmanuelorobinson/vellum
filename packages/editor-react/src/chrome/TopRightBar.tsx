import { Menu } from "@base-ui/react/menu";
import { type PresenceController, presenceState } from "@vellum/editor-core";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ActorMarker, type ActorView } from "./ActorMarker";
import { usePortalContainer } from "./portal";
import { Tip } from "./Tooltip";

export interface TopRightProps {
  right: number;
  presence?: PresenceController;
  /** Agents active in this document, shown as a stack of rounded squares. */
  agents: readonly ActorView[];
  /** Share is for signed-in Pro accounts; absent hides it. */
  onShare?: () => void;
  onPreview?: () => void;
  onSettings?: () => void;
  /** Pause all agents; absent while nothing can pause them. */
  onPauseAll?: () => void;
  onConnectAgents?: () => void;
}

export function TopRightBar(p: TopRightProps) {
  useStore(p.presence?.subscribe ?? (() => ({ dispose() {} })));
  const container = usePortalContainer();
  const agents = p.presence
    ? p.presence
        .agents()
        .map((a) => ({ ...a, kind: "agent" as const, state: presenceState(a.state) }))
    : p.agents;
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
            {agents.map((a) => (
              <ActorMarker key={a.id} actor={a} size={16} />
            ))}
          </span>
          <span>Agents</span>
        </Menu.Trigger>
        <Menu.Portal container={container}>
          <Menu.Positioner side="bottom" align="end" sideOffset={8}>
            <Menu.Popup className="vc-menu">
              {agents.length === 0 ? (
                <Menu.Item className="vc-menu__item" disabled>
                  No agents in this document
                </Menu.Item>
              ) : (
                agents.map((a) => (
                  <Menu.Group key={a.id}>
                    <Menu.GroupLabel className="vc-menu__item vc-menu__item--icon">
                      <ActorMarker actor={a} size={16} />
                      <span>{a.name}</span>
                      <span className="vc-menu__key">{a.state ?? ""}</span>
                    </Menu.GroupLabel>
                    {p.presence ? (
                      <>
                        <Menu.Item
                          className="vc-menu__item"
                          onClick={() => p.presence?.follow(a.id)}
                        >
                          Follow {a.name}
                        </Menu.Item>
                        <Menu.Item
                          className="vc-menu__item"
                          onClick={() => p.presence?.openSession(a.id)}
                        >
                          Open session · {a.name}
                        </Menu.Item>
                        <Menu.Item
                          className="vc-menu__item"
                          disabled={p.presence.state.pending}
                          onClick={() =>
                            void p.presence?.action({ type: "stopAgent", agent: a.id })
                          }
                        >
                          Stop {a.name}
                        </Menu.Item>
                      </>
                    ) : null}
                  </Menu.Group>
                ))
              )}
              <Menu.Separator className="vc-menu__separator" />
              {p.onConnectAgents ? (
                <Menu.Item className="vc-menu__item" onClick={p.onConnectAgents}>
                  Connect agents…
                </Menu.Item>
              ) : null}
              <Menu.Item
                className="vc-menu__item"
                disabled={
                  p.presence
                    ? !p.presence.state.snapshot.connected || p.presence.state.pending
                    : !p.onPauseAll
                }
                onClick={() =>
                  p.presence ? void p.presence.action({ type: "pauseAll" }) : p.onPauseAll?.()
                }
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
