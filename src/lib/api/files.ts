import { call } from "./client";
import type { FileShortcut } from "../types";

export const list = (projectId: number) => call<FileShortcut[]>("files_list", { projectId });
export const create = (projectId: number, label: string, path: string) =>
  call<FileShortcut>("files_create", { projectId, label, path });
export const update = (id: number, label: string, path: string) =>
  call<FileShortcut>("files_update", { id, label, path });
export const remove = (id: number) => call<void>("files_delete", { id });
