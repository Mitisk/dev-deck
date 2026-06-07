import { call } from "./client";
import type { Task, TaskStatus } from "../types";

export type TaskInput = {
  title: string;
  description?: string | null;
  status?: TaskStatus | null;
  priority?: number | null;
  dueDate?: string | null;
};

export const list = (projectId: number) => call<Task[]>("tasks_list", { projectId });
export const create = (projectId: number, input: TaskInput) => call<Task>("tasks_create", { projectId, input });
export const update = (id: number, input: TaskInput) => call<Task>("tasks_update", { id, input });
export const move = (id: number, status: TaskStatus, sortOrder: number) =>
  call<void>("tasks_move", { id, status, sortOrder });
export const reorder = (projectId: number, status: TaskStatus, ids: number[]) =>
  call<void>("tasks_reorder", { projectId, status, ids });
export const deleteCompleted = (projectId: number) => call<number>("tasks_delete_completed", { projectId });
export const remove = (id: number) => call<void>("tasks_delete", { id });
