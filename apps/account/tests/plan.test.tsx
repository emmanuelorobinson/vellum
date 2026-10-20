// TEST DATA. Static account projection, no vendor/network.
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { AccountPlanBody } from "../app/AccountPlan";

it("describes local and Pro features and includes session and shared-link revocation", () => {
  const html = renderToStaticMarkup(
    <AccountPlanBody
      data={{
        user: { name: "Fixture", email: "fixture@example.test" },
        entitlements: ["share_links"],
        sessions: [{ id: "session", current: true, expiresAt: 123 }],
        shares: [
          {
            id: "fixture",
            revoked: false,
            snapshot: { name: "Fixture file", at: 123, incomplete: true },
          },
        ],
      }}
      busy={false}
      notice=""
      action={() => {}}
    />,
  );
  expect(html).toContain("Manage plan");
  expect(html).toContain("Revoke");
  expect(html).toContain("Delete share link");
  expect(html).toContain("Incomplete");
});
