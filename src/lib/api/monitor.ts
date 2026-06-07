import { call } from "./client";

export type HealthResult = {
  ok: boolean;
  reachable: boolean;
  status: number;
  latencyMs: number;
  detail: string | null;
};

export const check = (url: string) => call<HealthResult>("health_check", { url });
