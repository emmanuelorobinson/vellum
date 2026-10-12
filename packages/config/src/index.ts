/** Brand and copy constants. Vellum is a working codename: rename here and in vellum-types::brand only. */
export const PACKAGE_NAME = "@vellum/config";

// Exact-copy strings from guide Appendix B (taken from DESIGN.md). scripts/copy-lint.mjs fails if a value
// drifts, if a key is added or removed, or if UI code repeats one of these strings. Import them; never retype them.
export const copy = {
  tagline: "Design where your code lives.",
  firstOpenHeading: "What are we designing?",
  homeComposer: "What do you want to design?",
  drawByHandHint: "Or draw by hand: press F for a frame",
  composerPlaceholder: "Describe what to design or change…",
  selectionPlaceholder: "Change the selected layers…",
  externalSessionLine: "Chat with {client} in its own window · Stop",
  keyStorage: "Stored in your system keychain. Never written to your files or sent to us.",
  notesForAgentsLine: "Agents read these before changing this frame",
  editingInsideLease: "You're editing here too",
  pausedBanner: "Agents paused · their changes are being rejected",
  historyFooter: "History since this file was opened · Full history is in Git.",
  historyEmpty: "Changes from you and your agents appear here",
  layersEmpty: "Ask the agent or press F to draw a frame",
  homeEmpty: "Open a folder to see its design files.",
  daemonOffline: "Daemon offline",
  composerWhenOffline: "Agents need the Vellum daemon · Restart",
  basicRenderer: "Basic renderer",
  newerFormat: "Made with a newer version of Vellum (format {n}) · Update to edit",
  writeRejected: "Frame leased by {agent}",
  keyFailed: "{provider} rejected the key · Update key",
  engineNotSignedIn: "{engine} isn't signed in · Open terminal to sign in",
  usageLimit: "{engine} reached its usage limit · resets at {time} · Switch engine",
  agentStuck: "Still working on this step",
  customFonts: "Stored with this file, so it travels with it",
  scaleNote: "Bound sizes become fixed values; instances get overrides.",
  undoTurnConfirm: "Removes {n} changes from {agent}'s turn. Your edits since then are kept.",
  costLabel: "Estimated cost at API rates",
  syncScopeA05: "Copies of files you choose to share, never your whole repo",
  scriptNodeEmpty: "Choose a script file or ask the agent to write one",
  reviewReminder: "Review agent changes in History.",
} as const;

// The Guard's reasons for rejecting an agent's write. vellum-core's `guard::reasons` holds the same strings under
// the same keys, and a Rust test reads this object to keep them equal. Only `leased` is in Appendix B.
export const guardReasons = {
  leased: copy.writeRejected,
  agentsPaused: "Agents paused",
  agentPaused: "{agent} is paused",
  stopped: "Stopped by you",
  stoppedHere: "Stopped by you on this frame",
  tooManyChanges: "{count} changes at once · the limit is {limit}",
  tooManyLayers: "{count} new layers at once · the limit is {limit}",
} as const;

export const brand = {
  name: "Vellum",
  tagline: copy.tagline,
} as const;
