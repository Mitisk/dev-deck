import { writable, derived } from "svelte/store";
import type { UserProfile } from "../types";
import * as userApi from "../api/user";

export const user = writable<UserProfile>({ name: "", handle: null });

export const displayName = derived(user, ($u) => $u.name.trim() || "Пользователь");
export const initials = derived(user, ($u) => {
  const parts = $u.name.trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return "·";
  const a = parts[0][0] ?? "";
  const b = parts.length > 1 ? (parts[1][0] ?? "") : "";
  return (a + b).toUpperCase();
});

export async function loadUser(): Promise<void> {
  try { user.set(await userApi.get()); } catch { /* нет бэка — оставить дефолт */ }
}
