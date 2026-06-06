import { writable } from "svelte/store";
import type { Project } from "../types";
import { MOCK_PROJECTS } from "../mock";

// На Phase 0 источник — моки. В срезе «Проекты CRUD» заменится загрузкой из БД.
export const projects = writable<Project[]>(MOCK_PROJECTS);

// id выбранного проекта; null = дашборд.
export const activeProjectId = writable<string | null>(null);
