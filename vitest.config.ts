import { defineConfig } from "vitest/config";
import { resolve } from "node:path";

// Отдельный конфиг для тестов: без плагина sveltekit() (стор-тесты импортируют
// только svelte/store и модули из $lib — алиас ниже). Vitest держим на 3.x:
// Vitest 4 несовместим с Vite 6 и падает на загрузке сьютов.
export default defineConfig({
  resolve: {
    alias: { $lib: resolve("./src/lib") },
  },
  test: {
    environment: "node",
    include: ["src/tests/**/*.test.ts"],
  },
});
