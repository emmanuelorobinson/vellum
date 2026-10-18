/**
 * The Agent tab (DESIGN.md "Signature component: Agent Panel"): session bar and switcher, the open session's turns,
 * and the composer pinned at the bottom (Prompt 50 supplies it; an external agent's session replaces it with a line).
 */
import { Menu } from "@base-ui/react/menu";
import { Popover } from "@base-ui/react/popover";
import { copy } from "@vellum/config";
import {
  type AgentController,
  connectedFor,
  createScrollFollow,
  type SessionInfo,
  sessionDot,
  sessionLine,
  sessionTitle,
  type TurnView,
} from "@vellum/editor-core";
import {
  type ReactNode,
  type RefObject,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { ActorMarker, type ActorView } from "../chrome/ActorMarker";
import { usePortalContainer } from "../chrome/portal";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ResourceButton, ResourceDialog } from "../resources/Dialog";
import { TurnItems } from "./Steps";
import "./agent.css";

export interface AgentTabProps {
  agent: AgentController;
  /** Every agent that can be named here, by agent id. */
  actors: Readonly<Record<string, ActorView>>;
  /** The composer (Prompt 50). */
  composer?: ReactNode;
}

const DOT_WORDS = {
  working: "working",
  "needs-you": "needs you",
  done: "done",
  failed: "failed",
} as const;

export function AgentTab({ agent, actors, composer }: AgentTabProps) {
  useStore(agent.subscribe);
  const s = agent.session();
  const turns = agent.turns();
  const external = s?.kind.type === "external";
  const actor = (id: string | null | undefined): ActorView =>
    (id && actors[id]) || { id: id ?? "", kind: "agent", name: "Agent" };
  const root = useRef<HTMLDivElement>(null);
  return (
    <div className="va-tab" ref={root}>
      <SessionBar agent={agent} session={s} actor={actor} />
      <Conversation agent={agent} turns={turns}>
        {external && s && turns.length === 0 ? (
          <section className="va-turn" aria-label={`${sessionTitle(s)} session`}>
            <ExternalHeader session={s} agent={agent} actor={actor(s.agent)} />
          </section>
        ) : null}
        {turns.map((t, i) => (
          <Turn
            key={t.key}
            turn={t}
            agent={agent}
            actor={actor}
            lead={
              external && s && i === 0 ? (
                <ExternalHeader session={s} agent={agent} actor={actor(s.agent)} />
              ) : null
            }
          />
        ))}
      </Conversation>
      {agent.state.error ? (
        <p className="va-error" role="alert">
          <Icon name="circle-alert" size={14} />
          <span>{agent.state.error}</span>
          <button type="button" className="va-link" onClick={() => agent.clearError()}>
            Dismiss
          </button>
        </p>
      ) : null}
      <div className="va-bottom">
        {external && s?.kind.type === "external" ? (
          <ExternalLine client={s.kind.client} onStop={() => void agent.stop()} />
        ) : (
          composer
        )}
      </div>
      <UndoConfirm agent={agent} actor={actor} root={root} />
    </div>
  );
}

function SessionBar({
  agent,
  session,
  actor,
}: {
  agent: AgentController;
  session: SessionInfo | null;
  actor(id: string | null | undefined): ActorView;
}) {
  const bar = useRef<HTMLDivElement>(null);
  const container = usePortalContainer();
  const [deleting, setDeleting] = useState<SessionInfo | null>(null);
  const time = agent.timeOptions();
  return (
    <div className="va-bar" ref={bar}>
      {session ? (
        <ActorMarker
          actor={{ ...actor(session.agent), state: DOT_WORDS[sessionDot(session)] }}
          size={16}
          outlined
        />
      ) : null}
      <span className="va-bar__title">{session ? sessionTitle(session) : "No session yet"}</span>
      <Menu.Root open={agent.state.switcher} onOpenChange={(open) => agent.toggleSwitcher(open)}>
        <Menu.Trigger className="vc-iconbutton va-bar__button" aria-label="Switch session">
          <Icon name="chevron-down" />
        </Menu.Trigger>
        <Menu.Portal container={container}>
          <Menu.Positioner
            anchor={bar}
            side="bottom"
            align="start"
            sideOffset={-16}
            alignOffset={8}
            collisionPadding={8}
          >
            <Menu.Popup className="va-switcher" aria-label="Sessions">
              {agent.groups().map((g) => (
                <Menu.Group key={g.group} className="va-switcher__group">
                  <Menu.GroupLabel className="va-switcher__label">{g.group}</Menu.GroupLabel>
                  {g.sessions.map((x) => (
                    <Menu.Item
                      key={x.id}
                      className="va-switcher__row"
                      data-current={x.id === agent.state.current ? "" : undefined}
                      onClick={() => void agent.select(x.id)}
                    >
                      <ActorMarker
                        actor={{ ...actor(x.agent), state: DOT_WORDS[sessionDot(x)] }}
                        size={20}
                        outlined
                      />
                      <span className="va-switcher__text">
                        <span className="va-switcher__title">
                          {x.kind.type === "external" ? x.kind.client : x.title}
                        </span>
                        <span className="va-switcher__line">{sessionLine(x, time)}</span>
                      </span>
                    </Menu.Item>
                  ))}
                </Menu.Group>
              ))}
              <div className="va-switcher__actions">
                <Menu.Item className="va-button" onClick={() => void agent.newSession()}>
                  New session
                </Menu.Item>
                {session ? (
                  <Menu.Item
                    className="va-link va-link--quiet"
                    onClick={() => setDeleting(session)}
                  >
                    Delete session
                  </Menu.Item>
                ) : null}
              </div>
            </Menu.Popup>
          </Menu.Positioner>
        </Menu.Portal>
      </Menu.Root>
      <button
        type="button"
        className="vc-iconbutton va-bar__button"
        aria-label="New session"
        onClick={() => void agent.newSession()}
      >
        <Icon name="plus" />
      </button>
      {deleting ? (
        <ResourceDialog
          title="Delete this session?"
          description={`${sessionTitle(deleting)}: its conversation and steps are removed from this computer. The changes its agents made stay in the file and in History.`}
          width={400}
          onClose={() => setDeleting(null)}
        >
          <div className="va-actions va-actions--dialog">
            <ResourceButton onClick={() => setDeleting(null)}>Cancel</ResourceButton>
            <button
              type="button"
              className="va-button"
              data-danger=""
              onClick={() => {
                const id = deleting.id;
                setDeleting(null);
                void agent.deleteSession(id);
              }}
            >
              Delete session
            </button>
          </div>
        </ResourceDialog>
      ) : null}
    </div>
  );
}

