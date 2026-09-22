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
  healthUrl: string | null;
  pinned: boolean;
  sortOrder: number;
  groupId: number | null; // папка в сайдбаре; null = корень
  tags: string[];
  createdAt: string;
  updatedAt: string;
};

// Папка проектов в сайдбаре (один уровень).
export type ProjectGroup = { id: number; name: string; sortOrder: number };

export type User = { name: string; handle: string; initials: string };

export type UserProfile = { name: string; handle: string | null };

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

export type GitFile = {
  path: string;
  code: string; // M | A | D | R | T | ?
  staged: boolean;
};

export type GitChanges = {
  insertions: number;
  deletions: number;
  files: GitFile[];
};

// Статус задачи = ключ колонки канбана. Дефолтные — todo/doing/done; кастомные — c{id}.
export type TaskStatus = string;
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
  labelIds: number[];
};

export type Label = { id: number; projectId: number; name: string; color: string | null; sortOrder: number };

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
  keyPath: string | null;
  isGlobal: boolean;
  startupCmd: string | null; // команда после входа (PuTTY, тип ssh)
};

export type Note = {
  id: number;
  projectId: number;
  title: string | null;
  contentMd: string | null;
  updatedAt: string;
};

export type Link = { id: number; projectId: number; label: string; url: string; icon: string | null; sortOrder: number };
export type FileShortcut = { id: number; projectId: number; label: string; path: string; sortOrder: number; showTerminal: boolean };

export type ProjectCommand = {
  id: number;
  projectId: number;
  label: string;
  command: string;
  workingDir: string | null;
  runIn: string;
  icon: string | null;
  sortOrder: number;
};

export type SearchHit = {
  kind: string;
  projectId: number;
  projectName: string;
  id: number;
  title: string;
  subtitle: string;
};

export type AttentionItem = {
  projectId: number;
  name: string;
  color: string | null;
  icon: string | null;
  branch: string | null;
  ahead: number;
  dirty: number;
  lastHash: string | null;
  lastMessage: string | null;
};

export type BackupInfo = { name: string; sizeBytes: number; createdEpoch: number };
export type ImportSummary = { projects: number };
export type ChecklistTemplate = { id: number; name: string; items: string[] };

export type CryptoStatus = { mode: "dpapi" | "master"; locked: boolean };

export type AgendaItem = {
  projectId: number;
  projectName: string;
  projectColor: string | null;
  taskId: number;
  title: string;
  dueDate: string;
  priority: number;
};

export type TaskColumn = { id: number; projectId: number; key: string; name: string; isDone: boolean; sortOrder: number };
