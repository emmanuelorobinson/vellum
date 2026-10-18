import { Menu } from "@base-ui/react/menu";
import { Select } from "@base-ui/react/select";
import { Switch } from "@base-ui/react/switch";
import {
  type FieldSpec,
  type FieldValue,
  formatField,
  parseField,
  scrubField,
  stepField,
  type VariableBinding,
  type VariableInfo,
} from "@vellum/editor-core";
import {
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { usePortalContainer } from "../chrome/portal";
import { Icon } from "../icons/Icon";
import "./design.css";

/* ---------------------------------------------------------------------------------------- value field */

export interface ValueFieldProps {
  /** What the person calls it, for assistive technology: "Gap", "Width". */
  label: string;
  value: FieldValue<number>;
  spec: FieldSpec;
  onCommit(value: number): void;
  /** A letter or word inside the field, before the number: "X", "Padding". Dragging it scrubs the value. */
  axis?: string;
  disabled?: boolean;
  /** Why it is disabled, shown as a tooltip. */
  reason?: string;
  className?: string;
  /** Shown when there is no value to show ("Auto"). A multi-selection that disagrees always reads "Mixed". */
  placeholder?: string;
  /** A word that stands for no number: typing it runs `commit` (line height "auto"). */
  keyword?: { text: string; commit(): void };
}

const MESSAGE_MS = 3000;
const SCRUB_THRESHOLD = 2;

const shown = (value: FieldValue<number>, spec: FieldSpec): string => {
  if (value.kind === "value") return formatField(value.value, spec);
  if (value.kind === "bound" && typeof value.variable.value === "number")
    return formatField(value.variable.value, spec);
  return "";
};
const current = (value: FieldValue<number>): number | null =>
  value.kind === "value"
    ? value.value
    : value.kind === "bound" && typeof value.variable.value === "number"
      ? value.variable.value
      : null;

/**
 * A number the person can type, step with the arrow keys (one, ten with Shift, a tenth with Alt), write as a sum
 * ("16*2"), or scrub by dragging its label. A change the file would not accept reverts, with a brief message beside
 * the field. A multi-selection that disagrees reads "Mixed"; typing replaces every value.
 */
export function ValueField(props: ValueFieldProps) {
  const { label, value, spec, onCommit, axis, disabled, reason } = props;
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  const [text, setText] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const scrub = useRef<{ x: number; start: number; moved: boolean } | null>(null);
  // Escape drops the draft; the blur it causes must not write it.
  const reverting = useRef(false);
  const display = shown(value, spec);
  const state = disabled
    ? "disabled"
    : message
      ? "invalid"
      : value.kind === "mixed"
        ? "mixed"
        : value.kind === "bound"
          ? "bound"
          : "default";

  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(() => setMessage(null), MESSAGE_MS);
    return () => clearTimeout(timer);
  }, [message]);

  const settle = (v: number) => {
    setMessage(null);
    if (current(value) !== v || value.kind !== "value") onCommit(v);
    setText(null);
  };
  const commit = (raw: string) => {
    if (raw === display) {
      setText(null);
      return;
    }
    if (props.keyword && raw.trim().toLowerCase() === props.keyword.text.toLowerCase()) {
      setMessage(null);
      setText(null);
      props.keyword.commit();
      return;
    }
    const parsed = parseField(raw, spec);
    if (!parsed.ok) {
      setMessage(parsed.message);
      setText(null);
      return;
    }
    if (parsed.clamped) setMessage("Kept within the allowed range");
    if (current(value) !== parsed.value || value.kind !== "value") onCommit(parsed.value);
    setText(null);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commit(e.currentTarget.value);
      e.currentTarget.select();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      reverting.current = true;
      setText(null);
      setMessage(null);
      e.currentTarget.blur();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const typed = parseField(e.currentTarget.value, spec);
      const base = typed.ok ? typed.value : (current(value) ?? 0);
      const next = stepField(
        base,
        e.key === "ArrowUp" ? 1 : -1,
        { shift: e.shiftKey, alt: e.altKey },
        spec,
      );
      settle(next);
    }
  };
  const down = (e: PointerEvent<HTMLElement>) => {
    if (disabled || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    scrub.current = { x: e.clientX, start: current(value) ?? 0, moved: false };
  };
  const move = (e: PointerEvent<HTMLElement>) => {
    const s = scrub.current;
    if (!s) return;
    const dx = e.clientX - s.x;
    if (!s.moved && Math.abs(dx) < SCRUB_THRESHOLD) return;
    s.moved = true;
    setText(formatField(scrubField(s.start, dx, { shift: e.shiftKey, alt: e.altKey }, spec), spec));
  };
  const up = (e: PointerEvent<HTMLElement>) => {
    const s = scrub.current;
    scrub.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId))
      e.currentTarget.releasePointerCapture(e.pointerId);
    if (!s) return;
    if (!s.moved) {
      input.current?.focus();
      return;
    }
    settle(scrubField(s.start, e.clientX - s.x, { shift: e.shiftKey, alt: e.altKey }, spec));
  };

  return (
    <span className="vc-fieldwrap">
      <span
        className={props.className ? `vc-field ${props.className}` : "vc-field"}
        data-state={state}
        title={disabled ? reason : undefined}
      >
        {axis ? (
          <span
            className="vc-field__axis"
            data-scrub={disabled ? undefined : ""}
            onPointerDown={down}
            onPointerMove={move}
            onPointerUp={up}
            onPointerCancel={up}
          >
            {axis}
          </span>
        ) : null}
        <input
          ref={input}
          id={id}
          className="vc-field__input vc-mono-value"
          aria-label={label}
          aria-invalid={message ? true : undefined}
          disabled={disabled}
          inputMode="decimal"
          autoComplete="off"
          spellCheck={false}
          placeholder={value.kind === "mixed" ? "Mixed" : props.placeholder}
          value={text ?? display}
          onFocus={(e) => e.currentTarget.select()}
          onChange={(e) => setText(e.currentTarget.value)}
          onBlur={(e) => {
            if (reverting.current) {
              reverting.current = false;
              return;
            }
            if (text !== null) commit(e.currentTarget.value);
          }}
          onKeyDown={onKeyDown}
        />
      </span>
      {message ? (
        <span className="vc-field__message" role="alert">
          {message}
        </span>
      ) : null}
    </span>
  );
}

