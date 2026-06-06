import { call } from "./client";
import type { GitStatus } from "../types";

// null = путь пуст или не git-репозиторий.
export const status = (repoPath: string) => call<GitStatus | null>("git_status", { repoPath });
