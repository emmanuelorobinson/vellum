/**
 * The composer pinned to the bottom of the Agent tab (DESIGN.md "Composer"; C01, C08, C26, C27): context chips, the
 * field with its @ and / menus, the footer (attach, Parallel, Explore, usage, Send ↔ Stop), the queue-or-stop
 * prompt, the skip-all line and the engine line. Sizes are the design's "Composer · Send" (C08).
 */
import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
import { copy } from "@vellum/config";
import {
  type AgentController,
  type ComposerController,
  type ContextChip,
  type ExploreVary,
  type ParallelMode,
  usageView,
} from "@vellum/editor-core";
import { type ReactNode, type RefObject, useEffect, useRef } from "react";
import { ActorMarker, type ActorView } from "../chrome/ActorMarker";
import { usePortalContainer } from "../chrome/portal";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { EnginePicker } from "./EnginePicker";
import "./composer.css";

/** What only the host can do for the composer. */
export interface ComposerHost {
  /** Attach (+): the host's file picker. Dropped files come here too. */
  attach?(files?: readonly File[]): Promise<ContextChip[]>;
  /** C02: Connect your coding agent. */
  onConnectAgents?(): void;
  /** S01: the daemon is not running; Restart asks the host to start it. */
  offline?: boolean;
  onRestart?(): void;
  /** S04, S05: the composer cannot send, and why. Wins over `offline`. */
  blocked?: { text: string; action?: { label: string; run(): void } };
  /** Bundled design systems and visual styles for the first-open suggestions (C01). */
  designSystems?: readonly { id: string; name: string; swatches: readonly string[] }[];
  visualStyles?: readonly {
    id: string;
    name: string;
    /** "Inter", "Lora", "JetBrains Mono": the type sample's family. */
    font: string;
    palette: readonly string[];
  }[];
  timeZone?: string;
}

