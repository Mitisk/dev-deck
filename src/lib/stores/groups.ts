import { writable } from "svelte/store";
import { browser } from "$app/environment";
import type { ProjectGroup } from "../types";
import * as groupsApi from "../api/groups";

// Папки проектов (сайдбар). Грузятся вместе с проектами (см. loadProjects).
export const groups = writable<ProjectGroup[]>([]);

export async function loadGroups(): Promise<void> {
  groups.set(await groupsApi.list());
}

// Свёрнутые папки — чисто UI-состояние, живёт в localStorage.
const KEY = "devdeck-collapsed-groups";

function initialCollapsed(): number[] {
  if (!browser) return [];
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as number[]) : [];
  } catch {
    return [];
  }
}

export const collapsedGroups = writable<number[]>(initialCollapsed());

collapsedGroups.subscribe((v) => {
  if (!browser) return;
  try {
    localStorage.setItem(KEY, JSON.stringify(v));
  } catch {
    /* приватный режим и т.п. — не критично */
  }
});

export function toggleCollapsed(id: number): void {
  collapsedGroups.update((list) => (list.includes(id) ? list.filter((x) => x !== id) : [...list, id]));
}
