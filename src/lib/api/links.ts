import { call } from "./client";
import type { Link } from "../types";

export const list = (projectId: number) => call<Link[]>("links_list", { projectId });
export const create = (projectId: number, label: string, url: string, icon: string | null = "globe") =>
  call<Link>("links_create", { projectId, label, url, icon });
export const update = (id: number, label: string, url: string, icon: string | null) =>
  call<Link>("links_update", { id, label, url, icon });
export const remove = (id: number) => call<void>("links_delete", { id });