/** The scrolling list of turns. Streaming never moves a reader who scrolled up; "Latest" takes them down. */
function Conversation({
  agent,
  turns,
  children,
}: {
  agent: AgentController;
  turns: readonly TurnView[];
  children: ReactNode;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const [follow] = useState(createScrollFollow);
  const [latest, setLatest] = useState(false);
  useLayoutEffect(() => {
    const el = scroller.current;
    const inner = content.current;
    if (!el || !inner) return;
    const grew = () => {
      const { scrollTo } = follow.content({ height: el.scrollHeight, client: el.clientHeight });
      if (scrollTo !== null) el.scrollTop = scrollTo;
      setLatest(follow.state.latest);
    };
    grew();
    const ro = new ResizeObserver(grew);
    ro.observe(inner);
    return () => ro.disconnect();
  }, [follow]);
  // A different session starts at its newest turn.
  const session = agent.state.current;
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el || !session) return;
    el.scrollTop = follow.jump({ height: el.scrollHeight, client: el.clientHeight }).scrollTo;
    setLatest(false);
  }, [session, follow]);
  return (
    <div className="va-scroll-wrap">
      <div
        className="va-scroll"
        ref={scroller}
        onScroll={(e) => {
          const el = e.currentTarget;
          follow.scrolled({ top: el.scrollTop, height: el.scrollHeight, client: el.clientHeight });
          setLatest(follow.state.latest);
        }}
      >
        <div className="va-turns" ref={content} aria-live="polite" aria-relevant="additions">
          {children}
          {turns.length === 0 && !agent.state.loading ? (
            <p className="va-empty">{copy.composerPlaceholder}</p>
          ) : null}
        </div>
      </div>
      {latest ? (
        <button
          type="button"
          className="va-latest"
          onClick={() => {
            const el = scroller.current;
            if (!el) return;
            el.scrollTop = follow.jump({
              height: el.scrollHeight,
              client: el.clientHeight,
            }).scrollTo;
            setLatest(false);
          }}
        >
          Latest
          <Icon name="arrow-down" size={14} />
        </button>
      ) : null}
    </div>
  );
}

function ExternalHeader({
  session,
  agent,
  actor,
}: {
  session: SessionInfo;
  agent: AgentController;
  actor: ActorView;
}) {
  const client = session.kind.type === "external" ? session.kind.client : actor.name;
  return (
    <>
      <div className="va-header">
        <ActorMarker actor={actor} size={20} outlined />
        <span className="va-header__name">{client}</span>
        <span className="va-header__state">
          {session.connected
            ? `Connected · ${connectedFor(agent.timeOptions().now - session.createdAt, true)}`
            : "Disconnected"}
        </span>
      </div>
      {session.model ? <p className="va-model vc-mono">{session.model}</p> : null}
      <p className="va-prose">
        The conversation lives in {client}. Every canvas change appears here.
      </p>
    </>
  );
}

