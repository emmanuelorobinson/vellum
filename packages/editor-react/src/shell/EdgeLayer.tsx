import { copy } from "@vellum/config";
import type {
  Act,
  Banner,
  EdgeEnvironment,
  EdgeHost,
  EdgeUi,
  MissingFont,
  Notice,
  Unreadable,
} from "@vellum/editor-core";
import { useState } from "react";
import { useLocale } from "../accessibility/locale";
import { ActorMarker, type ActorView } from "../chrome/ActorMarker";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ResourceButton, ResourceDialog } from "../resources/Dialog";
import "./shell.css";

function ActButton({ act, primary, run }: { act: Act; primary?: boolean; run(a: Act): void }) {
  return (
    <button
      type="button"
      className="vx-button"
      data-primary={primary ? "" : undefined}
      onClick={() => run(act)}
    >
      {act.label}
    </button>
  );
}

/**
 * The Agent panel's notice (S01–S06, S08, S09, S11, S16): what is wrong, in words, one next step, and what is still
 * fine. It takes the place of the first-open prompt, under the session's agent and model.
 */
export function AgentNotice({
  notice,
  actor,
  model,
  run,
}: {
  notice: Notice;
  actor: ActorView;
  model: string | null;
  run(a: Act): void;
}) {
  return (
    <section
      className="ve-notice"
      data-tone={notice.tone}
      data-notice={notice.id}
      aria-label={notice.heading}
    >
      <div className="ve-notice__who">
        <ActorMarker actor={actor} size={20} />
        <strong>{actor.name}</strong>
        <span className="ve-notice__status">{notice.status}</span>
      </div>
      {model ? <p className="ve-notice__model vc-mono">{model}</p> : null}
      <h3 role="status">{notice.heading}</h3>
      <p className="ve-notice__body">{notice.body}</p>
      {notice.command ? <p className="ve-notice__command vc-mono">{notice.command}</p> : null}
      {notice.link ? (
        <button type="button" className="ve-link" onClick={() => run(notice.link as Act)}>
          {notice.link.label} <Icon name="arrow-up-right" size={12} />
        </button>
      ) : null}
      {notice.action ? (
        <div className="ve-notice__actions">
          <ActButton act={notice.action} primary run={run} />
          {notice.secondary?.map((a) => (
            <ActButton key={a.id} act={a} run={run} />
          ))}
        </div>
      ) : null}
      {notice.footnote ? <p className="ve-notice__foot">{notice.footnote}</p> : null}
    </section>
  );
}

/** S02, S04, S06: one quiet strip under the document bar. Never blocks. */
export function EdgeBannerView({
  banner,
  top,
  inset,
  run,
}: {
  banner: Banner;
  top: number;
  inset: number;
  run(a: Act): void;
}) {
  return (
    <div
      className="ve-banner"
      role="status"
      data-tone={banner.tone}
      data-banner={banner.id}
      style={{ top }}
    >
      <span style={{ paddingInlineStart: inset }}>
        <Icon name="info" size={16} />
        {banner.message}
      </span>
      <button type="button" className="ve-link" onClick={() => run(banner.action)}>
        {banner.action.label}
      </button>
    </div>
  );
}

export interface FontsHost {
  /** Selects the layers that use the font. */
  jump(nodes: readonly string[]): void;
  install?(family: string): void;
  /** Families that can replace a missing one. */
  available: readonly string[];
  replace?(family: string, to: string): void;
  addFolder?(): void;
}

