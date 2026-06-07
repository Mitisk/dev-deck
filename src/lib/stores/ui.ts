import { writable } from "svelte/store";

export type Tab = "overview" | "tasks" | "checklists" | "creds" | "notes" | "settings";

// Активная вкладка карточки проекта.
export const activeTab = writable<Tab>("overview");

// Открыта ли модалка создания проекта.
export const showNewProject = writable(false);

export const showPalette = writable(false);
