import { call } from "./client";
import type { AttentionItem, AgendaItem } from "../types";

export const attention = () => call<AttentionItem[]>("dashboard_attention");

export const agenda = (today: string) => call<AgendaItem[]>("tasks_agenda", { today });
