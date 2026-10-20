// TEST DATA. Static dialog states require no daemon or account.
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { ShareDialogBody } from "./ShareDialog";

it("renders missing dependencies, an explicit incomplete choice and background upload progress", () => {
  const html = renderToStaticMarkup(
    <ShareDialogBody
      name="Fixture.vellum"
      state={{ state: "checking", missing: ["images/missing.png"] }}
      act={() => {}}
      close={() => {}}
    />,
  );
  expect(html).toContain("images/missing.png");
  expect(html).toContain("Share anyway");
  expect(html).toContain("Restore and try again");
  const uploading = renderToStaticMarkup(
    <ShareDialogBody
      name="Fixture.vellum"
      state={{ state: "uploading", progress: 0.62 }}
      act={() => {}}
      close={() => {}}
    />,
  );
  expect(uploading).toContain("62%");
  expect(uploading).toContain("Uploading continues");
});
