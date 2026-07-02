import { call } from "./client";

export const openPath = (path: string) => call<void>("open_path", { path });
export const openInEditor = (path: string) => call<void>("open_in_editor", { path });
export const openFileInEditor = (path: string) => call<void>("open_file_in_editor", { path });
export const launchPutty = (id: number) => call<void>("launch_putty", { id });
export const openTerminal = (path: string) => call<void>("open_terminal", { path });
export const openUrl = (url: string) => call<void>("open_url", { url });
export const openShortcut = (path: string) => call<void>("open_shortcut", { path });
export const openGitBash = (path: string) => call<void>("open_git_bash", { path });
