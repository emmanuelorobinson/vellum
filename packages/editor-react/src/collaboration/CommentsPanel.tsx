import type { CollaborationController } from "@vellum/editor-core";
import { useEffect, useRef } from "react";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";

export function CommentsPanel({ controller: c }: { controller: CollaborationController }) {
  useStore(c.subscribe);
  const composer = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (c.state.panel && !c.state.selected) composer.current?.focus();
  }, [c.state.panel, c.state.selected]);
  if (!c.state.panel) return null;
  const all = c.threads();
  const list = all.filter(([, t]) => t.resolved === (c.state.filter === "resolved"));
  const selected = all.find(([id]) => id === c.state.selected);
  const thread = selected?.[1];
  const allowed = c.canComment();
  const anchor = c.state.selected ? c.anchor(c.state.selected) : null;
  const close = () => {
    c.close();
    document.querySelector<HTMLButtonElement>(".vm-comments-entry")?.focus();
  };
  const position = list.findIndex(([id]) => id === c.state.selected) + 1;
  return (
    <aside
      className="vm-comments"
      aria-label="Comments"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          close();
        }
      }}
    >
      <header>
        <h2>
          <Icon name="message-square" size={15} />
          Comments <small>{all.filter(([, t]) => !t.resolved).length} open</small>
        </h2>
        {c.state.grant && (
          <span className="vm-role-badge">
            <Icon name="user-round" size={12} />
            {c.state.grant.role.charAt(0).toUpperCase() + c.state.grant.role.slice(1)}
          </span>
        )}
        <button type="button" className="vm-close" aria-label="Close comments" onClick={close}>
          <Icon name="x" size={16} />
        </button>
      </header>
      <div className="vm-tabs" role="tablist" aria-label="Thread status">
        {(["open", "resolved"] as const).map((filter) => (
          <button
            type="button"
            role="tab"
            aria-selected={c.state.filter === filter}
            key={filter}
            onClick={() => c.filter(filter)}
          >
            {filter === "open" ? "Open" : "Resolved"} ·{" "}
            {all.filter(([, t]) => t.resolved === (filter === "resolved")).length}
          </button>
        ))}
      </div>
      <div className="vm-comments__body">
        <section className="vm-thread-list">
          <h3>{c.state.filter === "open" ? "Open threads" : "Resolved threads"}</h3>
          {list.length ? (
            <div role="listbox" aria-label="Comment threads">
              {list.map(([id, t], i) => {
                const first = Object.values(t.messages).sort((a, b) => a.atMs - b.atMs)[0];
                return (
                  <div
                    className="vm-thread-row"
                    role="option"
                    aria-selected={id === c.state.selected}
                    tabIndex={id === c.state.selected || (!c.state.selected && i === 0) ? 0 : -1}
                    key={id}
                    onClick={() => c.select(id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        c.select(id);
                      }
                      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                        e.preventDefault();
                        const next =
                          (i + (e.key === "ArrowDown" ? 1 : list.length - 1)) % list.length;
                        const item = list[next];
                        if (item) c.select(item[0]);
                        e.currentTarget.parentElement
                          ?.querySelectorAll<HTMLElement>('[role="option"]')
                          [next]?.focus();
                      }
                    }}
                  >
                    <span className="vm-pin-number">{i + 1}</span>
                    <span>
                      <span className="vm-thread-row__heading">
                        <strong>{c.actorName(t.author)}</strong>
                        <time>{c.since(t.createdMs)}</time>
                      </span>
                      <small>{first?.text}</small>
                    </span>
                  </div>
                );
              })}
            </div>
          ) : (
            <>
              <p>No comments yet</p>
              <button
                type="button"
                disabled={!allowed}
                onClick={() => c.newComment(null, { x: 0, y: 0 })}
              >
                Comment on selection
              </button>
            </>
          )}
        </section>
        <section className="vm-thread" aria-label={thread ? "Selected thread" : "New comment"}>
          {thread ? (
            <>
              <header className="vm-thread-heading">
                <span className="vm-pin-number">{position || 1}</span>
                <span>
                  <strong>{anchor?.label}</strong>
                  <small>{anchor?.path ?? thread.anchor ?? "Canvas"}</small>
                </span>
                <span className="vm-thread-state">{thread.resolved ? "✓ Resolved" : "○ Open"}</span>
              </header>
              {!anchor?.available && (
                <p className="vm-warning">
                  △ Original layer was deleted. The conversation is still readable. Anchor removed ·
                  last at {thread.point.x}, {thread.point.y}
                </p>
              )}
              {Object.entries(thread.messages)
                .sort(([a, x], [b, y]) => x.atMs - y.atMs || a.localeCompare(b))
                .map(([id, m]) => (
                  <article className="vm-reply" key={id}>
                    <span
                      className="vm-person"
                      role="img"
                      aria-label="Person"
                      data-outline={
                        JSON.stringify(m.author) !== JSON.stringify(thread.author) || undefined
                      }
                    >
                      {c.actorName(m.author).slice(0, 1)}
                    </span>
                    <div>
                      <div className="vm-reply__heading">
                        <strong>{c.actorName(m.author)}</strong>
                        <small>
                          {JSON.stringify(m.author) === JSON.stringify(thread.author)
                            ? "Author"
                            : c.actorRole(m.author)}
                        </small>
                        <time dateTime={new Date(m.atMs).toISOString()}>{c.since(m.atMs)}</time>
                      </div>
                      <p>{m.text}</p>
                    </div>
                  </article>
                ))}
              <div className="vm-resolve">
                <span>
                  Only {c.actorName(thread.author).split(" ")[0]} or an Editor can{" "}
                  {thread.resolved ? "reopen" : "resolve"}.
                </span>
                <button
                  type="button"
                  disabled={!c.canResolve(c.state.selected!) || c.state.posting}
                  onClick={() => void c.resolve(c.state.selected!, !thread.resolved)}
                >
                  <Icon name="check" size={12} />
                  {thread.resolved ? "Reopen" : "Resolve"}
                </button>
              </div>
            </>
          ) : (
            <h3>New comment</h3>
          )}
          {!thread?.resolved && (
            <div className="vm-composer">
              <div className="vm-composer__row">
                <span className="vm-person" role="img" aria-label="Person">
                  {c.state.grant ? c.actorName(c.state.grant.actor).slice(0, 1) : ""}
                </span>
                <textarea
                  ref={composer}
                  aria-label={thread ? "Reply to this thread" : "New comment"}
                  placeholder={thread ? "Reply to this thread…" : "Write a comment…"}
                  value={c.state.draft}
                  readOnly={!allowed || c.state.posting}
                  maxLength={8192}
                  rows={1}
                  onChange={(e) => c.draft(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                      e.preventDefault();
                      void c.post();
                    }
                  }}
                />
              </div>
              <div className="vm-composer__footer">
                <small>{c.state.posting ? "Posting…" : "⌘⏎ to post"}</small>
                <button
                  type="button"
                  className="vm-primary"
                  disabled={!allowed || !c.state.draft.trim() || c.state.posting}
                  onClick={() => void c.post()}
                >
                  {thread ? "Post reply" : "Post comment"}
                </button>
              </div>
            </div>
          )}
          {!allowed && (
            <p className="vm-warning" role="status">
              {c.state.grant?.role === "viewer"
                ? "Viewer · Commenting is off"
                : "Posting is unavailable"}
              {c.state.draft && (
                <>
                  . Your draft is saved but can’t be posted.{" "}
                  <button type="button" onClick={() => void c.copyDraft()}>
                    Copy draft
                  </button>
                </>
              )}
            </p>
          )}
          {c.state.error && (
            <p className="vm-error" role="alert">
              △ {c.state.error}
              <small>Your draft is still here. Check your connection and try again.</small>
              <button
                type="button"
                disabled={!allowed || c.state.posting}
                onClick={() => void c.post()}
              >
                Try again
              </button>
            </p>
          )}
        </section>
      </div>
      <footer>
        <Icon name="info" size={12} />
        Comments are separate from agent prompts.
      </footer>
    </aside>
  );
}
