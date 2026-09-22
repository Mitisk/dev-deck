import { call } from "./client";
import type { Project, ProjectStatus } from "../types";

// Вход на создание/обновление (camelCase — сериализуется в ProjectInput на Rust).
export type ProjectInput = {
  name: string;
  description?: string | null;
  status?: ProjectStatus | null;
  color?: string | null;
  icon?: string | null;
  path?: string | null;
  repoPath?: string | null;
  healthUrl?: string | null;
  tags: string[];
};

export const list = () => call<Project[]>("projects_list");
export const get = (id: number) => call<Project>("projects_get", { id });
export const create = (input: ProjectInput) => call<Project>("projects_create", { input });
export const update = (id: number, input: ProjectInput) => call<Project>("projects_update", { id, input });
export const remove = (id: number) => call<void>("projects_delete", { id });
export const archive = (id: number) => call<void>("projects_archive", { id });
export const setPinned = (id: number, pinned: boolean) => call<void>("project_set_pinned", { id, pinned });
export const setSort = (id: number, sortOrder: number) => call<void>("project_set_sort", { id, sortOrder });
// Переместить проект в папку (null = корень).
export const setGroup = (id: number, groupId: number | null) => call<void>("project_set_group", { id, groupId });
// Выставить sort_order по порядку id (перестановка внутри одной секции сайдбара).
export const reorder = (ids: number[]) => call<void>("projects_reorder", { ids });
export const importIcon = (srcPath: string) => call<string>("project_import_icon", { srcPath });
