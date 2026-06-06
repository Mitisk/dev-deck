import { writable } from "svelte/store";

export type ToastKind = "ok" | "info" | "error";
export type Toast = { id: number; text: string; sub: string; kind: ToastKind };

let seq = 1;
export const toasts = writable<Toast[]>([]);

export function pushToast(text: string, sub = "", kind: ToastKind = "ok"): number {
  const id = seq++;
  toasts.update((list) => [...list, { id, text, sub, kind }]);
  setTimeout(() => dismissToast(id), 3200);
  return id;
}

export function dismissToast(id: number): void {
  toasts.update((list) => list.filter((t) => t.id !== id));
}
