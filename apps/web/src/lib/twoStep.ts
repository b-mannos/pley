import { isApiError } from "@/api/client";
import { saveFile } from "./download";

/** The server's answer to a sign-in that needs its second step, or got a wrong code. */
export function isTwoStepError(err: unknown): boolean {
  return isApiError(err, 401) && err.code === "two_step";
}

/** What a typed code is: one from the authenticator app (6 digits), a recovery code, or neither yet. */
export function codeKind(input: string): "app" | "recovery" | null {
  const compact = input.replace(/[\s-]/g, "");
  if (/^\d{6}$/.test(compact)) return "app";
  if (/^[0-9a-z]{16}$/i.test(compact)) return "recovery";
  return null;
}

/** An authenticator secret in groups of four, as people type it: `"JBSW Y3DP EHPK 3PXP"`. */
export function groupSecret(secret: string): string {
  return secret.replace(/(.{4})(?=.)/g, "$1 ");
}

/** The recovery codes as a text file. */
export function recoveryCodesText(codes: string[], username: string, server?: string): string {
  return [
    `Pley recovery codes for ${username}${server ? ` on ${server}` : ""}`,
    "Each code signs in once, instead of a code from your authenticator app.",
    "",
    ...codes,
    "",
  ].join("\n");
}

/** Saves `text` as a file called `name`. */
export function downloadText(name: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
  saveFile(url, name);
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}
