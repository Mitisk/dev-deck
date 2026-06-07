import { call } from "./client";
import type { AttentionItem } from "../types";

export const attention = () => call<AttentionItem[]>("dashboard_attention");
