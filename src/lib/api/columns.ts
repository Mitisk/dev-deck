import { call } from "./client";
import type { TaskColumn } from "../types";

export const list = (projectId: number) => call<TaskColumn[]>("columns_list", { projectId });
export const create = (projectId: number, name: string, isDone: boolean) => call<TaskColumn>("column_create", { projectId, name, isDone });
export const update = (id: number, name: string, isDone: boolean) => call<TaskColumn>("column_update", { id, name, isDone });
export const remove = (id: number) => call<void>("column_delete", { id });
