import { type AccountRunner, accountRunner, NativeAccount } from "@vellum/host-api/native-account";
import * as vscode from "vscode";

/** Account commands run only after a person's explicit request; no webview receives credentials. */
export function accountCommands(binary: string, run: AccountRunner = accountRunner(binary)) {
  let report: ((message: string) => void) | undefined;
  let offered = false;
  const account = new NativeAccount(run, () => {
    const state = account.state;
    if (state.kind !== "waiting") return;
    report?.(`Code: ${state.userCode} · Approve in your browser`);
    if (offered) return;
    offered = true;
    void vscode.window
      .showInformationMessage(
        `Vellum: compare code ${state.userCode} in your browser.`,
        "Open approval page",
      )
      .then(
        async (choice) => {
          const current = account.state;
          if (
            choice !== "Open approval page" ||
            current.kind !== "waiting" ||
            current.verificationUrl !== state.verificationUrl
          )
            return;
          try {
            if (!(await vscode.env.openExternal(vscode.Uri.parse(state.verificationUrl))))
              throw Error("unavailable");
          } catch {
            void vscode.window.showErrorMessage("Vellum: the approval page could not be opened.");
          }
        },
        () => {},
      );
  });
  const message = () => {
    const state = account.state;
    return state.kind === "signed-in"
      ? "Vellum: signed in."
      : state.kind === "signed-out"
        ? state.remoteRevoked === false
          ? "Vellum: signed out on this computer; remote session revocation unconfirmed."
          : "Vellum: signed out."
        : state.kind === "idle"
          ? "Vellum: sign-in cancelled."
          : "Vellum: account request unavailable. Try again when your account service is available.";
  };
  return [
    { dispose: () => account.dispose() },
    vscode.commands.registerCommand("vellum.signIn", async () => {
      if (["busy", "waiting"].includes(account.state.kind)) return;
      offered = false;
      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: "Vellum: sign in",
          cancellable: true,
        },
        async (progress, token) => {
          report = (message) => progress.report({ message });
          const subscription = token.onCancellationRequested(() => void account.cancel());
          try {
            if (!token.isCancellationRequested) await account.login();
          } finally {
            report = undefined;
            subscription.dispose();
          }
        },
      );
      void vscode.window.showInformationMessage(message());
    }),
    vscode.commands.registerCommand("vellum.accountStatus", async () => {
      if (["busy", "waiting"].includes(account.state.kind)) return;
      await account.refresh();
      void vscode.window.showInformationMessage(message());
    }),
    vscode.commands.registerCommand("vellum.signOut", async () => {
      await account.logout();
      void vscode.window.showInformationMessage(message());
    }),
  ];
}
