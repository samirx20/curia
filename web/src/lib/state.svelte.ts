// What the app shows, all from the host: threads, teams, the inbox. Refreshed on sign-in and
// whenever the host says something happened.
import { link, type LinkState } from "./link";

export const app = $state({
  link: "connecting" as LinkState,
  threads: [] as any[],
  teams: [] as any[],
  inbox: [] as any[],
  error: "",
  /** This device's role: owner, approve or view. */
  role: "",
});

let pending: ReturnType<typeof setTimeout> | null = null;

export async function refresh(): Promise<void> {
  try {
    const [threads, teams, inbox] = await Promise.all([link.call("threads_list"), link.call("teams_list"), link.call("inbox_list")]);
    app.threads = threads;
    app.teams = teams;
    app.inbox = inbox;
  } catch (e) {
    app.error = (e as Error).message;
  }
}

/** Many events in a row cause one refresh. */
export function soon(): void {
  if (pending) return;
  pending = setTimeout(() => { pending = null; refresh(); }, 250);
}

export const teamOf = (t: any) => app.teams.find((x) => x.id === t?.team_id) ?? (t ? { cli: t.cli, name: "" } : null);
export const needsYou = (threadId: string) => app.inbox.some((q) => q.thread_id === threadId);

/** Run after every sign-in (an open thread catches up from its last event). */
export const onSignIn = new Set<() => void>();

link.onState = (s) => (app.link = s);
link.onReady = () => {
  app.role = link.role;
  refresh();
  for (const f of onSignIn) f();
};
link.onActivity = () => soon();
