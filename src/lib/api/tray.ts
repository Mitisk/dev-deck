import { call } from "./client";

export const resync = () => call<void>("tray_resync");
