import { call } from "./client";
import type { Credential, CredType } from "../types";

export type CredInput = {
  label: string;
  type: CredType;
  username?: string | null;
  url?: string | null;
  notes?: string | null;
  secret?: string | null; // undefined/null = не менять (при update)
};

export const list = (projectId: number) => call<Credential[]>("creds_list", { projectId });
export const getSecret = (id: number) => call<string>("creds_get_secret", { id });
export const create = (projectId: number, input: CredInput) => call<Credential>("creds_create", { projectId, input });
export const update = (id: number, input: CredInput) => call<Credential>("creds_update", { id, input });
export const remove = (id: number) => call<void>("creds_delete", { id });
export const reorder = (projectId: number, ids: number[]) =>
  call<void>("creds_reorder", { projectId, ids });
