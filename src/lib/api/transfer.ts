import { call } from "./client";
import type { ImportSummary } from "../types";
export const exportJson = (includeSecrets: boolean) => call<string>("export_json", { includeSecrets });
export const exportToFile = (includeSecrets: boolean) => call<string>("export_to_file", { includeSecrets });
export const importJson = (json: string) => call<ImportSummary>("import_json", { json });
export const exportToPath = (path: string, includeSecrets: boolean) => call<string>("export_to_path", { path, includeSecrets });
export const importFromPath = (path: string) => call<ImportSummary>("import_from_path", { path });
