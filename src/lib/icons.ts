import { createIcons, icons } from "lucide";

// Иконки бандлятся локально (без CDN). Наши плейсхолдеры — <svg data-lucide="name">,
// поэтому nameAttr = "data-lucide".
export function paintIcons(): void {
  createIcons({ icons, nameAttr: "data-lucide" });
}

export function ico(name: string, cls = "ic"): string {
  return `<svg class="${cls}" data-lucide="${name}"></svg>`;
}
