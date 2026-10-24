/**
 * Engine and model (DESIGN.md C11): built-in providers with their models and capabilities in words, your installed
 * agents, reasoning effort, the provider-switch note, and Add provider with its setup and status row. API keys go
 * from the field straight to the daemon; nothing here keeps or shows one after it is entered.
 */
import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
import { copy } from "@vellum/config";
import {
  API_STYLES,
  type ApiStyle,
  type BuiltinProvider,
  type ComposerController,
  capabilityWords,
  EFFORTS,
  type InstalledEngine,
  installedLine,
  PROVIDERS,
  providerStatusLine,
} from "@vellum/editor-core";
import { type ReactNode, useRef, useState } from "react";
import { useLocale } from "../accessibility/locale";
import { usePortalContainer } from "../chrome/portal";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ResourceDialog } from "../resources/Dialog";

export function EnginePicker({
  composer,
  hasTurns,
  children,
}: {
  composer: ComposerController;
  hasTurns: boolean;
  children: ReactNode;
}) {
  const { t: translateUi, direction } = useLocale();
  useStore(composer.subscribe);
  const container = usePortalContainer();
  const trigger = useRef<HTMLButtonElement>(null);
  const surface = composer.state.surface;
  const catalog = composer.state.catalog;
  const choice = composer.state.choice;
  const builtin = catalog?.builtin ?? [];
  // Column-major, as the design lays them out: the first column takes the extra one.
  const half = Math.ceil(builtin.length / 2);
  // A host with only installed engines (the desktop) gets a small menu above the chip, not the full panel.
  const compact = builtin.length === 0;
  const columns = [builtin.slice(0, half), builtin.slice(half)].filter((c) => c.length);
  const anchor = {
    getBoundingClientRect: () => {
      const panel = trigger.current?.closest(".vc-panel")?.getBoundingClientRect();
      return new DOMRect(panel?.right ?? 0, (panel?.top ?? 0) + 24, 0, 0);
    },
  };
  return (
    <>
      <Popover.Root
        open={surface?.type === "picker"}
        onOpenChange={(open) => composer.open(open ? { type: "picker" } : null)}
      >
        <Popover.Trigger
          ref={trigger}
          className="vk-model-button"
          aria-label={`Engine and model: ${composer.modelLine() ?? "none"}`}
          title={composer.modelLine() ?? undefined}
        >
          {children}
        </Popover.Trigger>
        <Popover.Portal container={container}>
          <Popover.Positioner
            {...(compact ? {} : { anchor })}
            side={compact ? "top" : direction === "rtl" ? "left" : "right"}
            align={compact ? "end" : "start"}
            sideOffset={compact ? 8 : 72}
            collisionPadding={8}
          >
            <Popover.Popup className="vk-picker" data-compact={compact ? "" : undefined}>
              <Popover.Title className="vk-picker__title">
                {translateUi("Engine and model")}
              </Popover.Title>
              {compact ? null : <p className="vk-picker__group">Built-in</p>}
              <div className="vk-picker__columns" hidden={compact}>
                {columns.map((column) => (
                  <div key={column[0]?.id} className="vk-picker__column">
                    {column.map((p) => (
                      <ProviderOption key={p.id} provider={p} composer={composer} />
                    ))}
                  </div>
                ))}
              </div>
              {catalog?.installed.length ? (
                <>
                  {compact ? null : <p className="vk-picker__group">Your installed agents</p>}
                  <div className="vk-picker__installed">
                    {catalog.installed.map((e) =>
                      compact ? (
                        <InstalledEngineModels key={e.id} engine={e} composer={composer} />
                      ) : (
                        <button
                          key={e.id}
                          type="button"
                          className="vk-installed"
                          aria-pressed={choice?.engine === e.id}
                          disabled={e.state !== "signed_in"}
                          onClick={() => {
                            composer.choose({ engine: e.id, model: "default" });
                            composer.open(null);
                          }}
                        >
                          <span className="vk-installed__name">{e.name}</span>
                          <span className="vk-installed__state" data-state={e.state}>
                            {installedLine(e)}
                          </span>
                        </button>
                      ),
                    )}
                  </div>
                </>
              ) : null}
              {hasTurns ? (
                <p
                  className="vk-picker__note"
                  data-warn={composer.switchWarning() ? "" : undefined}
                >
                  A new provider won't see earlier turns in this session.
                </p>
              ) : null}
              {compact ? null : (
                <button
                  type="button"
                  className="va-button vk-picker__add"
                  onClick={() => composer.open({ type: "providers" })}
                >
                  Add provider
                </button>
              )}
            </Popover.Popup>
          </Popover.Positioner>
        </Popover.Portal>
      </Popover.Root>
      {surface?.type === "providers" ? <ProviderList composer={composer} /> : null}
      {surface?.type === "setup" ? (
        <ProviderSetupDialog composer={composer} provider={surface.provider} />
      ) : null}
    </>
  );
}