function Turn({
  turn,
  agent,
  actor,
  lead,
}: {
  turn: TurnView;
  agent: AgentController;
  actor(id: string | null | undefined): ActorView;
  /** An external agent's session header, which takes the place of the turn's own. */
  lead: ReactNode;
}) {
  const external = turn.external;
  const first = actor(turn.agents[0]);
  return (
    <article className="va-turn" aria-label={turn.prompt ? `Turn: ${turn.prompt.text}` : "Steps"}>
      {turn.prompt ? (
        <div className="va-you">
          <p className="va-you__text">{turn.prompt.text}</p>
          {turn.prompt.chips.length ? (
            <p className="va-you__chips vc-mono">{turn.prompt.chips.join(" · ")}</p>
          ) : null}
        </div>
      ) : null}
      {lead}
      {external ? null : (
        <>
          <div className="va-header">
            {turn.agents.length > 1 ? (
              <>
                {turn.agents.map((a) => (
                  <ActorMarker key={a} actor={actor(a)} size={20} outlined />
                ))}
                <span className="va-header__name">{turn.agents.length} agents</span>
              </>
            ) : (
              <>
                <ActorMarker actor={first} size={20} outlined />
                <span className="va-header__name">{first.name}</span>
              </>
            )}
            <span className="va-header__state">{turn.label}</span>
          </div>
          {turn.model ? <p className="va-model vc-mono">{turn.model}</p> : null}
        </>
      )}
      <TurnItems items={turn.items} agent={agent} turn={turn.id} />
      {turn.state === "stopped" && !turn.undone ? (
        <div className="va-card">
          <p className="va-card__title">{turn.label}</p>
          <p className="va-prose">Everything already built is kept. Retry or undo the turn.</p>
        </div>
      ) : null}
      {turn.footer ? <Footer turn={turn} agent={agent} /> : null}
    </article>
  );
}

function Footer({ turn, agent }: { turn: TurnView; agent: AgentController }) {
  return (
    <div className="va-footer">
      <p className="va-footer__usage vc-mono">
        {turn.footerLines.map((line) => (
          <span key={line}>{line}</span>
        ))}
      </p>
      {turn.undone ? (
        <p className="va-note">
          {`Undone · ${turn.undone.changed} ${turn.undone.changed === 1 ? "change" : "changes"} removed`}
          {turn.undone.kept ? ` · ${turn.undone.kept} kept because later edits build on them` : ""}
        </p>
      ) : null}
      <div className="va-actions">
        <button type="button" className="va-link" onClick={() => agent.reviewChanges(turn.id)}>
          Review changes
        </button>
        {turn.canUndo ? (
          <button
            type="button"
            className="va-button"
            data-primary={turn.state === "stopped" ? "" : undefined}
            data-undo={turn.key}
            aria-haspopup="dialog"
            aria-expanded={agent.state.confirmUndo === turn.key}
            onClick={() => agent.askUndo(turn.id)}
          >
            Undo this turn
          </button>
        ) : null}
        {turn.canRetry && turn.id ? (
          <button
            type="button"
            className="va-button"
            onClick={() => void agent.retry(turn.id as string)}
          >
            Retry
          </button>
        ) : null}
      </div>
    </div>
  );
}

/** The confirmation opens beside the panel, level with the turn's Undo button. */
function UndoConfirm({
  agent,
  actor,
  root,
}: {
  agent: AgentController;
  actor(id: string | null | undefined): ActorView;
  root: RefObject<HTMLDivElement | null>;
}) {
  const container = usePortalContainer();
  const prompt = agent.undoPrompt();
  const key = agent.state.confirmUndo;
  if (!prompt || !key) return null;
  const anchor = {
    getBoundingClientRect: () => {
      const tab = root.current;
      const button = tab?.querySelector(`[data-undo="${CSS.escape(key)}"]`);
      const p = (tab?.closest(".vc-panel") ?? tab)?.getBoundingClientRect();
      const b = button?.getBoundingClientRect();
      return new DOMRect(p?.right ?? 0, b?.top ?? 0, 0, b?.height ?? 0);
    },
  };
  return (
    <Popover.Root
      open
      onOpenChange={(open) => {
        if (!open) agent.cancelUndo();
      }}
    >
      <Popover.Portal container={container}>
        <Popover.Positioner
          anchor={anchor}
          side="right"
          align="center"
          sideOffset={32}
          collisionPadding={8}
        >
          <Popover.Popup className="va-confirm" initialFocus={false}>
            <Popover.Title className="va-confirm__title">Undo this turn?</Popover.Title>
            <Popover.Description className="va-prose">
              {copy.undoTurnConfirm
                .replace("{n}", String(prompt.changes))
                .replace("{agent}", actor(prompt.agent).name)}
            </Popover.Description>
            <div className="va-actions">
              <Popover.Close className="va-button">Cancel</Popover.Close>
              <button
                type="button"
                className="va-button"
                data-primary=""
                onClick={() => void agent.confirmUndo()}
              >
                Undo this turn
              </button>
            </div>
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** An external agent's conversation lives in its own window: the composer becomes this line (Appendix B). */
function ExternalLine({ client, onStop }: { client: string; onStop(): void }) {
  const [line, stop] = copy.externalSessionLine.replace("{client}", client).split(" · ");
  return (
    <div className="va-external">
      <p className="va-external__line">{line}</p>
      <button type="button" className="va-button" onClick={onStop}>
        {stop}
      </button>
    </div>
  );
}
