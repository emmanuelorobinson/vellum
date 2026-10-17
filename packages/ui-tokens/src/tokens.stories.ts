// Storybook story for the token page (Component Story Format 3, html renderer). Storybook itself is set up
// with the visual harness in Prompt 45; until then scripts/token-page-compare.mjs renders the same
// function in headless Chrome and compares it with the design's Tokens sheet.
import "./generated/tokens.css";
import { renderTokenPage, TOKEN_PAGE_WIDTH } from "./token-page";

export default {
  title: "Foundations/Tokens",
  parameters: { layout: "fullscreen", viewport: { width: TOKEN_PAGE_WIDTH } },
};

export const Tokens = { render: () => renderTokenPage() };
