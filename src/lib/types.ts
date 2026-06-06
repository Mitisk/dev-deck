export type GitInfo = {
  branch: string;
  ahead: number;
  behind: number;
  dirty: number;
  lastHash: string;
  lastMsg: string;
  staged: number;
  untracked: number;
};

export type Command = { label: string; run: string; icon: string; tint: string };
export type Link = { title: string; url: string; icon: string };
export type Task = { t: string; pri: "high" | "med" | "low"; due: string };
export type Tasks = { todo: Task[]; doing: Task[]; done: Task[] };
export type ChecklistItem = { t: string; done: boolean };
export type Checklist = { title: string; items: ChecklistItem[] };
export type CredField = { k: string; v: string; secret: boolean };
export type Cred = { title: string; type: string; fields: CredField[] };

export type Project = {
  id: string;
  name: string;
  emoji: string;
  color: string;
  pinned: boolean;
  status: string;
  tags: string[];
  path: string;
  desc: string;
  git: GitInfo;
  commands: Command[];
  links: Link[];
  tasks: Tasks;
  checklists: Checklist[];
  creds: Cred[];
  note: string;
};

export type User = { name: string; handle: string; initials: string };
