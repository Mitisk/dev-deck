import { call } from "./client";
import type { ProjectGroup } from "../types";

// Папки проектов в сайдбаре.
export const list = () => call<ProjectGroup[]>("groups_list");
export const create = (name: string) => call<ProjectGroup>("groups_create", { name });
export const rename = (id: number, name: string) => call<ProjectGroup>("groups_rename", { id, name });
export const remove = (id: number) => call<void>("groups_delete", { id });
export const reorder = (ids: number[]) => call<void>("groups_reorder", { ids });
