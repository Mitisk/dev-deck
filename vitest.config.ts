import { defineConfig } from "vitest/config";
import { resolve } from "node:path";

// Отдельный конфиг для тестов: без плагина sveltekit(), чтобы vitest не зависел
// от генерации .svelte-kit и не флакал при запуске сразу после `npm run check`/`build`.
// Стор-тесты импортируют только svelte/store и модули из $lib (алиас ниже).
export default defineConfig({
  resolve: {
    alias: { $lib: resolve("./src/lib") },
  },
  test: {
    environment: "node",
    include: ["src/tests/**/*.test.ts"],
    // Один форк-процесс: стабильнее на Windows, чем пул воркеров (тот изредка
    // молча падает при быстром последовательном запуске). Для пары тест-файлов
    // параллелизм не нужен.
    pool: "forks",
    poolOptions: { forks: { singleFork: true } },
  },
});
