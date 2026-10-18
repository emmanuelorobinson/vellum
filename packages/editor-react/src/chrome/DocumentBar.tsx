import { Menu } from "@base-ui/react/menu";
import type { Command } from "@vellum/editor-core";
import { Icon } from "../icons/Icon";

export type SaveState = "saved" | "saving" | "unsaved";
const SAVE_LABEL: Record<SaveState, string> = {
  saved: "Saved",
  saving: "Saving…",
  unsaved: "Unsaved",
};

/** The menu a Windows or Linux window shows in place of a menu bar: the registered commands, by category. */
export function MenuContent({
  commands,
  categories,
}: {
  commands: readonly Command[];
  categories: readonly string[];
}) {
  const groups = categories
    .map((c) => ({ category: c, items: commands.filter((x) => x.category === c) }))
    .filter((g) => g.items.length);
  return (
    <>
      {groups.map((g) => (
        <Menu.Group key={g.category}>
          <Menu.GroupLabel className="vc-menu__heading">{g.category}</Menu.GroupLabel>
          {g.items.map((c) => (
            <Menu.Item
              key={c.id}
              className="vc-menu__item"
              disabled={c.enabled ? !c.enabled() : false}
              onClick={() => void c.run()}
            >
              {c.title}
            </Menu.Item>
          ))}
        </Menu.Group>
      ))}
    </>
  );
}

export function DocumentBar({
  fileName,
  saveState,
  width,
  commands,
  menuCategories = ["Files", "Edit", "View", "Arrange"],
}: {
  fileName: string;
  saveState: SaveState;
  width: number;
  commands: readonly Command[];
  menuCategories?: readonly string[];
}) {
  return (
    <section className="vc-bar vc-docbar" style={{ width }} aria-label="Document">
      <div className="vc-docbar__identity">
        <Menu.Root>
          <Menu.Trigger className="vc-iconbutton" aria-label="Main menu">
            <Icon name="menu" />
          </Menu.Trigger>
          <Menu.Portal>
            <Menu.Positioner side="bottom" align="start" sideOffset={8}>
              <Menu.Popup className="vc-menu">
                <MenuContent commands={commands} categories={menuCategories} />
              </Menu.Popup>
            </Menu.Positioner>
          </Menu.Portal>
        </Menu.Root>
        <span className="vc-docbar__name">{fileName}</span>
        <span className="vc-docbar__save" data-state={saveState} role="status">
          {SAVE_LABEL[saveState]}
        </span>
      </div>
    </section>
  );
}
