import { createHash, createPublicKey } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { app } from "electron";
import updater from "electron-updater";
import { type SignedRelease, verifyRelease } from "./release-signature";
import type { UpdateDriver } from "./updates";

export async function updateDriver(): Promise<UpdateDriver | null> {
  if (!app.isPackaged) return null;
  try {
    const config = JSON.parse(
      await readFile(join(process.resourcesPath, "desktop-release.json"), "utf8"),
    ) as { feed: string; publicKey: string; publisher?: string };
    const url = new URL(config.feed);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.search ||
      url.hash ||
      createPublicKey(config.publicKey).asymmetricKeyType !== "ed25519" ||
      (process.platform === "win32" && !config.publisher)
    )
      return null;
    const { autoUpdater } = updater;
    autoUpdater.autoDownload = false;
    autoUpdater.autoInstallOnAppQuit = false;
    autoUpdater.allowDowngrade = false;
    autoUpdater.allowPrerelease = false;
    autoUpdater.disableWebInstaller = true;
    autoUpdater.logger = null;
    autoUpdater.on("error", () => {});
    autoUpdater.setFeedURL({ provider: "generic", url: url.href });
    let release: SignedRelease | null = null;
    let verified = false;
    return {
      async check() {
        release = null;
        verified = false;
        // No redirects, unbounded response or trust-key retrieval from the network.
        const response = await fetch(
          new URL(`release-${process.platform}-${process.arch}.json`, url),
          { redirect: "error", signal: AbortSignal.timeout(15000) },
        );
        if (!response.ok || !response.body) throw Error("update");
        const reader = response.body.getReader();
        let bytes = Buffer.alloc(0);
        try {
          for (;;) {
            const { value, done } = await reader.read();
            if (done) break;
            if (bytes.length + value.length > 70000) throw Error("update");
            bytes = Buffer.concat([bytes, value]);
          }
        } finally {
          await reader.cancel();
        }
        const signed = verifyRelease(
          JSON.parse(bytes.toString("utf8")),
          config.publicKey,
          process.platform,
          process.arch,
        );
        const result = await autoUpdater.checkForUpdates();
        if (!result) return null;
        const info = result.updateInfo;
        if (
          info.version !== signed.version ||
          ("packages" in info && info.packages) ||
          !info.files.length ||
          !info.files.every((file) =>
            signed.files.some(
              (f) => file.url === f.url && file.sha512 === f.sha512 && file.size === f.size,
            ),
          )
        )
          throw Error("signature");
        release = signed;
        return info.version;
      },
      async download() {
        verified = false;
        if (!release) throw Error("update");
        const paths = await autoUpdater.downloadUpdate();
        if (!paths.length) throw Error("update");
        for (const path of paths) {
          const size = (await stat(path)).size;
          const candidates = release.files.filter((f) => f.size === size);
          if (!candidates.length) throw Error("signature");
          const { createReadStream } = await import("node:fs");
          const hash = createHash("sha512");
          for await (const bytes of createReadStream(path)) hash.update(bytes);
          if (!candidates.some((f) => f.sha512 === hash.copy().digest("base64")))
            throw Error("signature");
        }
        verified = true;
      },
      install() {
        if (!verified) throw Error("signature");
        verified = false;
        autoUpdater.quitAndInstall(false, true);
      },
    };
  } catch {
    return null;
  }
}
