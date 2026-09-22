import { writable } from "svelte/store";
import type { Project } from "../types";
import * as projectsApi from "../api/projects";
import { loadGroups } from "./groups";

// Грузится из БД (см. loadProjects). Пусто до первой загрузки.
export const projects = writable<Project[]>([]);

// id выбранного проекта; null = дашборд.
export const activeProjectId = writable<number | null>(null);

// Признак, что первая загрузка завершена (для отрисовки empty-state, а не «пусто во время загрузки»).
export const projectsLoaded = writable(false);

// Перечитать список из БД.
export async function loadProjects(): Promise<void> {
  const [list] = await Promise.all([projectsApi.list(), loadGroups()]);
  projects.set(list);
  projectsLoaded.set(true);
  // обновить вотчер папок и меню трея под актуальный список (не критично при ошибке)
  import("../api/watch").then((w) => w.resync().catch(() => {}));
  import("../api/tray").then((t) => t.resync().catch(() => {}));
}
