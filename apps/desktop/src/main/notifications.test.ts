// TEST DATA. No OS notification is shown by unit tests.
import { expect, it, vi } from "vitest";
import { TurnNotifications } from "./notifications";

it("does not replay old events, respects opt-out/focus and routes only a new background event", () => {
  const show = vi.fn();
  const notifications = new TurnNotifications(show);
  const event = { id: "session1", seq: 2, kind: "done" as const, path: "/project/design.vellum" };
  notifications.receive([event], true, false);
  expect(show).not.toHaveBeenCalled();
  notifications.receive([{ ...event, seq: 3, kind: "failed" }], true, false);
  expect(show).toHaveBeenCalledExactlyOnceWith("failed", event.path);
  notifications.receive([{ ...event, seq: 3, kind: "failed" }], true, false);
  notifications.receive([{ ...event, seq: 4, kind: "permission" }], false, false);
  notifications.receive([{ ...event, seq: 5 }], true, true);
  expect(show).toHaveBeenCalledOnce();
});
