import { pushToast } from "./stores/toasts";

export async function copy(text: string, label = "Скопировано"): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    pushToast(label, "", "ok");
  } catch {
    pushToast("Не удалось скопировать", "", "error");
  }
}

// Копировать секрет и очистить буфер через clearMs (по умолчанию 20с).
export async function copySecret(text: string, clearMs = 20000): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    pushToast("Секрет скопирован", `Буфер очистится через ${Math.round(clearMs / 1000)} с`, "ok");
    setTimeout(() => {
      navigator.clipboard.writeText("").catch(() => {});
    }, clearMs);
  } catch {
    pushToast("Не удалось скопировать", "", "error");
  }
}
