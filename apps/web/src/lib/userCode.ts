/**
 * The codes TVs show to sign in (OAuth device authorization, RFC 8628
 * §6.1): 8 letters out of 20 consonants, so no words and nothing that looks
 * alike, shown as "BCDF-GHJK". Case, spaces and the dash don't matter.
 */

/** The letters codes are made of. */
export const CODE_LETTERS = "BCDFGHJKLMNPQRSTVWXZ";

/** How many letters a code has. */
export const CODE_LENGTH = 8;

/** The letters of a code as typed, in capitals: spaces, dashes and other signs are left out, and so is a 9th letter. */
export function codeLetters(input: string): string {
  return input
    .replace(/[^a-z]/gi, "")
    .toUpperCase()
    .slice(0, CODE_LENGTH);
}

/** Letters as a TV shows them, as far as they go: "BCDF", "BCDF-G", "BCDF-GHJK". */
export function showCode(letters: string): string {
  return letters.length > 4 ? `${letters.slice(0, 4)}-${letters.slice(4)}` : letters;
}

/** Whether there's a letter no code has: A, E, I, O, U or Y. */
export function hasStrayLetter(letters: string): boolean {
  return [...letters].some((letter) => !CODE_LETTERS.includes(letter));
}

/** Whether `letters` are a whole code. */
export function isWholeCode(letters: string): boolean {
  return letters.length === CODE_LENGTH && !hasStrayLetter(letters);
}
