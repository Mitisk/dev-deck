// Статусы проекта (как в БД).
export type ProjectStatus = "active" | "paused" | "done" | "archived";

// Запись проекта (camelCase, совпадает с сериализацией Rust).
export type Project = {
  id: number;
  name: string;
  description: string | null;
  status: ProjectStatus;
  color: string | null;
  icon: string | null; // emoji
  path: string | null;
  repoPath: string | null;
  pinned: boolean;
  sortOrder: number;
  tags: string[];
  createdAt: string;
  updatedAt: string;
};

export type User = { name: string; handle: string; initials: string };

export type GitStatus = {
  branch: string | null;
  ahead: number;
  behind: number;
  dirty: number;
  staged: number;
  untracked: number;
  lastHash: string | null;
  lastMessage: string | null;
  lastTimestamp: number | null;
};

export type GitOpResult = { ok: boolean; output: string };

export type TaskStatus = "todo" | "doing" | "done";
export type Task = {
  id: number;
  projectId: number;
  title: string;
  description: string | null;
  status: TaskStatus;
  priority: number; // 0 | 1 | 2
  dueDate: string | null;
  sortOrder: number;
  createdAt: string;
  completedAt: string | null;
};