function MissingFontsDialog({
  fonts,
  host,
  onClose,
  readOnly,
}: {
  fonts: readonly MissingFont[];
  host: FontsHost;
  onClose(): void;
  readOnly: boolean;
}) {
  const [choosing, setChoosing] = useState<string | null>(null);
  const substitute = fonts[0]?.substitute ?? "A substitute font";
  return (
    <ResourceDialog
      title="Missing fonts"
      description={`${substitute} substitutes missing fonts so you can continue editing.`}
      width={680}
      className="ve-fonts"
      onClose={onClose}
    >
      {fonts.map((f) => (
        <section key={f.family} className="ve-font" aria-label={f.family}>
          <h3>{f.family}</h3>
          <p>
            {f.layers} {f.layers === 1 ? "layer" : "layers"} · substitute: {f.substitute}
          </p>
          <div className="vc-resource-actions">
            <ResourceButton onClick={() => host.jump(f.nodes)}>Jump to layers</ResourceButton>
            <ResourceButton disabled={!host.install} onClick={() => host.install?.(f.family)}>
              Install from Google Fonts
            </ResourceButton>
            <ResourceButton
              disabled={readOnly || !host.replace}
              onClick={() => setChoosing(choosing === f.family ? null : f.family)}
            >
              Choose replacement
            </ResourceButton>
          </div>
          {choosing === f.family ? (
            <select
              className="vs-select"
              aria-label={`Replace ${f.family}`}
              defaultValue=""
              onChange={(e) => {
                host.replace?.(f.family, e.target.value);
                setChoosing(null);
              }}
            >
              <option value="" disabled>
                Choose replacement
              </option>
              {host.available.map((a) => (
                <option key={a}>{a}</option>
              ))}
            </select>
          ) : null}
        </section>
      ))}
      <div className="vc-resource-actions">
        <ResourceButton disabled={!host.addFolder} onClick={() => host.addFolder?.()}>
          Add font folder
        </ResourceButton>
      </div>
    </ResourceDialog>
  );
}

function UnreadableDialog({
  file,
  error,
  run,
}: {
  file: string;
  error: Unreadable;
  run(a: Act): void;
}) {
  const { t: translateUi } = useLocale();
  return (
    <ResourceDialog
      title={`${file} can’t be read`}
      description={
        error.summary ?? "The file contains invalid JSON. No partial design has been rendered."
      }
      width={620}
      className="ve-unreadable"
      onClose={() => {}}
    >
      <p className="ve-where vc-mono">
        {translateUi("Line")} {error.line} · column {error.column}
      </p>
      <p className="ve-detail vc-mono">{error.detail ?? error.reason}</p>
      <div className="vc-resource-actions">
        <ResourceButton primary onClick={() => run({ id: "openAsText", label: "Open as text" })}>
          Open as text
        </ResourceButton>
        <ResourceButton onClick={() => run({ id: "formatSpec", label: "Format specification" })}>
          Format specification
        </ResourceButton>
      </div>
    </ResourceDialog>
  );
}

function UnsavedDialog({
  file,
  choose,
}: {
  file: string;
  choose(c: "cancel" | "discard" | "save"): void;
}) {
  const { t: translateUi } = useLocale();
  return (
    <ResourceDialog
      title={`Save ${file}?`}
      description="Your changes have not been written to disk."
      width={400}
      className="ve-unsaved"
      onClose={() => choose("cancel")}
    >
      <div className="vc-resource-actions ve-unsaved__actions">
        <ResourceButton onClick={() => choose("cancel")}>{translateUi("Cancel")}</ResourceButton>
        <ResourceButton onClick={() => choose("discard")}>Don’t save</ResourceButton>
        <ResourceButton primary onClick={() => choose("save")}>
          Save
        </ResourceButton>
      </div>
    </ResourceDialog>
  );
}

function SignInDialog({
  notice,
  run,
  onClose,
}: {
  notice: Notice;
  run(a: Act): void;
  onClose(): void;
}) {
  return (
    <ResourceDialog
      title={notice.heading}
      description={notice.body}
      width={600}
      className="ve-signin"
      onClose={onClose}
    >
      {notice.command ? <p className="ve-command vc-mono">{notice.command}</p> : null}
      <div className="vc-resource-actions">
        {notice.action ? (
          <ResourceButton primary onClick={() => run(notice.action as Act)}>
            {notice.action.label}
          </ResourceButton>
        ) : null}
        {notice.secondary?.map((a) => (
          <ResourceButton key={a.id} onClick={() => run(a)}>
            {a.label}
          </ResourceButton>
        ))}
      </div>
    </ResourceDialog>
  );
}

