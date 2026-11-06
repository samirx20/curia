// The design's line icons on a 24-unit grid (the same paths as the desktop's Theme.qml).
export const icons: Record<string, string> = {
  plus: "M12 5v14M5 12h14",
  close: "M6 6l12 12M18 6L6 18",
  back: "M14 6l-6 6 6 6",
  open: "M10 7l5 5-5 5",
  down: "M7 10l5 5 5-5",
  send: "M12 19V6M6 12l6-6 6 6",
  folder: "M3 6.5h6l2 2h10V19H3z",
  inbox: "M3 13h5l1.5 3h5l1.5-3h5M5.5 5h13l2.5 8v6H3v-6z",
  runs: "M8 7h11M8 12h11M8 17h11M4.5 7h.01M4.5 12h.01M4.5 17h.01",
  people: "M9 11a3.5 3.5 0 1 0 0-7a3.5 3.5 0 1 0 0 7zM2.5 20c0-3.6 2.9-6 6.5-6s6.5 2.4 6.5 6M16 4.5a3.5 3.5 0 0 1 0 6.5M18 14.3c2 .8 3.5 2.7 3.5 5.7",
  shell: "M5 7l5 5-5 5M12 17h7",
  read: "M6.5 3h8l4 4v14h-12zM14.5 3v4h4M9 12h6M9 16h6",
  edit: "M4 20h4L19 9l-4-4L4 16zM14 6l4 4",
  search: "M11 4.5a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13zM20 20l-4.3-4.3",
  mcp: "M9 3v5M15 3v5M6 8h12v3a6 6 0 0 1-12 0zM12 17v4",
  tools: "M14.5 6.5a4 4 0 0 0-5.3 5.3L4 17l3 3 5.2-5.2a4 4 0 0 0 5.3-5.3l-2.5 2.5-2.5-2.5z",
  handOut: "M7 17L17 7M9 7h8v8",
  handIn: "M17 7L7 17M15 17H7V9",
  message: "M4 5h16v11H9l-5 4z",
  check: "M5 12.5l4.5 4.5L19 7.5",
  ring: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z",
  arc: "M12 3a9 9 0 0 1 9 9",
  dot: "M12 7.5a4.5 4.5 0 1 0 0 9a4.5 4.5 0 1 0 0-9z",
  pause: "M8.5 5v14M15.5 5v14",
  usage: "M5 20v-8M12 20V5M19 20v-5",
  warning: "M12 3.5l9.5 16.5h-19zM12 10v4.5M12 17.5v.01",
  phone: "M8.5 2.5h7a1.5 1.5 0 0 1 1.5 1.5v16a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 7 20V4a1.5 1.5 0 0 1 1.5-1.5zM11 18.5h2",
};

export const clis: Record<string, { label: string; glyph: string }> = {
  claude: { label: "Claude", glyph: "M12 4v16M4 12h16M6.3 6.3l11.4 11.4M17.7 6.3L6.3 17.7" },
  codex: { label: "Codex", glyph: "M9 7l-5 5 5 5M15 7l5 5-5 5" },
  agy: { label: "Antigravity", glyph: "M12 20V5M6 11l6-6 6 6" },
};

export const cliLabel = (cli: string) => clis[cli]?.label ?? cli;
export const who = (team: { cli: string; name: string } | null | undefined) => (team ? `${cliLabel(team.cli)} · ${team.name}` : "no team");
export function initials(name: string): string {
  const words = (name || "").split(/[\s_\-.]+/).filter(Boolean);
  return (words.length > 1 ? words[0][0] + words[1][0] : (words[0] || "?").slice(0, 2)).toUpperCase();
}
export function ago(ms: number | null | undefined): string {
  if (!ms) return "";
  const m = Math.max(0, Math.round((Date.now() - ms) / 60000));
  return m < 1 ? "now" : m < 60 ? `${m} min` : m < 1440 ? `${Math.round(m / 60)} h` : `${Math.round(m / 1440)} d`;
}
