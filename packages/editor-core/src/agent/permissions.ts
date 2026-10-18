/**
 * The Permission card (DESIGN.md "Permission card", C27): what an agent wants to do outside the canvas, in plain
 * words with the exact path or command. The broker in the daemon decides and records (ADR-006, Invariant 11); the
 * panel only asks the person and passes the answer on.
 */
import type { AgentId, TurnId } from "@vellum/types-ts";

/** As the session codec writes vellum-core's `Action`. */
export type PermissionAction =
  | { readFile: { path: string } }
  | { writeFile: { path: string } }
  | { runCommand: { program: string; args: readonly string[]; cwd: string } }
  | { network: { url: string } };

export type PermissionAnswer = "allow_once" | "allow_for_session" | "deny";

/** A question the broker is waiting on (vellum-core `Question`), with the id the answer goes back with. */
export interface PermissionQuestion {
  id: string;
  turn: TurnId | null;
  agent: AgentId | null;
  engine: string | null;
  action: PermissionAction;
  kind: string;
  /** The answers offered, in display order. */
  offered: readonly PermissionAnswer[];
}

const basename = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
/** The host of an absolute URL ("api.example.test"), else the URL as written. */
const host = (url: string) => /^[a-z][a-z0-9+.-]*:\/\/([^/?#@]*@)?([^/?#]+)/i.exec(url)?.[2] ?? url;

/** "Teal wants to read DESIGN.md", with the exact path or command below it. */
export function permissionCard(
  action: PermissionAction,
  agent: string,
): { sentence: string; target: string } {
  if ("readFile" in action)
    return {
      sentence: `${agent} wants to read ${basename(action.readFile.path)}`,
      target: action.readFile.path,
    };
  if ("writeFile" in action)
    return {
      sentence: `${agent} wants to write ${basename(action.writeFile.path)}`,
      target: action.writeFile.path,
    };
  if ("runCommand" in action) {
    const line = [action.runCommand.program, ...action.runCommand.args].join(" ");
    return { sentence: `${agent} wants to run ${line}`, target: line };
  }
  return {
    sentence: `${agent} wants to reach ${host(action.network.url)}`,
    target: action.network.url,
  };
}

/** A decision the person made, as the turn shows it afterwards ("Permission / Allowed", "Permission / Denied"). */
export interface DecidedCard {
  caption: "Allowed" | "Denied";
  message: string;
  target: string;
}

export function decidedCard(action: PermissionAction, answer: string): DecidedCard {
  const { target } = permissionCard(action, "");
  if (answer === "deny") return { caption: "Denied", message: "Permission denied by you", target };
  return {
    caption: "Allowed",
    message:
      answer === "allow_for_session"
        ? "Permission allowed for this session"
        : "Permission allowed once",
    target,
  };
}
