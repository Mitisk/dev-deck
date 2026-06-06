import { call } from "./client";
import type { Checklist, ChecklistItem } from "../types";

export const list = (projectId: number) => call<Checklist[]>("checklists_list", { projectId });
export const create = (projectId: number, title: string) => call<Checklist>("checklists_create", { projectId, title });
export const update = (id: number, title: string) => call<void>("checklists_update", { id, title });
export const remove = (id: number) => call<void>("checklists_delete", { id });
export const addItem = (checklistId: number, text: string) =>
  call<ChecklistItem>("checklist_items_add", { checklistId, text });
export const updateItem = (id: number, text: string) => call<void>("checklist_items_update", { id, text });
export const toggleItem = (id: number, isDone: boolean) => call<void>("checklist_items_toggle", { id, isDone });
export const removeItem = (id: number) => call<void>("checklist_items_delete", { id });
