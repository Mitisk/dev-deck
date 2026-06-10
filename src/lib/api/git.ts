import { call } from "./client";
import type { GitStatus, GitOpResult, GitChanges } from "../types";

// null = путь пуст или не git-репозиторий.
export const status = (repoPath: string) => call<GitStatus | null>("git_status", { repoPath });

// null = путь пуст или не git-репозиторий.
export const changes = (repoPath: string) => call<GitChanges | null>("git_changes", { repoPath });

export const fetch = (repoPath: string) => call<GitOpResult>("git_fetch", { repoPath });
export const pull = (repoPath: string) => call<GitOpResult>("git_pull", { repoPath });
export const push = (repoPath: string) => call<GitOpResult>("git_push", { repoPath });
export const commitAll = (repoPath: string, message: string) =>
  call<GitOpResult>("git_commit_all", { repoPath, message });
