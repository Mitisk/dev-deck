import { call } from "./client";

export type HealthComponent = {
  name: string;
  state: "up" | "warn" | "down" | "unknown";
  label: string;
};

export type HealthResult = {
  ok: boolean;
  reachable: boolean;
  status: number;
  latencyMs: number;
  detail: string | null;
  components: HealthComponent[];
};

export const check = (url: string) => call<HealthResult>("health_check", { url });
