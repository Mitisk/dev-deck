import { call } from "./client";

// Возвращает версию схемы (PRAGMA user_version). Проверяет, что БД открыта
// и миграции применены.
export function dbHealth(): Promise<number> {
  return call<number>("db_health");
}
