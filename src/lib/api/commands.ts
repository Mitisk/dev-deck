import { call } from "./client";
import type { ProjectCommand } from "../types";

export type CommandInput = {
  label: string;
  command: string;
  workingDir?: string | null;
  runIn?: string | null;
  icon?: string | null;
};

export const list = (projectId: number) => call<ProjectCommand[]>("commands_list", { projectId });
export const create = (projectId: number, input: CommandInput) => call<ProjectCommand>("commands_create", { projectId, input });
export const update = (id: number, input: CommandInput) => call<ProjectCommand>("commands_update", { id, input });
export const remove = (id: number) => call<void>("commands_delete", { id });
export const run = (id: number) => call<void>("command_run", { id });
