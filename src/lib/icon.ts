import { convertFileSrc } from "@tauri-apps/api/core";

// Иконка проекта может быть эмодзи, URL (favicon сайта) или путём к локальному файлу.
const URL_RE = /^https?:\/\//i;
const IMG_EXT_RE = /\.(png|jpe?g|gif|webp|svg|ico|bmp|avif)$/i;

/** true, если значение иконки — изображение (URL/путь), а не эмодзи. */
export function isImageIcon(icon: string | null | undefined): boolean {
  if (!icon) return false;
  return URL_RE.test(icon) || /[/\\]/.test(icon) || IMG_EXT_RE.test(icon);
}

/** Источник для <img>: URL — как есть, локальный путь — через asset-протокол. */
export function iconSrc(icon: string): string {
  return URL_RE.test(icon) ? icon : convertFileSrc(icon);
}
