import { invoke } from "@tauri-apps/api/core";
import { pushToast } from "../stores/toasts";

// Все вызовы бэкенда идут через эту обёртку.
// Бэкенд возвращает Result<T, AppError>; при ошибке Tauri отклоняет промис
// объектом AppError ({ kind, message }) — ловим и показываем тостом.
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    const e = err as { kind?: string; message?: string };
    // «locked» компонент показывает собственным тостом — не дублируем здесь.
    if (e?.kind !== "locked") pushToast("Ошибка", e?.message ?? String(err), "error");
    throw err;
  }
}