/* ----------------------------------------------------------------------------------------- text field */

export function TextField(props: {
  label: string;
  value: string | null;
  placeholder?: string;
  /** Returns a message when the text is not acceptable: it is shown beside the field and the old value stays. */
  onCommit(value: string): string | null | undefined;
  disabled?: boolean;
  mono?: boolean;
  /** Mixed across the selection. */
  mixed?: boolean;
  height?: 26 | 28;
  className?: string;
}) {
  const [text, setText] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const reverting = useRef(false);
  const display = props.mixed ? "" : (props.value ?? "");
  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(() => setMessage(null), MESSAGE_MS);
    return () => clearTimeout(timer);
  }, [message]);
  const commit = (raw: string) => {
    setText(null);
    if (raw === display) return;
    const problem = props.onCommit(raw);
    setMessage(problem ?? null);
  };
  const state = props.disabled
    ? "disabled"
    : message
      ? "invalid"
      : props.mixed
        ? "mixed"
        : "default";
  return (
    <span className="vc-fieldwrap">
      <span
        className={props.className ? `vc-field ${props.className}` : "vc-field"}
        data-state={state}
        {...(props.height === 28 ? { "data-height": "28" } : {})}
      >
        <input
          className={props.mono === false ? "vc-field__input" : "vc-field__input vc-mono-value"}
          aria-label={props.label}
          aria-invalid={message ? true : undefined}
          disabled={props.disabled}
          autoComplete="off"
          spellCheck={false}
          placeholder={props.mixed ? "Mixed" : props.placeholder}
          value={text ?? display}
          onFocus={(e) => e.currentTarget.select()}
          onChange={(e) => setText(e.currentTarget.value)}
          onBlur={(e) => {
            if (reverting.current) {
              reverting.current = false;
              return;
            }
            if (text !== null) commit(e.currentTarget.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              commit(e.currentTarget.value);
              e.currentTarget.select();
            } else if (e.key === "Escape") {
              e.stopPropagation();
              reverting.current = true;
              setText(null);
              setMessage(null);
              e.currentTarget.blur();
            }
          }}
        />
      </span>
      {message ? (
        <span className="vc-field__message" role="alert">
          {message}
        </span>
      ) : null}
    </span>
  );
}

/* -------------------------------------------------------------------------------------------- segmented */

