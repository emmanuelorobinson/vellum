// @chrome fixture startup probe. Fails rather than hiding a missing API/viewer process.
for (const url of ["http://127.0.0.1:3112/v1/entitlements", "http://127.0.0.1:3111/"]) {
  let ready = false;
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(url, { signal: AbortSignal.timeout(1000) })).ok) {
        ready = true;
        break;
      }
    } catch {}
    await new Promise((r) => setTimeout(r, 200));
  }
  if (!ready) throw Error("Share fixture service did not start");
}
