import { describe, it, expect } from "vitest";
import { reorderIds } from "../lib/credsOrder";

describe("reorderIds", () => {
  it("перемещает элемент перед указанным (вверх)", () => {
    expect(reorderIds([1, 2, 3], 3, 1)).toEqual([3, 1, 2]);
  });

  it("перемещает элемент перед указанным (вниз)", () => {
    expect(reorderIds([1, 2, 3], 1, 3)).toEqual([2, 1, 3]);
  });

  it("beforeId === null → в конец", () => {
    expect(reorderIds([1, 2, 3], 1, null)).toEqual([2, 3, 1]);
  });

  it("бросок на самого себя не меняет порядок", () => {
    expect(reorderIds([1, 2, 3], 2, 2)).toEqual([1, 2, 3]);
  });

  it("несуществующий beforeId → в конец", () => {
    expect(reorderIds([1, 2, 3], 1, 99)).toEqual([2, 3, 1]);
  });
});
