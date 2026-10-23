/** Shortest password the server accepts. */
export const MIN_PASSWORD_LENGTH = 8;

/**
 * What's wrong with a new password that can be told without asking the
 * server (which also refuses common passwords), or `null`.
 */
export function passwordProblem(password: string, username = ""): string | null {
  if ([...password].length < MIN_PASSWORD_LENGTH) return `Use at least ${MIN_PASSWORD_LENGTH} characters for the password.`;
  if (username.trim() && password.toLowerCase() === username.trim().toLowerCase()) return "The password can't be the username.";
  return null;
}
