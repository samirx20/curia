// A thread's events folded into what the screen shows (the same rules as the desktop's
// Conversation.qml): consecutive tool calls make one group, streamed text grows in place,
// approvals, hand-offs and results become cards.

export interface Step { item: string; name: string; input: any; state: "running" | "done" | "error"; output: string }
export type Row =
  | { kind: "user"; text: string }
  | { kind: "task"; from: string; text: string }
  | { kind: "text" | "thinking"; item: string; text: string }
  | { kind: "tools"; steps: Step[] }
  | { kind: "approval"; requestId: string; e: any; phase: string }
  | { kind: "handoff"; e: any }
  | { kind: "handback"; e: any; phase: string }
  | { kind: "message"; e: any }
  | { kind: "notice" | "failed"; text: string };

export class Rows {
  // Reactive, so text that streams into a row and steps that finish redraw it. Rows are kept
  // by the proxied copy the array holds (changing the original wouldn't redraw anything).
  rows = $state<Row[]>([]);
  private items = new Map<string, Row>();

  add(e: any): void {
    const r = this.rows;
    const last = r[r.length - 1];
    switch (e.type) {
      case "turn_started":
        if (e.by === "curia") r.push({ kind: "notice", text: `Curia: ${e.prompt ?? ""}` });
        else if (e.by) r.push({ kind: "task", from: e.by, text: e.prompt ?? "" });
        else if (e.prompt === null) r.push({ kind: "notice", text: "The agent carried on by itself" });
        else r.push({ kind: "user", text: e.prompt });
        break;
      case "text_started": this.text(e.item, e.kind === "thinking" ? "thinking" : "text", e.text); break;
      case "text_delta": { const t = this.items.get(e.item); if (t && "text" in t) t.text += e.text; break; }
      case "text_done": { const t = this.items.get(e.item); if (t && "text" in t) t.text = e.text; else this.text(e.item, "text", e.text); break; }
      case "tool_call": {
        const step: Step = { item: e.item, name: e.name, input: e.input, state: "running", output: "" };
        if (last?.kind === "tools") last.steps.push(step);
        else r.push({ kind: "tools", steps: [step] });
        this.items.set(e.item, r[r.length - 1]);
        break;
      }
      case "tool_result": {
        const g = this.items.get(e.item);
        const s = g?.kind === "tools" ? g.steps.find((x) => x.item === e.item) : undefined;
        if (s) { s.state = e.is_error ? "error" : "done"; s.output = e.output; }
        break;
      }
      case "approval_requested": r.push({ kind: "approval", requestId: e.request_id, e, phase: "pending" }); this.items.set(`req:${e.request_id}`, r[r.length - 1]); break;
      case "approval_resolved": { const a = this.items.get(`req:${e.request_id}`); if (a?.kind === "approval") a.phase = e.resolution; break; }
      case "handed_off": r.push({ kind: "handoff", e }); break;
      case "hand_back": r.push({ kind: "handback", e, phase: e.merge.result }); if (e.merge.merge_id) this.items.set(`merge:${e.merge.merge_id}`, r[r.length - 1]); break;
      case "merge_undone": { const m = this.items.get(`merge:${e.merge_id}`); if (m?.kind === "handback") m.phase = "undone"; break; }
      case "message":
        // A result's own card already says it.
        if (e.from && !(last?.kind === "handback" && last.e.from === e.from)) r.push({ kind: "message", e });
        break;
      case "notice": r.push({ kind: "notice", text: e.text }); break;
      case "turn_finished":
        if (e.outcome.outcome === "interrupted") r.push({ kind: "notice", text: "Stopped." });
        else if (e.outcome.outcome === "failed") r.push({ kind: "failed", text: e.outcome.message });
        else if (e.outcome.outcome === "usage_limited") r.push({ kind: "notice", text: "The team ran out of quota." });
        break;
    }
  }

  private text(item: string, kind: "text" | "thinking", text: string): void {
    this.rows.push({ kind, item, text });
    this.items.set(item, this.rows[this.rows.length - 1]);
  }
}

/** "Ran 2 commands, edited 1 file": a tool group's line. */
export function summary(steps: Step[]): string {
  const n: Record<string, number> = {};
  for (const s of steps) n[s.name] = (n[s.name] ?? 0) + 1;
  const say: Record<string, string> = {
    Read: "read %1 file(s)", Write: "wrote %1 file(s)", Edit: "edited %1 file(s)", Bash: "ran %1 command(s)", Shell: "ran %1 command(s)",
    Grep: "searched %1 time(s)", Glob: "listed files %1 time(s)", Agent: "started %1 subagent(s)", write_file: "wrote %1 file(s)",
    run_command: "ran %1 command(s)", mcp__curia__report_result: "reported the result", mcp__curia__hand_off: "handed off %1 task(s)",
    mcp__curia__send_message: "sent %1 message(s)", mcp__curia__read_messages: "read its messages", mcp__curia__list_teams: "looked at the teams",
  };
  const s = Object.entries(n).map(([k, c]) => (say[k] ?? `used ${k.replace(/^mcp__[^_]+__/, "")} %1×`).replace("%1", String(c))).join(", ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export function kindIcon(name: string): string {
  if (/^(Read|read_file|view_file)$/.test(name)) return "read";
  if (/^(Write|Edit|MultiEdit|write_file|edit_file)$/.test(name)) return "edit";
  if (/^(Bash|Shell|run_command)$/.test(name)) return "shell";
  if (/^(Grep|Glob|search|grep_search|list_dir)$/.test(name)) return "search";
  if (/^mcp__/.test(name)) return "mcp";
  return "tools";
}

/** What a request wants, in one line. */
export function asked(input: any): string {
  return input?.command ?? input?.file_path ?? input?.questions?.[0]?.question ?? JSON.stringify(input ?? {});
}
