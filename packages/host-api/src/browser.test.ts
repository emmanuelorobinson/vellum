import { describe, expect, it } from "vitest";
import { browserRequest } from "./browser";

describe("BrowserPort input", () => {
  it("supports localhost and bounds a phone device", () => {
    expect(browserRequest({ url: "http://localhost:3000/", width: 390, height: 844 }).width).toBe(
      390,
    );
  });
  it("refuses credentials, non-web schemes, oversized devices and arbitrary origins", () => {
    for (const url of [
      "file:///etc/passwd",
      "javascript:alert(1)",
      "https://name:pass@example.com/",
    ])
      expect(() => browserRequest({ url, width: 390, height: 844 })).toThrow();
    expect(() => browserRequest({ url: "http://localhost/", width: 0, height: 844 })).toThrow();
    expect(() =>
      browserRequest({
        url: "http://localhost/",
        width: 390,
        height: 844,
        origins: ["https://cdn.invalid/path"],
      }),
    ).toThrow();
  });
});
