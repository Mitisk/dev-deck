// lucide подключён глобально через <script> в app.html (window.lucide).
declare global {
  interface Window {
    lucide?: { createIcons: (opts?: { nameAttr?: string }) => void };
  }
}

// Перерисовать <svg data-lucide="name"> в реальные иконки.
export function paintIcons(): void {
  window.lucide?.createIcons();
}

export function ico(name: string, cls = "ic"): string {
  return `<svg class="${cls}" data-lucide="${name}"></svg>`;
}

export {};
