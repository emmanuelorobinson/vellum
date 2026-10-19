/** Public account state only. Native hosts retain all process and credential access. */
export type AccountState =
  | { kind: "idle" | "busy" | "failed" | "signed-in" }
  | { kind: "waiting"; userCode: string; verificationUrl: string }
  | { kind: "signed-out"; remoteRevoked?: boolean };