export function Segmented<T extends string>(props: {
  label: string;
  value: T | null;
  options: readonly { value: T; label: ReactNode; title?: string; disabled?: boolean }[];
  onChange(value: T): void;
  disabled?: boolean;
  /** Nothing is selected because the selection disagrees. */
  mixed?: boolean;
  className?: string;
}) {
  const name = useId();
  const { options, value } = props;
  return (
    <fieldset
      className={props.className ? `vc-seg ${props.className}` : "vc-seg"}
      aria-label={props.label}
      disabled={props.disabled}
      data-mixed={props.mixed ? "" : undefined}
    >
      {options.map((o) => {
        const checked = !props.mixed && value === o.value;
        return (
          <label
            key={o.value}
            className="vc-seg__option"
            data-checked={checked ? "" : undefined}
            data-disabled={props.disabled || o.disabled ? "" : undefined}
            title={o.title}
          >
            <input
              type="radio"
              name={name}
              value={o.value}
              className="vc-seg__input"
              checked={checked}
              disabled={props.disabled || o.disabled}
              aria-label={typeof o.label === "string" ? undefined : (o.title ?? o.value)}
              onChange={() => props.onChange(o.value)}
            />
            {o.label}
          </label>
        );
      })}
    </fieldset>
  );
}

/* ---------------------------------------------------------------------------------------------- select */

export function SelectField<T extends string>(props: {
  label: string;
  value: T | null;
  options: readonly { value: T; label: string; disabled?: boolean }[];
  onChange(value: T): void;
  disabled?: boolean;
  mixed?: boolean;
  className?: string;
  /** Shown when nothing is chosen. */
  placeholder?: string;
  /** What the field reads instead of the chosen option's label ("Normal · 100%"). */
  display?: string;
}) {
  const container = usePortalContainer();
  const items = props.options.map((o) => ({ value: o.value, label: o.label }));
  return (
    <Select.Root
      items={items}
      value={props.mixed ? null : props.value}
      onValueChange={(v) => {
        if (v !== null) props.onChange(v as T);
      }}
      disabled={props.disabled}
    >
      <Select.Trigger
        className={props.className ? `vc-field vc-select ${props.className}` : "vc-field vc-select"}
        aria-label={props.label}
        data-state={props.disabled ? "disabled" : props.mixed ? "mixed" : "default"}
        data-nocaret={props.display !== undefined ? "" : undefined}
      >
        <Select.Value
          className="vc-select__value vc-mono-value"
          placeholder={props.mixed ? "Mixed" : (props.placeholder ?? "")}
        >
          {props.display !== undefined && !props.mixed ? () => props.display : undefined}
        </Select.Value>
        <Select.Icon className="vc-select__icon" aria-hidden>
          ▾
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal container={container}>
        <Select.Positioner
          className="vc-select__positioner"
          sideOffset={4}
          alignItemWithTrigger={false}
        >
          <Select.Popup className="vc-menu vc-select__popup">
            <Select.List>
              {props.options.map((o) => (
                <Select.Item
                  key={o.value}
                  value={o.value}
                  disabled={o.disabled}
                  className="vc-menu__item vc-select__item"
                >
                  <Select.ItemIndicator className="vc-select__check">
                    <Icon name="check" size={14} />
                  </Select.ItemIndicator>
                  <Select.ItemText>{o.label}</Select.ItemText>
                </Select.Item>
              ))}
            </Select.List>
          </Select.Popup>
        </Select.Positioner>
      </Select.Portal>
    </Select.Root>
  );
}

/* ---------------------------------------------------------------------------------------------- switch */

export function ToggleRow(props: {
  label: string;
  checked: boolean;
  onChange(checked: boolean): void;
  disabled?: boolean;
  mixed?: boolean;
  /** A Reset next to the label, for an instance's own choice. */
  reset?: ReactNode;
}) {
  const id = useId();
  return (
    <div className="vc-togglerow">
      <label className="vc-togglerow__label" htmlFor={id}>
        {props.label}
      </label>
      {props.reset}
      <Switch.Root
        id={id}
        className="vc-switch"
        checked={props.checked}
        disabled={props.disabled}
        data-mixed={props.mixed ? "" : undefined}
        onCheckedChange={props.onChange}
      >
        <Switch.Thumb className="vc-switch__thumb" />
      </Switch.Root>
    </div>
  );
}

export function IconToggle(props: {
  icon: string;
  label: string;
  pressed: boolean;
  onChange(pressed: boolean): void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className="vc-iconbutton vc-icontoggle"
      aria-label={props.label}
      title={props.label}
      aria-pressed={props.pressed}
      disabled={props.disabled}
      onClick={() => props.onChange(!props.pressed)}
    >
      <Icon name={props.icon} size={16} />
    </button>
  );
}

