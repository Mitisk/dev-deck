import { writable } from "svelte/store";
import { browser } from "$app/environment";

// Включённость опциональных вкладок проекта. Выключенные просто скрываются.
export type Features = {
  tasks: boolean;
  checklists: boolean;
  creds: boolean;
};

const KEY = "devdeck-features";
const DEFAULTS: Features = { tasks: true, checklists: true, creds: true };

function initial(): Features {
  if (!browser) return { ...DEFAULTS };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    return { ...DEFAULTS, ...(JSON.parse(raw) as Partial<Features>) };
  } catch {
    return { ...DEFAULTS };
  }
}

export const features = writable<Features>(initial());

features.subscribe((f) => {
  if (!browser) return;
  localStorage.setItem(KEY, JSON.stringify(f));
});

export function toggleFeature(key: keyof Features): void {
  features.update((f) => ({ ...f, [key]: !f[key] }));
}
