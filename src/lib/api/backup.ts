import { call } from "./client";
import type { BackupInfo } from "../types";
export const backupNow = () => call<string>("backup_now");
export const backupsList = () => call<BackupInfo[]>("backups_list");
