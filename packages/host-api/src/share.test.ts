// TEST DATA. A host may accept only the snapshot protocol, and return public state.
import { expect, it } from "vitest";
import { shareRequest, shareState } from "./share";

it("refuses arbitrary paths, upload data and credential-bearing URLs", () => {
  expect(shareRequest({ action: "create", document: "/private" })).toBe(false);
  expect(shareRequest({ action: "create", shareAnyway: true })).toBe(true);
  expect(() =>
    shareState({ state: "shared", url: `https://token@host/share/${"a".repeat(32)}` }),
  ).toThrow();
  expect(
    shareState({ state: "failed", message: "private detail", token: "private detail" }),
  ).toEqual({
    state: "failed",
    message: "Upload failed. Retry to resume, or check your account and connection.",
  });
});
