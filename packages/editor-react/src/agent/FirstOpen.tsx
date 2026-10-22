/**
 * First open (DESIGN.md C01): the question, five suggestions and the draw-by-hand hint, in a session with no turns.
 * Each suggestion fills the composer; nothing is sent until the person sends it.
 */
import { Popover } from "@base-ui/react/popover";
import { copy } from "@vellum/config";
import type { ComposerController } from "@vellum/editor-core";
import { type RefObject, useState } from "react";
import { useLocale } from "../accessibility/locale";
import { usePortalContainer } from "../chrome/portal";
import { Icon } from "../icons/Icon";
import { ResourceDialog } from "../resources/Dialog";
import type { ComposerHost } from "./Composer";

const CODE = [".tsx", ".jsx", ".vue", ".svelte", ".html"];

export function FirstOpen({
  composer,
  host,
  field,
}: {
  composer: ComposerController;
  host: ComposerHost;
  field: RefObject<HTMLTextAreaElement | null>;
}) {
  const { t, direction } = useLocale();
  const container = usePortalContainer();
  const [styles, setStyles] = useState(false);
  const fill = (text: string) => {
    composer.setText(text, text.length);
    field.current?.focus();
  };
  const mention = (extensions: readonly string[]) => {
    composer.mention(extensions);
    field.current?.focus();
  };
  const suggestion = (icon: string, label: string, onClick?: () => void) => (
    <button type="button" className="vk-suggestion" onClick={onClick}>
      <Icon name={icon} size={16} />
      {t(label)}
    </button>
  );
  return (
    <section className="vk-first" aria-labelledby="vk-first-heading">
      <h2 id="vk-first-heading" className="vk-first__heading">
        {t(copy.firstOpenHeading)}
      </h2>
      <p className="vk-first__text">
        {t("Describe your idea, or point an agent at a file in your repo.")}
      </p>
      <div className="vk-first__suggestions">
        {suggestion("file-text", "Design from a spec", () => mention([".md"]))}
        {host.designSystems?.length ? (
          <Popover.Root>
            <Popover.Trigger className="vk-suggestion">
              <Icon name="component" size={16} />
              {t("Start from a design system")}
            </Popover.Trigger>
            <Popover.Portal container={container}>
              <Popover.Positioner
                side={direction === "rtl" ? "left" : "right"}
                align="start"
                sideOffset={8}
              >
                <Popover.Popup className="vk-popover vk-popover--menu">
                  <Popover.Title className="vk-popover__label">Design systems</Popover.Title>
                  {host.designSystems.map((d) => (
                    <Popover.Close
                      key={d.id}
                      className="vk-popover__row"
                      onClick={() => fill(`Start from the ${d.name} design system. `)}
                    >
                      <span className="vk-swatches" aria-hidden>
                        {d.swatches.map((c) => (
                          <span key={c} className="vk-swatch" style={{ background: c }} />
                        ))}
                      </span>
                      {d.name}
                    </Popover.Close>
                  ))}
                </Popover.Popup>
              </Popover.Positioner>
            </Popover.Portal>
          </Popover.Root>
        ) : (
          suggestion("component", "Start from a design system", () =>
            fill("Start from a design system. "),
          )
        )}
        {suggestion("palette", "Pick a visual style", () =>
          host.visualStyles?.length ? setStyles(true) : fill("Pick a visual style for "),
        )}
        {suggestion("palette", "Import tokens from CSS", () => mention([".css"]))}
        {suggestion("square-terminal", "Recreate a screen from my code", () => mention(CODE))}
      </div>
      <p className="vk-first__hint">{t(copy.drawByHandHint)}</p>
      {styles && host.visualStyles ? (
        <ResourceDialog
          title="Pick a visual style"
          description="Start with a type sample and a palette. You can refine the design as it arrives."
          width={864}
          className="vk-styles"
          onClose={() => setStyles(false)}
        >
          <ul className="vk-styles__grid">
            {host.visualStyles.map((v) => (
              <li key={v.id} className="vk-style">
                <span className="vk-style__sample" style={{ fontFamily: v.font }} aria-hidden>
                  Aa / 123
                </span>
                <span className="vk-style__palette" aria-hidden>
                  {v.palette.map((c) => (
                    <span key={c} className="vk-style__colour" style={{ background: c }} />
                  ))}
                </span>
                <span className="vk-style__name">{v.name}</span>
                <button
                  type="button"
                  className="va-button"
                  data-primary=""
                  aria-label={`Use ${v.name}`}
                  onClick={() => {
                    setStyles(false);
                    fill(`Use the ${v.name} visual style. `);
                  }}
                >
                  Use this style
                </button>
              </li>
            ))}
          </ul>
        </ResourceDialog>
      ) : null}
    </section>
  );
}
