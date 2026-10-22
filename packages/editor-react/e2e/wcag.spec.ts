// Chrome lane, TEST DATA. Automated AA checks complement the keyboard suite and manual screen-reader acceptance.
import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

for (const language of ["en", "ar"])
  for (const appearance of ["light", "dark", "hc-light", "hc-dark"])
    for (const surface of ["layers", "agent", "settings"]) {
      test(`WCAG AA ${language} ${appearance} ${surface}`, async ({ page }, info) => {
        await page.goto(
          `/?language=${language}&appearance=${appearance}&shell=1&tab=${surface === "settings" ? "agent" : surface}&${surface === "settings" ? "settings=open" : "agent=stream"}`,
        );
        await page.locator(".vc-workspace").waitFor();
        if (surface !== "settings") await page.evaluate(() => window.__vellum.agentReady());
        await page.evaluate(() => document.fonts.ready);
        const results = await new AxeBuilder({ page })
          .include(".vc-workspace")
          .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
          .analyze();
        await info.attach("axe-results", {
          body: JSON.stringify(results, null, 2),
          contentType: "application/json",
        });
        expect(
          results.violations.map(({ id, nodes }) => ({
            id,
            nodes: nodes.map(({ target, failureSummary }) => ({ target, failureSummary })),
          })),
        ).toEqual([]);
      });
    }
