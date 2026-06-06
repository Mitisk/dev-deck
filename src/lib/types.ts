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
