import { call } from "./client";

export const resync = () => call<void>("watch_resync");
