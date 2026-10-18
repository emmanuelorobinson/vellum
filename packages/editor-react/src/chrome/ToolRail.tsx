import { Menu } from "@base-ui/react/menu";
import type { ToolId } from "@vellum/editor-core";
import { Icon } from "../icons/Icon";
import { usePortalContainer } from "./portal";
import { Tip } from "./Tooltip";

export interface RailItem {
  id: string;
  name: string;
  icon: string;
  command: string;
  /** The tool this selects; absent for actions such as Ask. */
  tool?: ToolId | "image";
}
export const RAIL: readonly RailItem[] = [
  { id: "move", name: "Move", icon: "mouse-pointer-2", command: "tool.move", tool: "move" },
  { id: "frame", name: "Frame", icon: "frame", command: "tool.frame", tool: "frame" },
  { id: "rect", name: "Rectangle", icon: "square", command: "tool.rect", tool: "rect" },
  { id: "pen", name: "Pen", icon: "pen-tool", command: "tool.pen", tool: "pen" },
  { id: "text", name: "Text", icon: "type", command: "tool.text", tool: "text" },
  { id: "image", name: "Image", icon: "image", command: "image.insert", tool: "image" },
  { id: "hand", name: "Hand", icon: "hand", command: "tool.hand", tool: "hand" },
];
/** Under Rectangle's menu (DESIGN.md "Tool rail"). Polygon has no key. */
export const SHAPE_MENU: readonly RailItem[] = [
  { id: "rect", name: "Rectangle", icon: "square", command: "tool.rect", tool: "rect" },
  { id: "ellipse", name: "Ellipse", icon: "circle", command: "tool.ellipse", tool: "ellipse" },
  { id: "line", name: "Line", icon: "minus", command: "tool.line", tool: "line" },
  { id: "polygon", name: "Polygon", icon: "pentagon", command: "tool.polygon", tool: "polygon" },
];
const SHAPES = new Set<string>(["rect", "ellipse", "line", "polygon", "star"]);

export interface RailProps {
  x: number;
  y: number;
  /** The active tool; a temporary tool such as the held hand shows as itself. */
  tool: ToolId;
  /** Switches to a drawing or navigation tool; always available. */
  selectTool(tool: ToolId): void;
  /** Actions that need a feature or the host (Ask, Image, shortcuts); disabled while nothing handles them. */
  run(command: string): void;
  enabled(command: string): boolean;
  shortcut(command: string): string | null;
}

function RailButton({
  item,
  active,
  props,
  label,
}: {
  item: RailItem;
  active: boolean;
  props: RailProps;
  label?: string;
}) {
  const isTool = !!item.tool && item.tool !== "image";
  const enabled = isTool || props.enabled(item.command);
  const activate = () => {
    if (!enabled) return;
    if (isTool) props.selectTool(item.tool as ToolId);
    else props.run(item.command);
  };
  return (
    <Tip name={label ?? item.name} shortcut={props.shortcut(item.command)}>
      <button
        type="button"
        className="vc-rail__button"
        aria-label={label ?? item.name}
        aria-pressed={item.tool ? active : undefined}
        aria-disabled={enabled ? undefined : true}
        data-active={active ? "" : undefined}
        onClick={activate}
      >
        <Icon name={item.icon} size={18} />
      </button>
    </Tip>
  );
}

export function ToolRail(props: RailProps) {
  const container = usePortalContainer();
  const shapeActive = SHAPES.has(props.tool);
  const shown = SHAPE_MENU.find((s) => s.tool === props.tool) ?? (SHAPE_MENU[0] as RailItem);
  return (
    <div
      className="vc-bar vc-rail"
      style={{ left: props.x, top: props.y }}
      role="toolbar"
      aria-label="Tools"
      aria-orientation="vertical"
    >
      {RAIL.map((item) =>
        item.id === "rect" ? (
          <Menu.Root key={item.id}>
            <Tip name={shown.name} shortcut={props.shortcut(shown.command)}>
              <Menu.Trigger
                className="vc-rail__button"
                aria-label="Shapes"
                data-active={shapeActive ? "" : undefined}
                aria-pressed={shapeActive}
              >
                <Icon name={shown.icon} size={18} />
              </Menu.Trigger>
            </Tip>
            <Menu.Portal container={container}>
              <Menu.Positioner side="right" align="start" sideOffset={8}>
                <Menu.Popup className="vc-menu">
                  {SHAPE_MENU.map((s) => (
                    <Menu.Item
                      key={s.id}
                      className="vc-menu__item vc-menu__item--icon"
                      onClick={() => props.selectTool(s.tool as ToolId)}
                    >
                      <Icon name={s.icon} />
                      <span>{s.name}</span>
                      <span className="vc-menu__key">{props.shortcut(s.command) ?? ""}</span>
                    </Menu.Item>
                  ))}
                </Menu.Popup>
              </Menu.Positioner>
            </Menu.Portal>
          </Menu.Root>
        ) : (
          <RailButton key={item.id} item={item} active={props.tool === item.tool} props={props} />
        ),
      )}
      <hr className="vc-rail__gap" />
      <RailButton
        item={{ id: "ask", name: "Ask", icon: "message-square", command: "agent.inline" }}
        active={false}
        props={props}
      />
      <RailButton
        item={{ id: "keys", name: "Keyboard shortcuts", icon: "keyboard", command: "palette.open" }}
        active={false}
        props={props}
      />
    </div>
  );
}