export interface Rejection {
  /** Whose change was refused ("Saffron"). */
  agent: string;
  /** Who holds the lease ("Teal"). */
  leasedBy: string;
  /** The agent the person is following, when they are. */
  following: string | null;
}

/**
 * The surfaces an edge state puts over the editor: dialogs (S03, S05, S11, S16) and the toasts at the bottom (S01,
 * S07). The banner and the notice are placed by the workspace, which knows where the panels are.
 */
export function EdgeDialogs({
  env,
  ui,
  host,
  file,
  notice,
  fontsHost,
  readOnly,
  run,
  rejection,
  toastLeft,
}: {
  env: EdgeEnvironment;
  ui: EdgeUi;
  host: EdgeHost;
  file: string;
  notice: Notice | null;
  fontsHost: FontsHost;
  readOnly: boolean;
  run(a: Act): void;
  rejection?: Rejection | null | undefined;
  toastLeft: number;
}) {
  useStore(ui.subscribe);
  const signIn =
    env.problem?.kind === "not-signed-in" && notice?.id === "not-signed-in" ? notice : null;
  return (
    <>
      {env.unreadable ? <UnreadableDialog file={file} error={env.unreadable} run={run} /> : null}
      {!env.unreadable && env.closing ? (
        <UnsavedDialog file={env.closing.fileName} choose={(c) => host.closeChoice?.(c)} />
      ) : null}
      {ui.state.fonts && env.fonts.length ? (
        <MissingFontsDialog
          fonts={env.fonts}
          host={fontsHost}
          readOnly={readOnly}
          onClose={() => ui.closeFonts()}
        />
      ) : null}
      {signIn && !ui.isDismissed("sign-in") ? (
        <SignInDialog notice={signIn} run={run} onClose={() => ui.dismiss("sign-in")} />
      ) : null}
      {env.daemon === "offline" && !ui.isDismissed("daemon-toast") ? (
        <div
          className="ve-toast"
          role="status"
          aria-label={copy.daemonOffline}
          style={{ left: toastLeft }}
        >
          <strong>{copy.daemonOffline}</strong>
          <p>Editing and saving still work, but agents cannot connect.</p>
          <div className="vc-resource-actions">
            <ResourceButton onClick={() => run({ id: "restartDaemon", label: "Restart daemon" })}>
              Restart daemon
            </ResourceButton>
            <ResourceButton onClick={() => run({ id: "troubleshoot", label: "Troubleshoot" })}>
              Troubleshoot
            </ResourceButton>
          </div>
          <button
            type="button"
            className="vc-iconbutton ve-toast__close"
            aria-label="Dismiss"
            onClick={() => ui.dismiss("daemon-toast")}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      ) : null}
      {rejection && !ui.isDismissed("rejection") ? (
        <div
          className="ve-toast"
          role="status"
          aria-label="Write rejected"
          style={{ left: toastLeft }}
        >
          <strong data-tone="error">{rejection.agent}’s change rejected</strong>
          <p>
            {copy.writeRejected.replace("{agent}", rejection.leasedBy)}.
            {rejection.following ? ` You are following ${rejection.following}.` : ""}
          </p>
          <div className="vc-resource-actions">
            <ResourceButton onClick={() => run({ id: "reviewChanges", label: "View in ledger" })}>
              View in ledger
            </ResourceButton>
          </div>
          <button
            type="button"
            className="vc-iconbutton ve-toast__close"
            aria-label="Dismiss"
            onClick={() => ui.dismiss("rejection")}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      ) : null}
    </>
  );
}
