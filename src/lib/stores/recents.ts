import { writable } from "svelte/store";
import { browser } from "$app/environment";
import { activeProjectId } from "./projects";

const KEY = "devdeck-recents";
const MAX = 6;

function initial(): number[] {
  if (!browser) return [];
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as number[]) : [];
  } catch {
    return [];
  }
}

export const recents = writable<number[]>(initial());

recents.subscribe((v) => {
  if (browser) localStorage.setItem(KEY, JSON.stringify(v));
});

// При открытии проекта — поднять его в начало списка недавних.
activeProjectId.subscribe((id) => {
  if (id == null) return;
  recents.update((list) => [id, ...list.filter((x) => x !== id)].slice(0, MAX));
});
