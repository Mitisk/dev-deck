import { writable, get } from "svelte/store";
import { browser } from "$app/environment";
import * as updaterApi from "../api/updater";
import { pushToast } from "./toasts";

// Состояние обновления приложения. Показывается в левом нижнем углу сайдбара и в настройках.
export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "none"; checkedAt: number }
  | { status: "available"; version: string; notes: string | null }
  | { status: "downloading"; version: string; percent: number | null }
  | { status: "ready"; version: string }
  | { status: "error"; message: string };

export const updateState = writable<UpdateState>({ status: "idle" });

// Текущая версия приложения (из tauri.conf.json → package.json).
export const appVersion = writable<string>("");

export async function loadAppVersion(): Promise<void> {
  try {
    appVersion.set(await updaterApi.appVersion());
  } catch {
    /* вне Tauri (vite dev в браузере) — оставляем пусто */
  }
}

// «Проверять при запуске» — хранится в localStorage, по умолчанию включено.
const KEY = "devdeck-auto-update-check";

function initialAuto(): boolean {
  if (!browser) return true;
  try {
    const raw = localStorage.getItem(KEY);
    return raw === null ? true : raw === "1";
  } catch {
    return true;
  }
}

export const autoCheck = writable<boolean>(initialAuto());

autoCheck.subscribe((v) => {
  if (!browser) return;
  try {
    localStorage.setItem(KEY, v ? "1" : "0");
  } catch {
    /* не критично */
  }
});

export function toggleAutoCheck(): void {
  autoCheck.update((v) => !v);
}

function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return "Не удалось проверить обновления";
}

// Проверить обновления. silent = фоновая проверка при запуске: без тостов об отсутствии/ошибке.
export async function checkForUpdate(opts: { silent?: boolean } = {}): Promise<void> {
  const s = get(updateState);
  if (s.status === "checking" || s.status === "downloading" || s.status === "ready") return;
  updateState.set({ status: "checking" });
  try {
    const info = await updaterApi.checkForUpdate();
    if (info) {
      updateState.set({ status: "available", version: info.version, notes: info.notes });
    } else {
      updateState.set({ status: "none", checkedAt: Date.now() });
      if (!opts.silent) pushToast("Обновлений нет", "У вас последняя версия", "ok");
    }
  } catch (e) {
    const message = errorMessage(e);
    // при фоновой проверке ошибку сети не показываем — просто возвращаемся в idle
    updateState.set(opts.silent ? { status: "idle" } : { status: "error", message });
    if (!opts.silent) pushToast("Проверка обновлений", message, "error");
  }
}

// Скачать и установить найденное обновление, затем перезапустить приложение.
export async function installUpdate(): Promise<void> {
  const s = get(updateState);
  if (s.status !== "available") return;
  const version = s.version;
  updateState.set({ status: "downloading", version, percent: null });
  try {
    await updaterApi.downloadAndInstall((percent) => {
      updateState.set({ status: "downloading", version, percent });
    });
    updateState.set({ status: "ready", version });
    // На Windows установщик сам завершает приложение; relaunch на случай, если не завершил.
    await updaterApi.relaunchApp();
  } catch (e) {
    const message = errorMessage(e);
    updateState.set({ status: "error", message });
    pushToast("Обновление не установлено", message, "error");
  }
}

// Повторная попытка после ошибки.
export function resetUpdateState(): void {
  updateState.set({ status: "idle" });
}
