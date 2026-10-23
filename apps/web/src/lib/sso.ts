/** What a single sign-on that came back says (`?sso=<code>` on the sign-in and account pages). */
export interface SsoNotice {
  tone: "success" | "error";
  text: string;
}

/**
 * The notice for `code`, naming the `provider`; `null` without a code. The
 * server keeps the details in its log: these say what to do next.
 */
export function ssoNotice(code: string | null, provider: string): SsoNotice | null {
  if (!code) return null;
  const error = (text: string): SsoNotice => ({ tone: "error", text });
  switch (code) {
    case "linked":
      return { tone: "success", text: `Your ${provider} account is linked: you can sign in with it now.` };
    case "off":
      return error("Single sign-on is off on this server.");
    case "expired":
      return error("That sign-in took too long, or was started in another browser. Try again.");
    case "denied":
      return error(`The sign-in was cancelled at ${provider}.`);
    case "provider":
      return error(`${provider} didn't complete the sign-in. The server's log says why.`);
    case "invalid":
      return error(`${provider}'s answer couldn't be verified. The server's log says why.`);
    case "unknown":
      return error(
        `No account here is linked to that ${provider} account. Sign in with your password, then link it on your account page.`,
      );
    case "disabled":
      return error("This account is disabled.");
    case "group":
      return error(`That ${provider} account isn't allowed to use this server.`);
    case "signin":
      return error("Sign in again here to link your account.");
    case "taken":
      return error(`That ${provider} account is already linked to another account here.`);
    default:
      return error("Something went wrong. Try again.");
  }
}

/** Where to send the browser to sign in with the provider, coming back to `next`. */
export function ssoStartUrl(start: string, next: string): string {
  return `${start}?next=${encodeURIComponent(next)}`;
}

/** Whether this page is at pley's public address, where linking happens. */
export function atPublicAddress(publicUrl: string | undefined, origin: string = window.location.origin): boolean {
  if (!publicUrl) return true;
  try {
    return new URL(publicUrl).origin === origin;
  } catch {
    return true;
  }
}
