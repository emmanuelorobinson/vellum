/**
 * The Permission card (DESIGN.md "Permission card", C27): the turn waits here until the person answers. The
 * answer goes to the broker; the card stays, its buttons disabled, until the daemon has recorded the decision.
 */
import type { ComposerController, DecidedCard, PermissionQuestion } from "@vellum/editor-core";
import { permissionCard } from "@vellum/editor-core";
import { Icon } from "../icons/Icon";

const ANSWERS = {
  allow_once: "Allow once",
  allow_for_session: "Allow for this session",
  deny: "Deny",
} as const;

export function PermissionCard({
  question,
  agentName,
  composer,
}: {
  question: PermissionQuestion;
  agentName: string;
  composer: ComposerController;
}) {
  const { sentence, target } = permissionCard(question.action, agentName);
  const busy = composer.answering(question.id);
  const [first, ...rest] = question.offered;
  const button = (answer: keyof typeof ANSWERS, primary = false) => (
    <button
      key={answer}
      type="button"
      className="va-button"
      data-primary={primary ? "" : undefined}
      disabled={busy}
      onClick={() => void composer.answer(question.id, answer)}
    >
      {ANSWERS[answer]}
    </button>
  );
  return (
    <section
      className="vk-permission"
      aria-label="Waiting for permission"
      data-busy={busy ? "" : undefined}
    >
      <p className="vk-permission__state">
        <Icon name="triangle-alert" size={14} />
        Waiting for permission
      </p>
      <p className="vk-permission__sentence">{sentence}</p>
      <p className="vk-permission__target vc-mono">{target}</p>
      {first ? button(first, true) : null}
      {rest.length ? <div className="va-actions">{rest.map((a) => button(a))}</div> : null}
    </section>
  );
}

/** A decision already made, in the turn where it happened. */
export function DecidedPermission({ card }: { card: DecidedCard }) {
  return (
    <section className="vk-permission" data-decided={card.caption} aria-label={card.caption}>
      <p className="vk-permission__caption">
        <Icon name={card.caption === "Allowed" ? "check" : "x"} size={14} />
        {card.caption}
      </p>
      <p className="vk-permission__sentence">{card.message}</p>
      <p className="vk-permission__target vc-mono">{card.target}</p>
    </section>
  );
}
