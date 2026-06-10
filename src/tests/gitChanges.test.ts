import { describe, it, expect } from "vitest";
import { changeCategories } from "../lib/gitChanges";

describe("changeCategories", () => {
  it("разбивает dirty на modified/untracked/staged", () => {
    expect(changeCategories(6, 1, 2)).toEqual({ modified: 3, untracked: 2, staged: 1 });
  });

  it("клампит modified в 0 при пересечениях", () => {
    expect(changeCategories(2, 2, 2)).toEqual({ modified: 0, untracked: 2, staged: 2 });
  });

  it("всё чисто", () => {
    expect(changeCategories(0, 0, 0)).toEqual({ modified: 0, untracked: 0, staged: 0 });
  });
});
