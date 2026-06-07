import { writable, get } from "svelte/store";
import { browser } from "$app/environment";
import { activeProjectId } from "./projects";

// id проектов с изменениями в папке (не открытых после изменения).
export const changedProjects = writable<number[]>([]);

export function markChanged(id: number) {
  // не помечаем открытый проект
  if (get(activeProjectId) === id) return;
  changedProjects.update((list) => (list.includes(id) ? list : [...list, id]));
}

// При открытии проекта — снять пометку.
activeProjectId.subscribe((id) => {
  if (id == null) return;
  changedProjects.update((list) => list.filter((x) => x !== id));
});

// Подписка на событие бэкенда (один раз).
let started = false;
export async function startWatchEvents() {
  if (!browser || started) return;
  started = true;
  const { listen } = await import("@tauri-apps/api/event");
  await listen<number>("folder-changed", (e) => markChanged(e.payload));
}
