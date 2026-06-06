import { describe, it, expect, vi, beforeEach } from "vitest";
import { get } from "svelte/store";
import { toasts, pushToast, dismissToast } from "../lib/stores/toasts";

describe("toasts store", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    toasts.set([]);
  });

  it("adds a toast and returns its id", () => {
    const id = pushToast("Готово", "детали", "ok");
    const list = get(toasts);
    expect(list).toHaveLength(1);
    expect(list[0]).toMatchObject({ id, text: "Готово", sub: "детали", kind: "ok" });
  });

  it("dismisses a toast by id", () => {
    const id = pushToast("X");
    dismissToast(id);
    expect(get(toasts)).toHaveLength(0);
  });

  it("auto-dismisses after timeout", () => {
    pushToast("X");
    expect(get(toasts)).toHaveLength(1);
    vi.advanceTimersByTime(3300);
    expect(get(toasts)).toHaveLength(0);
  });
});
