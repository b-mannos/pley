import type { Id, Library, User } from "@/api/types";
import { pluralize } from "./format";

/** Age limits offered for users and invitations (see the server's rating table). */
export const AGE_OPTIONS: { value: number | null; label: string }[] = [
  { value: null, label: "No limit" },
  { value: 0, label: "Little kids (G, TV-Y)" },
  { value: 7, label: "Kids (TV-Y7)" },
  { value: 10, label: "Older kids (PG, TV-PG)" },
  { value: 13, label: "Teens (PG-13)" },
  { value: 14, label: "TV-14" },
  { value: 15, label: "15+" },
  { value: 17, label: "Adults (R, TV-MA)" },
];

/** "Up to 13", or null without a limit. */
export function ageBadge(maxAge: number | null | undefined): string | null {
  return maxAge == null ? null : `Up to ${maxAge}`;
}

export function ageOptionLabel(maxAge: number | null | undefined): string {
  return AGE_OPTIONS.find((o) => o.value === (maxAge ?? null))?.label ?? `Up to ${maxAge}`;
}

/** "All libraries", "Movies", "2 libraries". */
export function librariesLabel(
  access: Pick<User, "all_libraries" | "library_ids">,
  libraries: Library[] | undefined,
): string {
  if (access.all_libraries) return "All libraries";
  const ids = access.library_ids;
  if (ids.length === 0) return "No libraries";
  if (ids.length === 1) {
    const name = libraries?.find((l) => l.id === ids[0])?.name;
    if (name) return name;
  }
  return pluralize(ids.length, "library", "libraries");
}

/** Encode the age select's value (`""` = no limit). */
export function ageToSelect(age: number | null): string {
  return age == null ? "" : String(age);
}

export function selectToAge(value: string): number | null {
  return value === "" ? null : Number(value);
}

export function toggleId(ids: Id[], id: Id): Id[] {
  return ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
}
