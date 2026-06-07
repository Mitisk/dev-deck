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

export type ChecklistItem = {
  id: number;
  checklistId: number;
  text: string;
  isDone: boolean;
  sortOrder: number;
};
export type Checklist = {
  id: number;
  projectId: number;
  title: string;
  sortOrder: number;
  items: ChecklistItem[];
};

export type CredType = "login" | "api_key" | "token" | "ssh" | "conn_string" | "note";
export type Credential = {
  id: number;
  projectId: number;
  label: string;
  type: CredType;
  username: string | null;
  url: string | null;
  notes: string | null;
  sortOrder: number;
  hasSecret: boolean;
};

export type Note = {
  id: number;
  projectId: number;
  title: string | null;
  contentMd: string | null;
  updatedAt: string;
};
