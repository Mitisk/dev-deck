import { call } from "./client";
import type { SearchHit } from "../types";

export const global = (query: string) => call<SearchHit[]>("search_global", { query });
