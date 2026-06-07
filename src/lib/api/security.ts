import { call } from "./client";
import type { CryptoStatus } from "../types";

export const status = () => call<CryptoStatus>("crypto_status");
export const enable = (password: string) => call<void>("master_enable", { password });
export const disable = (password: string) => call<void>("master_disable", { password });
export const change = (oldPassword: string, newPassword: string) => call<void>("master_change", { oldPassword, newPassword });
export const unlock = (password: string) => call<void>("master_unlock", { password });
export const lock = () => call<void>("master_lock");