/* ------------------------------------------------------------------------------------------ structure */

export function Section(props: {
  className?: string;
  id: string;
  title: string;
  /** Spacing: dense for frames, roomy for text and components, wide for rows of detail (paint, stroke, effects). */
  density?: "dense" | "roomy" | "wide";
  /** Shown on hover and focus: Add, Remove. */
  actions?: ReactNode;
  /** Always shown at the end of the heading: a stroke's colour. */
  aside?: ReactNode;
  children?: ReactNode;
}) {
  const heading = `vc-section-${props.id}`;
  return (
    <section
      className={`vc-section ${props.className ?? ""}`}
      data-section={props.id}
      data-density={props.density ?? "dense"}
      aria-labelledby={heading}
    >
      <div className="vc-section__head">
        <h3 className="vc-heading" id={heading}>
          {props.title}
        </h3>
        {props.actions || props.aside ? (
          <span className="vc-section__tools">
            {props.actions ? <span className="vc-section__actions">{props.actions}</span> : null}
            {props.aside}
          </span>
        ) : null}
      </div>
      {props.children}
    </section>
  );
}

export function PropertyRow(props: { label: string; children: ReactNode }) {
  return (
    <div className="vc-prop">
      <span className="vc-prop__label">{props.label}</span>
      <span className="vc-prop__control">{props.children}</span>
    </div>
  );
}
export function Pair(props: { children: ReactNode }) {
  return <div className="vc-pair">{props.children}</div>;
}
export function TextButton(props: {
  children: ReactNode;
  onClick?(): void;
  disabled?: boolean;
  primary?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      className="vc-button"
      data-primary={props.primary ? "" : undefined}
      disabled={props.disabled}
      title={props.title}
      onClick={props.onClick}
    >
      {props.children}
    </button>
  );
}
export function Banner(props: { icon: string; children: ReactNode }) {
  return (
    <div className="vc-banner" role="status">
      <Icon name={props.icon} size={16} />
      <span>{props.children}</span>
    </div>
  );
}

/* ----------------------------------------------------------------------------------------- the chip */

export function VariableChip(props: {
  binding: VariableBinding;
  /** The unit to leave out of the chip text, as the design writes `{space/4} 16`. */
  suffix?: string;
  /** Other variables of the same type to switch to. */
  options?: readonly VariableInfo[];
  onBind?(id: string): void;
  onDetach?(): void;
  disabled?: boolean;
  /** Written as the chip's text when given; otherwise `{name} value`. */
  text?: string;
}) {
  const container = usePortalContainer();
  const { binding } = props;
  const missing = binding.name === null;
  const value =
    typeof binding.value === "number"
      ? String(Math.round(binding.value * 100) / 100)
      : binding.value === undefined
        ? ""
        : String(binding.value);
  const text =
    props.text ?? (missing ? "Missing variable" : `{${binding.name}}${value ? ` ${value}` : ""}`);
  const chip = (
    <span
      className="vc-chip"
      data-missing={missing ? "" : undefined}
      title={binding.name ?? undefined}
    >
      <span className="vc-mono-value">{text}</span>
    </span>
  );
  if (props.disabled || (!props.onDetach && !props.options?.length)) return chip;
  return (
    <Menu.Root>
      <Menu.Trigger
        className="vc-chip vc-chip--button"
        data-missing={missing ? "" : undefined}
        aria-label={`Variable ${binding.name ?? "missing"}`}
      >
        <span className="vc-mono-value">{text}</span>
      </Menu.Trigger>
      <Menu.Portal container={container}>
        <Menu.Positioner side="bottom" align="start" sideOffset={4}>
          <Menu.Popup className="vc-menu">
            {props.options?.length ? (
              <Menu.Group>
                <Menu.GroupLabel className="vc-menu__heading">Variables</Menu.GroupLabel>
                {props.options.map((o) => (
                  <Menu.Item
                    key={o.id}
                    className="vc-menu__item"
                    onClick={() => props.onBind?.(o.id)}
                  >
                    <span className="vc-mono-value">{o.name}</span>
                    <span className="vc-menu__key">{String(o.value ?? "")}</span>
                  </Menu.Item>
                ))}
              </Menu.Group>
            ) : null}
            {props.onDetach ? (
              <>
                {props.options?.length ? <Menu.Separator className="vc-menu__separator" /> : null}
                <Menu.Item className="vc-menu__item" onClick={props.onDetach}>
                  Detach variable
                </Menu.Item>
              </>
            ) : null}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}
