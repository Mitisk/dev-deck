import { call } from "./client";
import type { ChecklistTemplate, Checklist } from "../types";

export const list = () => call<ChecklistTemplate[]>("templates_list");
export const save = (checklistId: number, name: string) => call<ChecklistTemplate>("template_save", { checklistId, name });
export const apply = (templateId: number, projectId: number) => call<Checklist>("template_apply", { templateId, projectId });
export const remove = (id: number) => call<void>("template_delete", { id });
