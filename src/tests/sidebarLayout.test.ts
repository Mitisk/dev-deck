import { describe, it, expect } from "vitest";
import { layoutSidebar, planProjectDrop, sectionOf } from "../lib/sidebarLayout";
import type { Project, ProjectGroup } from "../lib/types";

function proj(id: number, o: Partial<Project> = {}): Project {
  return {
    id,
    name: `P${id}`,
    description: null,
    status: "active",
    color: null,
    icon: null,
    path: null,
    repoPath: null,
    healthUrl: null,
    pinned: false,
    sortOrder: id,
    groupId: null,
    tags: [],
    createdAt: "",
    updatedAt: "",
    ...o,
  };
}
const g1: ProjectGroup = { id: 1, name: "A", sortOrder: 1 };
const g2: ProjectGroup = { id: 2, name: "B", sortOrder: 0 };

describe("layoutSidebar", () => {
  it("раскладывает по секциям, папки — по sortOrder, архивные скрыты", () => {
    const projects = [
      proj(1, { pinned: true, groupId: 1 }), // закреплённый не показывается в папке
      proj(2, { groupId: 1 }),
      proj(3),
      proj(4, { groupId: 2, sortOrder: 0 }),
      proj(5, { status: "archived" }),
      proj(6, { groupId: 99 }), // папки нет → корень
    ];
    const l = layoutSidebar(projects, [g1, g2]);
    expect(l.pinned.map((p) => p.id)).toEqual([1]);
    expect(l.folders.map((f) => f.group.id)).toEqual([2, 1]);
    expect(l.folders[0].items.map((p) => p.id)).toEqual([4]);
    expect(l.folders[1].items.map((p) => p.id)).toEqual([2]);
    expect(l.rest.map((p) => p.id)).toEqual([3, 6]);
  });

  it("сортирует по sortOrder, затем по имени", () => {
    const l = layoutSidebar([proj(1, { name: "b", sortOrder: 5 }), proj(2, { name: "a", sortOrder: 5 }), proj(3, { sortOrder: 0 })], []);
    expect(l.rest.map((p) => p.id)).toEqual([3, 2, 1]);
  });
});

describe("sectionOf", () => {
  it("закреплённый → pinned даже с папкой; несуществующая папка → root", () => {
    expect(sectionOf(proj(1, { pinned: true, groupId: 1 }), [g1])).toEqual({ kind: "pinned" });
    expect(sectionOf(proj(1, { groupId: 1 }), [g1])).toEqual({ kind: "group", id: 1 });
    expect(sectionOf(proj(1, { groupId: 7 }), [g1])).toEqual({ kind: "root" });
  });
});

describe("planProjectDrop", () => {
  const groups = [g1, g2];
  const projects = [proj(1, { pinned: true }), proj(2, { pinned: true }), proj(3), proj(4), proj(5, { groupId: 1 }), proj(6, { groupId: 1 })];
  const layout = layoutSidebar(projects, groups);

  it("перестановка внутри секции", () => {
    expect(planProjectDrop(layout, groups, 4, { section: { kind: "root" }, beforeId: 3 })).toEqual({ ids: [4, 3] });
    expect(planProjectDrop(layout, groups, 6, { section: { kind: "group", id: 1 }, beforeId: 5 })).toEqual({ ids: [6, 5] });
    expect(planProjectDrop(layout, groups, 2, { section: { kind: "pinned" }, beforeId: 1 })).toEqual({ ids: [2, 1] });
  });

  it("бросок без изменения порядка → null", () => {
    expect(planProjectDrop(layout, groups, 3, { section: { kind: "root" }, beforeId: 4 })).toBeNull();
    expect(planProjectDrop(layout, groups, 4, { section: { kind: "root" }, beforeId: null })).toBeNull();
  });

  it("перенос в папку: setGroup + место броска", () => {
    expect(planProjectDrop(layout, groups, 3, { section: { kind: "group", id: 1 }, beforeId: 6 })).toEqual({ setGroup: 1, ids: [5, 3, 6] });
    expect(planProjectDrop(layout, groups, 3, { section: { kind: "group", id: 2 }, beforeId: null })).toEqual({ setGroup: 2, ids: [3] });
  });

  it("перенос из папки в корень: setGroup = null", () => {
    expect(planProjectDrop(layout, groups, 5, { section: { kind: "root" }, beforeId: null })).toEqual({ setGroup: null, ids: [3, 4, 5] });
  });

  it("в/из «Закреплённых» перетаскиванием нельзя", () => {
    expect(planProjectDrop(layout, groups, 3, { section: { kind: "pinned" }, beforeId: null })).toBeNull();
    expect(planProjectDrop(layout, groups, 1, { section: { kind: "root" }, beforeId: null })).toBeNull();
  });

  it("неизвестный проект → null", () => {
    expect(planProjectDrop(layout, groups, 99, { section: { kind: "root" }, beforeId: null })).toBeNull();
  });
});
