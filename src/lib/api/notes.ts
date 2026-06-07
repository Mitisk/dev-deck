import { call } from "./client";
import type { Note } from "../types";

export const list = (projectId: number) => call<Note[]>("notes_list", { projectId });
export const create = (projectId: number) => call<Note>("notes_create", { projectId });
export const update = (id: number, title: string, contentMd: string) =>
  call<void>("notes_update", { id, title, contentMd });
export const remove = (id: number) => call<void>("notes_delete", { id });
