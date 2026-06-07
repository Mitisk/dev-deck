import { call } from "./client";
import type { UserProfile } from "../types";

export const get = () => call<UserProfile>("user_get");
export const set = (name: string, handle: string | null) => call<void>("user_set", { name, handle });