export function Composer({
  composer,
  agent,
  actor,
  host,
  field,
  compact = false,
}: {
  compact?: boolean;
  composer: ComposerController;
  agent: AgentController;
  actor(id: string | null | undefined): ActorView;
  host: ComposerHost;
  field: RefObject<HTMLTextAreaElement | null>;
}) {
  useStore(composer.subscribe);
  const box = useRef<HTMLDivElement>(null);
  const s = composer.state;
  const draft = composer.draft();
  const chips = composer.chips();
  const action = composer.action();
  const [offlineLine, restart] = copy.composerWhenOffline.split(" · ");
  const blocked =
    host.blocked ??
    (host.offline
      ? {
          text: offlineLine as string,
          ...(host.onRestart ? { action: { label: restart as string, run: host.onRestart } } : {}),
        }
      : null);
  const offline = !!blocked;
  const hasTurns = agent.turns().length > 0;
  const model = composer.modelLine();
  const focus = s.focus;
  useEffect(() => {
    if (focus) field.current?.focus();
  }, [focus, field]);
  return (
    <div className="vk-composer-wrap" data-compact={compact ? "" : undefined}>
      {s.skipAll ? (
        <div className="vk-skipall" role="status">
          <Icon name="triangle-alert" size={14} />
          <span className="vk-skipall__text">
            Permission prompts skipped · Files and commands run without asking.
          </span>
          <button type="button" className="va-link" onClick={() => void composer.turnPromptsOn()}>
            Turn prompts on
          </button>
        </div>
      ) : null}
      {s.queue ? <QueuePrompt composer={composer} /> : null}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: a drop target for files; the field and buttons are the controls */}
      <div
        className="vk-composer"
        ref={box}
        data-disabled={offline ? "" : undefined}
        onDragOver={(e) => {
          if (host.attach && e.dataTransfer.types.includes("Files")) e.preventDefault();
        }}
        onDrop={(e) => {
          if (!host.attach || !e.dataTransfer.files.length) return;
          e.preventDefault();
          void host.attach([...e.dataTransfer.files]).then((list) => composer.attach(list));
        }}
      >
        {chips.length ? (
          <ul className="vk-chips" aria-label="Attached context">
            {chips.map((c) => (
              <li key={c.key} className="vk-chip" data-kind={c.kind}>
                <span className="vk-chip__label">{c.label}</span>
                <button
                  type="button"
                  className="vk-chip__remove"
                  aria-label={`Remove ${c.label}`}
                  onClick={() => composer.removeChip(c.key)}
                >
                  <Icon name="x" size={12} />
                </button>
              </li>
            ))}
          </ul>
        ) : null}
        <textarea
          ref={field}
          className="vk-field"
          rows={1}
          value={draft.text}
          disabled={offline}
          placeholder={composer.placeholder()}
          aria-label="Request"
          role="combobox"
          aria-expanded={!!s.menu}
          aria-controls={s.menu ? "vk-menu" : undefined}
          aria-autocomplete="list"
          aria-activedescendant={s.menu ? `vk-option-${s.menu.active}` : undefined}
          onChange={(e) => composer.setText(e.target.value, e.target.selectionStart)}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (composer.key(e.key, { shift: e.shiftKey })) e.preventDefault();
          }}
          onInput={(e) => {
            const el = e.currentTarget;
            el.style.height = "auto";
            el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
          }}
        />
        <div className="vk-controls">
          <AttachMenu composer={composer} host={host} field={field} disabled={offline} />
          <ParallelPopover composer={composer} disabled={offline} />
          <ExplorePopover composer={composer} disabled={offline} />
          <UsageMeter composer={composer} host={host} />
          {action.type === "stop" ? (
            <StopButton composer={composer} agents={action.agents} actor={actor} />
          ) : (
            <button
              type="button"
              className="vk-send"
              disabled={offline || s.sending || (!draft.text.trim() && !draft.chips.length)}
              onClick={() => void composer.send()}
            >
              Send
            </button>
          )}
        </div>
        <MentionMenu composer={composer} anchor={box} />
      </div>
      {blocked ? (
        <p className="vk-offline" role="status">
          <span>{blocked.text}</span>
          {blocked.action ? (
            <button type="button" className="va-link" onClick={blocked.action.run}>
              {blocked.action.label}
            </button>
          ) : null}
        </p>
      ) : (
        <>
          {model ? (
            <EnginePicker composer={composer} hasTurns={hasTurns}>
              <span className="vk-model vc-mono">{model}</span>
            </EnginePicker>
          ) : null}
          {!hasTurns && host.onConnectAgents ? (
            <button type="button" className="va-link vk-connect" onClick={host.onConnectAgents}>
              Connect your coding agent
            </button>
          ) : null}
        </>
      )}
      {s.error ? (
        <p className="va-error vk-error" role="alert">
          <Icon name="circle-alert" size={14} />
          <span>{s.error}</span>
          <button type="button" className="va-link" onClick={() => composer.clearError()}>
            Dismiss
          </button>
        </p>
      ) : null}
    </div>
  );
}

/** "Send while an agent works" (Turn / Queue): Queue after this turn is the default. */
function QueuePrompt({ composer }: { composer: ComposerController }) {
  return (
    <section className="vk-queue" aria-label="Send while an agent works">
      <p className="vk-queue__title">Send while an agent works</p>
      <p className="va-prose">Your draft is kept until the current turn finishes.</p>
      <button
        type="button"
        className="va-button"
        data-primary=""
        // biome-ignore lint/a11y/noAutofocus: the default answer takes focus, as a dialog's does
        autoFocus
        onClick={() => void composer.resolveQueue("after")}
        onKeyDown={(e) => {
          if (e.key === "Escape") composer.cancelQueue();
        }}
      >
        Queue after this turn
      </button>
      <button type="button" className="va-button" onClick={() => void composer.resolveQueue("now")}>
        Stop and send now
      </button>
    </section>
  );
}

