// TEST DATA. No network or OS installer is used by these policy tests.
import { expect, it, vi } from "vitest";
import { Updates } from "./updates";

it("retains the installed build and refuses install after a bad signature, including retry", async () => {
  const install = vi.fn();
  const driver = {
    check: vi.fn(async () => "1.1.0"),
    download: vi.fn(async () => {
      throw Error("signature");
    }),
    install,
  };
  const updates = new Updates(driver, "1.0.0");
  await updates.check();
  await updates.download();
  expect(updates.state).toEqual({ kind: "failed" });
  expect(updates.install()).toBe(false);
  await updates.check();
  expect(updates.install()).toBe(false);
  expect(install).not.toHaveBeenCalled();
});
it("requires an explicit verified download and rejects downgrades", async () => {
  const driver = {
    check: vi.fn(async () => "1.1.0"),
    download: vi.fn(async () => {}),
    install: vi.fn(),
  };
  const updates = new Updates(driver, "1.0.0");
  expect(updates.install()).toBe(false);
  await updates.check();
  expect(driver.download).not.toHaveBeenCalled();
  await updates.download();
  expect(updates.install()).toBe(true);
  driver.check.mockResolvedValue("0.9.0");
  await updates.check();
  expect(updates.state).toEqual({ kind: "current" });
});