/** One installed engine and the models it lists, as a menu: the engine's own choice first, then each model. */
function InstalledEngineModels({
  engine,
  composer,
}: {
  engine: InstalledEngine;
  composer: ComposerController;
}) {
  const choice = composer.state.choice;
  const usable = engine.state === "signed_in";
  const selected = choice?.engine === engine.id;
  const models = engine.models ?? [];
  const effort = choice?.effort ?? "medium";
  const pick = (model: string) => {
    composer.choose({ engine: engine.id, model });
    composer.open(null);
  };
  const row = (id: string, label: string, caps?: string) => (
    <button
      key={id}
      type="button"
      className="vk-modelrow"
      role="menuitemradio"
      aria-checked={selected && choice?.model === id}
      disabled={!usable}
      onClick={() => pick(id)}
    >
      <span className="vk-modelrow__label vc-mono">{label}</span>
      {caps ? <span className="vk-modelrow__caps">{caps}</span> : null}
      {selected && choice?.model === id ? <Icon name="check" size={14} /> : null}
    </button>
  );
  return (
    <section className="vk-engine" aria-label={engine.name}>
      <header className="vk-engine__head">
        <span className="vk-installed__name">{engine.name}</span>
        <span className="vk-installed__state" data-state={engine.state}>
          {installedLine(engine)}
        </span>
      </header>
      <div role="menu" aria-label={`${engine.name} models`} className="vk-engine__models">
        {row("default", "Default", models.length ? undefined : "Chooses its own model")}
        {models.map((m) => row(m.id, m.label))}
      </div>
      {selected && usable && models.find((m) => m.id === choice?.model)?.caps.reasoning ? (
        <div className="vk-engine__effort" role="radiogroup" aria-label="Reasoning effort">
          <span className="vk-picker__group">Reasoning effort</span>
          <div className="vk-choice">
            {EFFORTS.map((e) => (
              // biome-ignore lint/a11y/useSemanticElements: a segmented control; radio inputs would not take this style
              <button
                key={e.id}
                type="button"
                role="radio"
                aria-checked={effort === e.id}
                className="vk-choice__option"
                onClick={() => composer.setEffort(e.id)}
              >
                {e.label}
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </section>
  );
}

function ProviderOption({
  provider,
  composer,
}: {
  provider: BuiltinProvider;
  composer: ComposerController;
}) {
  const container = usePortalContainer();
  const choice = composer.state.choice;
  const selected = choice?.engine === "builtin" && choice.provider === provider.id;
  const current =
    provider.models.find((m) => selected && m.id === choice?.model) ?? provider.models[0];
  const many = provider.models.length > 1;
  const pick = (model: string) =>
    composer.choose({ engine: "builtin", provider: provider.id, model });
  return (
    // biome-ignore lint/a11y/useSemanticElements: an option holding its own model and effort menus
    <div
      className="vk-option"
      role="button"
      tabIndex={0}
      aria-pressed={selected}
      aria-label={`${provider.name}: ${current?.label ?? "no model"}`}
      onClick={() => current && pick(current.id)}
      onKeyDown={(e) => {
        if ((e.key === "Enter" || e.key === " ") && e.target === e.currentTarget && current) {
          e.preventDefault();
          pick(current.id);
        }
      }}
    >
      <span className="vk-option__provider">{provider.name}</span>
      {many ? (
        <Menu.Root>
          <Menu.Trigger
            className="vk-option__model vc-mono"
            onClick={(e) => e.stopPropagation()}
            aria-label={`${provider.name} model: ${current?.label ?? ""}`}
          >
            {current?.label} ▾
          </Menu.Trigger>
          <Menu.Portal container={container}>
            <Menu.Positioner side="bottom" align="start" sideOffset={4}>
              <Menu.Popup className="vk-popover vk-popover--menu">
                <Menu.RadioGroup value={current?.id ?? ""} onValueChange={(v) => pick(String(v))}>
                  {provider.models.map((m) => (
                    <Menu.RadioItem
                      key={m.id}
                      value={m.id}
                      closeOnClick
                      className="vk-popover__row vc-mono"
                    >
                      {m.label}
                      <Menu.RadioItemIndicator className="vk-popover__check">
                        <Icon name="check" size={14} />
                      </Menu.RadioItemIndicator>
                    </Menu.RadioItem>
                  ))}
                </Menu.RadioGroup>
              </Menu.Popup>
            </Menu.Positioner>
          </Menu.Portal>
        </Menu.Root>
      ) : (
        <span className="vk-option__model vc-mono">{current?.label ?? "No model"}</span>
      )}
      <span className="vk-option__caps">
        {current ? capabilityWords(current.caps) : providerStatusLine(provider.status)}
        {selected && current?.caps.reasoning ? <EffortMenu composer={composer} /> : null}
      </span>
    </div>
  );
}

/** Reasoning effort, where the model supports it. */
function EffortMenu({ composer }: { composer: ComposerController }) {
  const { t: translateUi, direction } = useLocale();
  const container = usePortalContainer();
  const effort = composer.state.choice?.effort ?? "medium";
  return (
    <Menu.Root>
      <Menu.Trigger
        className="vc-iconbutton vk-option__effort"
        aria-label={`Reasoning effort: ${EFFORTS.find((e) => e.id === effort)?.label}`}
        onClick={(e) => e.stopPropagation()}
      >
        <Icon name="chevron-right" size={14} />
      </Menu.Trigger>
      <Menu.Portal container={container}>
        <Menu.Positioner
          side={direction === "rtl" ? "left" : "right"}
          align="center"
          sideOffset={8}
        >
          <Menu.Popup className="vk-effort">
            <Menu.Group>
              <Menu.GroupLabel className="vk-effort__title">
                {translateUi("Reasoning effort")}
              </Menu.GroupLabel>
              <Menu.RadioGroup
                value={effort}
                onValueChange={(v) => composer.setEffort(v as "low" | "medium" | "high")}
              >
                {EFFORTS.map((e) => (
                  <Menu.RadioItem key={e.id} value={e.id} closeOnClick className="vk-effort__row">
                    {e.label}
                    <Menu.RadioItemIndicator className="vk-popover__check">
                      <Icon name="check" size={14} />
                    </Menu.RadioItemIndicator>
                  </Menu.RadioItem>
                ))}
              </Menu.RadioGroup>
            </Menu.Group>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

/** Add provider: the list (C11-providers). */
function ProviderList({ composer }: { composer: ComposerController }) {
  const half = Math.ceil(PROVIDERS.length / 2);
  return (
    <ResourceDialog
      title="Add provider"
      description="Choose a provider, or configure your own endpoint."
      width={860}
      className="vk-providers"
      onClose={() => composer.open(null)}
    >
      <div className="vk-providers__columns">
        {[PROVIDERS.slice(0, half), PROVIDERS.slice(half)].map((column) => (
          <ul key={column[0]?.id} className="vk-providers__column">
            {column.map((p) => {
              const status = composer.providerStatus(p.id);
              return (
                <li key={p.id}>
                  <button
                    type="button"
                    className="vk-provider"
                    onClick={() => composer.open({ type: "setup", provider: p.id })}
                  >
                    <span className="vk-provider__name">{p.name}</span>
                    {status ? (
                      <span className="vk-provider__status">{providerStatusLine(status)}</span>
                    ) : null}
                    <Icon name="chevron-right" size={16} />
                  </button>
                </li>
              );
            })}
          </ul>
        ))}
      </div>
      <p className="vk-providers__hint">Choose the API style only for a custom endpoint.</p>
    </ResourceDialog>
  );
}

/** Add provider: one provider's key, endpoint and status (C11-provider). */
function ProviderSetupDialog({
  composer,
  provider,
}: {
  composer: ComposerController;
  provider: string;
}) {
  const key = useRef<HTMLInputElement>(null);
  const endpoint = useRef<HTMLInputElement>(null);
  const [style, setStyle] = useState<ApiStyle>("chat_completions");
  const [testing, setTesting] = useState(false);
  const meta = PROVIDERS.find((p) => p.id === provider);
  const status = composer.providerStatus(provider);
  const submit = async () => {
    // Read the key once, then clear the field: it is never kept in React state or shown again.
    const value = key.current?.value.trim() ?? "";
    if (key.current) key.current.value = "";
    const url = endpoint.current?.value.trim() ?? "";
    setTesting(true);
    await composer.saveProvider({
      provider,
      ...(value ? { key: value } : {}),
      ...(url ? { endpoint: url } : {}),
      ...(provider === "custom" ? { apiStyle: style } : {}),
    });
    setTesting(false);
  };
  return (
    <ResourceDialog
      title="Add provider"
      width={560}
      className="vk-setup"
      onClose={() => composer.open(null)}
    >
      <form
        className="vk-setup__body"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <button
          type="button"
          className="va-link vk-setup__provider"
          onClick={() => composer.open({ type: "providers" })}
        >
          {meta?.name ?? provider} ▾ · Choose provider
        </button>
        {provider === "local" ? null : (
          <label className="vk-setup__field">
            <span>API key</span>
            <input
              ref={key}
              type="password"
              className="vk-input vc-mono"
              autoComplete="off"
              spellCheck={false}
              placeholder={
                status && status !== "configured_externally" ? "••••••••••••••••••••" : ""
              }
            />
          </label>
        )}
        {provider === "local" ? null : <p className="vk-setup__note">{copy.keyStorage}</p>}
        {meta?.endpoint ? (
          <label className="vk-setup__field">
            <span>{meta.endpoint === "optional" ? "Local endpoint · optional" : "Endpoint"}</span>
            <input
              ref={endpoint}
              type="url"
              className="vk-input vc-mono"
              placeholder="http://localhost:11434"
              required={meta.endpoint === "required"}
            />
          </label>
        ) : null}
        {provider === "custom" ? (
          <label className="vk-setup__field">
            <span>API style</span>
            <select
              className="vk-input vc-mono"
              value={style}
              onChange={(e) => setStyle(e.target.value as ApiStyle)}
            >
              {API_STYLES.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <button type="submit" className="va-button" data-primary="" disabled={testing}>
          {testing ? "Testing…" : "Test and continue"}
        </button>
        {status ? (
          <div className="vk-setup__status">
            <span role="status" data-status={status}>
              {providerStatusLine(status)}
            </span>
            <button
              type="button"
              className="va-button"
              onClick={() => void composer.recheck(provider)}
            >
              Re-check
            </button>
            {provider === "local" ? null : (
              <button
                type="button"
                className="va-button"
                onClick={() => void composer.clearKey(provider)}
              >
                Clear key
              </button>
            )}
          </div>
        ) : null}
      </form>
    </ResourceDialog>
  );
}