/** The @ mention and / skills menus (C26, "Composer / Mentions"). Focus stays in the field. */
function MentionMenu({
  composer,
  anchor,
}: {
  composer: ComposerController;
  anchor: RefObject<HTMLDivElement | null>;
}) {
  const container = usePortalContainer();
  const menu = composer.state.menu;
  if (!menu) return null;
  const items = composer.menuItems();
  let index = -1;
  const option = (line: ReactNode, key: string, mono = false, icon?: string) => {
    index += 1;
    const i = index;
    return (
      <div
        key={key}
        id={`vk-option-${i}`}
        role="option"
        tabIndex={-1}
        aria-selected={menu.active === i}
        className="vk-menu__row"
        data-mono={mono ? "" : undefined}
        onPointerDown={(e) => e.preventDefault()}
        onPointerMove={() => menu.active !== i && composer.setActive(i)}
        onClick={() => composer.pick(i)}
        onKeyDown={() => {}}
      >
        {icon ? <Icon name={icon} size={16} /> : null}
        <span>{line}</span>
      </div>
    );
  };
  return (
    <Popover.Root
      open
      onOpenChange={(open) => {
        if (!open) composer.closeMenu();
      }}
    >
      <Popover.Portal container={container}>
        <Popover.Positioner
          anchor={anchor}
          side="top"
          align="start"
          sideOffset={menu.type === "skills" ? -90 : 4}
          alignOffset={-8}
          collisionPadding={8}
        >
          <Popover.Popup
            className="vk-menu"
            data-kind={menu.type}
            initialFocus={false}
            finalFocus={false}
          >
            <div
              id="vk-menu"
              role="listbox"
              aria-label={menu.type === "skills" ? "Skills" : "Mentions"}
            >
              {menu.type === "skills" ? (
                <>
                  {composer.skillGroups().map((g) => (
                    // biome-ignore lint/a11y/useSemanticElements: a listbox group; a fieldset is for form controls
                    <div key={g.id} role="group" aria-label={g.group} className="vk-menu__group">
                      <p className="vk-menu__label" aria-hidden>
                        {g.group}
                      </p>
                      {g.skills.map((skill) =>
                        option(
                          g.id === "user" ? `${skill.name} · ${skill.path}` : skill.name,
                          skill.path,
                          g.id === "user",
                          "book-open",
                        ),
                      )}
                    </div>
                  ))}
                  {items.length === 0 ? <p className="vk-menu__empty">No skills match</p> : null}
                </>
              ) : items.length ? (
                items.map((item, i) => option(item.line, `${item.kind}:${i}`))
              ) : (
                <p className="vk-menu__empty">
                  {menu.query
                    ? `Nothing matches @${menu.query}`
                    : "Type to search files, frames, components and variables"}
                </p>
              )}
            </div>
            {menu.type === "skills" ? (
              <button
                type="button"
                className="va-button vk-menu__add"
                onPointerDown={(e) => e.preventDefault()}
                onClick={() => void composer.addSkill()}
              >
                Add skill…
              </button>
            ) : null}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

function AttachMenu({
  composer,
  host,
  field,
  disabled,
}: {
  composer: ComposerController;
  host: ComposerHost;
  field: RefObject<HTMLTextAreaElement | null>;
  disabled: boolean;
}) {
  const container = usePortalContainer();
  return (
    <Menu.Root>
      <Menu.Trigger className="vc-iconbutton vk-attach" aria-label="Attach" disabled={disabled}>
        <Icon name="plus" />
      </Menu.Trigger>
      <Menu.Portal container={container}>
        <Menu.Positioner side="top" align="start" sideOffset={8}>
          <Menu.Popup className="vk-popover vk-popover--menu">
            {host.attach ? (
              <Menu.Item
                className="vk-popover__row"
                onClick={() => void host.attach?.().then((list) => composer.attach(list))}
              >
                <Icon name="file" size={16} />
                Attach a file…
              </Menu.Item>
            ) : null}
            <Menu.Item
              className="vk-popover__row"
              onClick={() => {
                composer.mention([]);
                field.current?.focus();
              }}
            >
              <Icon name="search" size={16} />
              Mention a file, frame, component or variable
            </Menu.Item>
            <Menu.Item
              className="vk-popover__row"
              onClick={() => {
                composer.setText("/", 1);
                field.current?.focus();
              }}
            >
              <Icon name="book-open" size={16} />
              Use a skill
            </Menu.Item>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

function Stepper({
  label,
  value,
  min,
  max,
  onChange,
  format = String,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange(n: number): void;
  format?: (n: number) => string;
}) {
  return (
    <fieldset className="vk-stepper">
      <legend className="vk-popover__label">{label}</legend>
      <button
        type="button"
        className="vc-iconbutton"
        aria-label={`Fewer: ${label}`}
        disabled={value <= min}
        onClick={() => onChange(value - 1)}
      >
        <Icon name="minus" size={14} />
      </button>
      <output className="vk-stepper__value" aria-live="polite">
        {format(value)}
      </output>
      <button
        type="button"
        className="vc-iconbutton"
        aria-label={`More: ${label}`}
        disabled={value >= max}
        onClick={() => onChange(value + 1)}
      >
        <Icon name="plus" size={14} />
      </button>
    </fieldset>
  );
}

function Choice<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly (readonly [T, string])[];
  onChange(v: T): void;
}) {
  return (
    <div className="vk-choice" role="radiogroup" aria-label={label}>
      {options.map(([id, text]) => (
        // biome-ignore lint/a11y/useSemanticElements: a segmented control; radio inputs would not take this style
        <button
          key={id}
          type="button"
          role="radio"
          aria-checked={value === id}
          className="vk-choice__option"
          onClick={() => onChange(id)}
        >
          {text}
        </button>
      ))}
    </div>
  );
}

function FooterPopover({
  composer,
  type,
  label,
  disabled,
  children,
}: {
  composer: ComposerController;
  type: "parallel" | "explore" | "usage";
  label: ReactNode;
  disabled?: boolean;
  children: ReactNode;
}) {
  const container = usePortalContainer();
  const open = composer.state.surface?.type === type;
  return (
    <Popover.Root open={open} onOpenChange={(next) => composer.open(next ? { type } : null)}>
      <Popover.Trigger className={`vk-footer-button vk-footer-button--${type}`} disabled={disabled}>
        {label}
      </Popover.Trigger>
      <Popover.Portal container={container}>
        <Popover.Positioner
          side="top"
          align="start"
          sideOffset={type === "usage" ? 12 : 8}
          collisionPadding={8}
        >
          <Popover.Popup className={`vk-popover vk-popover--${type}`}>{children}</Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

function ParallelPopover({
  composer,
  disabled,
}: {
  composer: ComposerController;
  disabled: boolean;
}) {
  const p = composer.state.parallel;
  return (
    <FooterPopover
      composer={composer}
      type="parallel"
      disabled={disabled}
      label={composer.parallelLabel()}
    >
      <Popover.Title className="vk-popover__title">Parallel</Popover.Title>
      <Stepper
        label="Agents"
        value={p.count}
        min={1}
        max={6}
        format={(n) => `${n}×`}
        onChange={(count) => composer.setParallel({ ...p, count })}
      />
      <Choice<ParallelMode>
        label="Mode"
        value={p.mode}
        options={[
          ["split", "Split"],
          ["side_by_side", "Side by side"],
        ]}
        onChange={(mode) => composer.setParallel({ ...p, mode })}
      />
      <p className="vk-popover__note">
        {p.mode === "split"
          ? "Agents divide the request between them."
          : "Each agent builds the whole request, for comparison."}
      </p>
    </FooterPopover>
  );
}

function ExplorePopover({
  composer,
  disabled,
}: {
  composer: ComposerController;
  disabled: boolean;
}) {
  const e = composer.state.explore;
  return (
    <FooterPopover
      composer={composer}
      type="explore"
      disabled={disabled}
      label={composer.exploreLabel()}
    >
      <Popover.Title className="vk-popover__title">Explore</Popover.Title>
      <Stepper
        label="Variants"
        value={e.count}
        min={0}
        max={6}
        format={(n) => (n ? String(n) : "Off")}
        onChange={(n) =>
          composer.setExplore({ ...e, count: n === 1 ? (e.count === 0 ? 2 : 0) : n })
        }
      />
      <Choice<ExploreVary>
        label="Vary"
        value={e.vary}
        options={[
          ["layout", "Layout"],
          ["style", "Style"],
        ]}
        onChange={(vary) => composer.setExplore({ ...e, vary })}
      />
      <p className="vk-popover__note">Variants arrive one after another, after the first result.</p>
    </FooterPopover>
  );
}

/** The ring: how full the model's context is. The popover has tokens by kind and the estimated cost. */
function UsageMeter({ composer, host }: { composer: ComposerController; host: ComposerHost }) {
  const usage = composer.usage();
  const view = usage ? usageView(usage, host.timeZone ? { timeZone: host.timeZone } : {}) : null;
  const fraction = view?.fraction ?? 0;
  const r = 6;
  const length = 2 * Math.PI * r;
  return (
    <FooterPopover
      composer={composer}
      type="usage"
      label={
        <svg
          className="vk-ring"
          width="14"
          height="14"
          viewBox="0 0 14 14"
          role="img"
          aria-label={view ? view.title : "Context usage"}
        >
          <circle cx="7" cy="7" r={r} className="vk-ring__track" />
          <circle
            cx="7"
            cy="7"
            r={r}
            className="vk-ring__fill"
            strokeDasharray={`${fraction * length} ${length}`}
            transform="rotate(-90 7 7)"
          />
        </svg>
      }
    >
      <Popover.Title className="vk-popover__title vk-usage__title">
        {view?.title ?? "Context usage"}
      </Popover.Title>
      {view ? (
        <>
          <dl className="vk-usage__rows">
            {view.rows.map(([label, value]) => (
              <div key={label} className="vk-usage__row">
                <dt>{label}</dt>
                <dd className="vc-mono">{value}</dd>
              </div>
            ))}
          </dl>
          {view.plan ? <p className="vk-usage__cost">{view.plan}</p> : null}
          {view.cost ? <p className="vk-usage__cost">{view.cost}</p> : null}
          {view.note ? <p className="vk-usage__note">{view.note}</p> : null}
        </>
      ) : (
        <p className="vk-usage__note">Usage appears after the first turn.</p>
      )}
    </FooterPopover>
  );
}

/** Stop; with several agents working, Stop this agent per agent and Stop all. */
function StopButton({
  composer,
  agents,
  actor,
}: {
  composer: ComposerController;
  agents: readonly string[];
  actor(id: string | null | undefined): ActorView;
}) {
  const container = usePortalContainer();
  if (agents.length < 2)
    return (
      <button
        type="button"
        className="vk-send"
        data-stop=""
        onClick={() => void composer.stopAll()}
      >
        Stop
      </button>
    );
  return (
    <Menu.Root>
      <Menu.Trigger className="vk-send" data-stop="">
        Stop
      </Menu.Trigger>
      <Menu.Portal container={container}>
        <Menu.Positioner side="top" align="end" sideOffset={8}>
          <Menu.Popup className="vk-popover vk-popover--menu">
            <Menu.Group>
              <Menu.GroupLabel className="vk-popover__label">Stop this agent</Menu.GroupLabel>
              {agents.map((id) => (
                <Menu.Item
                  key={id}
                  className="vk-popover__row"
                  onClick={() => void composer.stopAgent(id)}
                >
                  <ActorMarker actor={actor(id)} size={16} outlined />
                  {actor(id).name}
                </Menu.Item>
              ))}
            </Menu.Group>
            <Menu.Separator className="vk-popover__separator" />
            <Menu.Item className="vk-popover__row" onClick={() => void composer.stopAll()}>
              Stop all
            </Menu.Item>
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
