import { call } from "./client";
import type { Label } from "../types";

export const list = (projectId: number) => call<Label[]>("labels_list", { projectId });
export const create = (projectId: number, name: string, color: string | null) => call<Label>("label_create", { projectId, name, color });
export const update = (id: number, name: string, color: string | null) => call<Label>("label_update", { id, name, color });
export const remove = (id: number) => call<void>("label_delete", { id });
export const setTaskLabels = (taskId: number, labelIds: number[]) => call<void>("task_set_labels", { taskId, labelIds });
