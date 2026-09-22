// Раскладка сайдбара по секциям и планирование drag-and-drop — чистые функции без DOM.
import type { Project, ProjectGroup } from "./types";
import { reorderIds } from "./credsOrder";

export type Section = { kind: "pinned" } | { kind: "group"; id: number } | { kind: "root" };

export type SidebarLayout = {
  pinned: Project[];
  folders: { group: ProjectGroup; items: Project[] }[];
  rest: Project[];
};

function byOrder(a: Project, b: Project): number {
  return a.sortOrder - b.sortOrder || a.name.localeCompare(b.name);
}

// Секция проекта: закреплённые — отдельно (независимо от папки); папка — если она существует; иначе корень.
export function sectionOf(p: Project, groups: ProjectGroup[]): Section {
  if (p.pinned) return { kind: "pinned" };
  if (p.groupId != null && groups.some((g) => g.id === p.groupId)) return { kind: "group", id: p.groupId };
  return { kind: "root" };
}

export function sameSection(a: Section, b: Section): boolean {
  if (a.kind !== b.kind) return false;
  return a.kind !== "group" || b.kind !== "group" || a.id === b.id;
}

// Архивные не показываем. Порядок внутри секций — sortOrder, затем имя.
export function layoutSidebar(projects: Project[], groups: ProjectGroup[]): SidebarLayout {
  const visible = projects.filter((p) => p.status !== "archived");
  const sortedGroups = [...groups].sort((a, b) => a.sortOrder - b.sortOrder || a.id - b.id);
  const pinned = visible.filter((p) => p.pinned).sort(byOrder);
  const folders = sortedGroups.map((group) => ({
    group,
    items: visible.filter((p) => !p.pinned && p.groupId === group.id).sort(byOrder),
  }));
  const rest = visible
    .filter((p) => !p.pinned && (p.groupId == null || !sortedGroups.some((g) => g.id === p.groupId)))
    .sort(byOrder);
  return { pinned, folders, rest };
}

function sectionItems(layout: SidebarLayout, s: Section): Project[] {
  if (s.kind === "pinned") return layout.pinned;
  if (s.kind === "root") return layout.rest;
  return layout.folders.find((f) => f.group.id === s.id)?.items ?? [];
}

export type DropPlan = {
  // undefined = папка не меняется; null = в корень; число = в папку
  setGroup?: number | null;
  // новый порядок id целевой секции
  ids: number[];
};

// Спланировать бросок проекта dragId в секцию target перед beforeId (null = в конец).
// null = бросок недопустим или ничего не меняет. Перенос в/из «Закреплённых» перетаскиванием запрещён.
export function planProjectDrop(
  layout: SidebarLayout,
  groups: ProjectGroup[],
  dragId: number,
  target: { section: Section; beforeId: number | null },
): DropPlan | null {
  const all = [...layout.pinned, ...layout.rest, ...layout.folders.flatMap((f) => f.items)];
  const dragged = all.find((p) => p.id === dragId);
  if (!dragged) return null;
  const source = sectionOf(dragged, groups);
  if ((source.kind === "pinned") !== (target.section.kind === "pinned")) return null;

  const targetIds = sectionItems(layout, target.section).map((p) => p.id);
  if (sameSection(source, target.section)) {
    const next = reorderIds(targetIds, dragId, target.beforeId);
    if (next.join(",") === targetIds.join(",")) return null;
    return { ids: next };
  }
  const ids = reorderIds([...targetIds, dragId], dragId, target.beforeId);
  return { setGroup: target.section.kind === "group" ? target.section.id : null, ids };
}
