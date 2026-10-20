// TEST DATA. W02 states and W03 use the product components, never deployed.

import { ShareDialogBody } from "@vellum/editor-react/share";
import type { ShareState } from "@vellum/host-api";
import { createRoot } from "react-dom/client";
import { AccountPlanBody } from "../../../account/app/AccountPlan";
import "../../../account/app/auth.css";

const q = new URLSearchParams(location.search);
document.documentElement.dataset.appearance = q.get("theme") ?? "light";
const state = q.get("state") ?? "not-shared";
const shared = {
  id: "test_00000000000000000000000000000001",
  url: "https://share.vellum.design/share/test_00000000000000000000000000000001",
  snapshot: { at: 1791729120000, incomplete: false },
};
const states: Record<string, ShareState> = {
  "not-shared": { state: "not-shared" },
  checking: { state: "checking", missing: ["images/hero.png", "fonts/JuniperSans.woff2"] },
  uploading: { state: "uploading", progress: 0.62 },
  shared: { state: "shared", ...shared },
  changed: { state: "changed", ...shared },
  failed: { state: "failed" },
};
const data = {
  user: { name: "Sam", email: "sam@example.com" },
  entitlements: ["share_links"],
  sessions: [
    { id: "desktop", expiresAt: 1791988320000, current: true },
    { id: "cli", expiresAt: 1791988320000, current: false },
  ],
  shares: [
    {
      id: "test_00000000000000000000000000000001",
      revoked: false,
      snapshot: { name: "juniper.vellum", at: 1791729120000, incomplete: false },
    },
    {
      id: "test_00000000000000000000000000000002",
      revoked: false,
      snapshot: { name: "components.vellum", at: 1791729120000, incomplete: false },
    },
  ],
};
const root = document.getElementById("root");
if (!root) throw Error("Missing gallery root");
createRoot(root).render(
  state === "account" ? (
    <AccountPlanBody data={data} busy={false} notice="" action={() => {}} />
  ) : (
    <dialog open className="vs-dialog">
      <ShareDialogBody
        name="juniper.vellum"
        state={states[state] ?? { state: "not-shared" }}
        act={() => {}}
        close={() => {}}
      />
    </dialog>
  ),
);
