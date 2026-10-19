import { copy } from "@vellum/config";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { AuthForm } from "../app/AuthForm";

it("keeps sign-in optional and uses the canonical sync scope", () => {
  const html = renderToStaticMarkup(<AuthForm />);
  expect(html).toContain(copy.syncScopeA05);
  expect(html).toContain("Continue with email");
  expect(html).toContain('type="email"');
});
it("requires explicit approval of a visible device code", () => {
  const html = renderToStaticMarkup(<AuthForm deviceCode="ABCD-2345" signedIn />);
  expect(html).toContain("ABCD-2345");
  expect(html).toContain("Approve");
  expect(html).toContain("Deny");
  expect(html).toContain("does not share your repo");
});
