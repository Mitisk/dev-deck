import type { ProjectStatus } from "./types";

const STATUS_LABELS: Record<ProjectStatus, string> = {
  active: "Активен",
  paused: "На паузе",
  done: "Завершён",
  archived: "В архиве",
};

export function statusLabel(s: ProjectStatus): string {
  return STATUS_LABELS[s] ?? s;
}

export const STATUS_OPTIONS: { value: ProjectStatus; label: string }[] = [
  { value: "active", label: "Активен" },
  { value: "paused", label: "На паузе" },
  { value: "done", label: "Завершён" },
  { value: "archived", label: "В архиве" },
];
