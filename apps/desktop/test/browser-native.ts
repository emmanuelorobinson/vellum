// TEST DATA. Electron lane: real local Chromium capture and retained live page; no display interaction.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { app, BrowserWindow } from "electron";
import { createElectronBrowser } from "../src/main/browser";

void app
  .whenReady()
  .then(async () => {
    console.log("Electron ready");
    const server = createServer((_req, res) => {
      res.setHeader("content-type", "text/html");
      res.end(
        '<!doctype html><!-- TEST DATA --><style>body{margin:0;font:24px sans-serif}#card{width:240px;height:160px;background:lightblue}</style><article id="card">Editable text <span id="count">0</span></article><script>setInterval(()=>{document.getElementById("count").textContent=String(Number(document.getElementById("count").textContent)+1)},100)</script>',
      );
    });
    await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
    const address = server.address();
    if (!address || typeof address === "string") throw Error("no listener");
    const origins: string[] = [];
    const browser = createElectronBrowser({
      authorize: async (origin) => {
        origins.push(origin);
      },
      create: (options) => new BrowserWindow(options),
    });
    try {
      const request = {
        url: `http://127.0.0.1:${address.port}/`,
        width: 375,
        height: 812,
        snapshot: true,
      };
      console.log("Electron capture started");
      const capture = await browser.capture(request);
      console.log("Electron capture complete");
      assert.equal(capture.width, 375);
      assert.equal(capture.height, 812);
      assert.ok(capture.dom?.nodes.some((n) => n.text?.includes("Editable text")));
      console.log("Electron element capture started");
      const element = await browser.capture({ ...request, selector: "#card" });
      assert.equal(element.width, 240);
      assert.equal(element.height, 160);
      console.log("Electron element capture complete");
      let previous = "";
      let frames = 0;
      let finish!: () => void;
      const changed = new Promise<void>((ok) => {
        finish = ok;
      });
      const watch = await browser.watch?.(
        request,
        (frame) => {
          frames++;
          if (previous && frame.png !== previous) finish();
          previous = frame.png;
        },
        () => {
          throw Error("live session failed");
        },
      );
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        await Promise.race([
          changed,
          new Promise<never>((_ok, no) => {
            timer = setTimeout(() => no(Error("live page did not change")), 10000);
          }),
        ]);
      } finally {
        clearTimeout(timer);
        watch?.dispose();
      }
      assert.ok(frames >= 2);
      assert.equal(origins.length, 3);
      assert.equal(BrowserWindow.getAllWindows().length, 0);
      console.log(
        "Electron BrowserPort: viewport, DOM, element crop, live updates and disposal passed",
      );
    } finally {
      server.closeAllConnections();
      await new Promise<void>((ok) => server.close(() => ok()));
      app.quit();
    }
  })
  .catch((error) => {
    console.error(error);
    app.exit(1);
  });
