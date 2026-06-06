import { writable } from "svelte/store";
import { browser } from "$app/environment";

type Theme = "dark" | "light";
const KEY = "devdeck-theme";

function initial(): Theme {
  if (!browser) return "dark";
  return localStorage.getItem(KEY) === "light" ? "light" : "dark";
}

export const theme = writable<Theme>(initial());

// Применяем класс к <html> и сохраняем при каждом изменении (только в браузере).
theme.subscribe((t) => {
  if (!browser) return;
  document.documentElement.classList.toggle("dark", t === "dark");
  document.documentElement.classList.toggle("light", t === "light");
  localStorage.setItem(KEY, t);
});

export function toggleTheme(): void {
  theme.update((t) => (t === "dark" ? "light" : "dark"));
}
