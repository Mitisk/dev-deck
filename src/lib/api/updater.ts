// Обёртка над tauri-plugin-updater / plugin-process: компоненты и сторы
// не трогают плагины напрямую (конвенция проекта).
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export type UpdateInfo = {
  version: string;
  notes: string | null;
  date: string | null;
};

// Найденное обновление держим здесь: объект Update нужен для скачивания.
let current: Update | null = null;

// Запросить манифест latest.json. null = обновлений нет.
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  const u = await check();
  current = u;
  if (!u) return null;
  return { version: u.version, notes: u.body ?? null, date: u.date ?? null };
}

// Скачать и установить найденное обновление. onProgress: 0..100 или null, если размер неизвестен.
export async function downloadAndInstall(onProgress: (percent: number | null) => void): Promise<void> {
  if (!current) throw new Error("Сначала нужно проверить обновления");
  let total = 0;
  let got = 0;
  await current.downloadAndInstall((ev) => {
    if (ev.event === "Started") {
      total = ev.data.contentLength ?? 0;
      onProgress(total ? 0 : null);
    } else if (ev.event === "Progress") {
      got += ev.data.chunkLength;
      onProgress(total ? Math.min(100, Math.round((got * 100) / total)) : null);
    } else if (ev.event === "Finished") {
      onProgress(100);
    }
  });
}

export const relaunchApp = (): Promise<void> => relaunch();

export const appVersion = (): Promise<string> => getVersion();
