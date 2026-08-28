/**
 * What this application can ask of the agent beside it, and what comes back.
 *
 * The one place the interface crosses to the Rust side about the agent. Everything here wraps
 * `invoke` in a way that cannot throw at a screen — the same arrangement every other file in
 * this directory has — because a webview that threw here would take the conversation with it.
 *
 * **A run's events arrive on a channel rather than as a return value.** One channel per run,
 * created here and handed over with the question, so nothing has to be filtered by identifier
 * on this side and a channel cannot outlive the run it belongs to. It closes when one of the
 * three terminal events arrives.
 *
 * Nothing in this file knows what a frame, a contract version or a process is. That is the
 * Agent Protocol's business and the supervisor's, and the whole point of both is that a screen
 * never has to learn either.
 */

import { Channel, invoke } from "@tauri-apps/api/core";

/** Where the agent is, as far as the application knows. */
export type AgentState =
  | "notBundled"
  | "notStarted"
  | "willNotStart"
  | "running"
  | "stopped";

/** Which part of answering a run has reached. */
export type Stage = "gathering" | "thinking" | "shaping" | "calling";

/** What the agent is being asked for. */
export type Intent = "chat" | "propose";

/** Who said one turn of a conversation. */
export type Role = "person" | "agent";

/** One turn of a conversation, as it crosses to the agent. */
export interface Turn {
  /** Who said it. */
  role: Role;
  /** What was said. */
  content: string;
}

/**
 * What the application knows about the agent without asking it anything.
 *
 * Every figure may be `null`, and `null` is doing real work: nobody measured it. A screen that
 * drew a zero or an empty string for one of these would be claiming a reading nobody took.
 */
export interface AgentStatus {
  /** Where the agent is. */
  state: AgentState;
  /**
   * The model the **running** agent reported, or `null` while none is running.
   *
   * Not what was chosen. The two differ from the moment somebody changes the setting until the
   * agent next starts, and they are two facts worth showing separately.
   */
  model: string | null;
  /** What the running agent calls itself, or `null` while none is running. */
  agentVersion: string | null;
  /**
   * The identifier of the run in flight, or `null` when none is.
   *
   * An identifier rather than a boolean, and the difference is a webview that reloaded. A page
   * that has just mounted holds no memory of the run it started, so a boolean would tell it
   * only that it cannot ask anything — for as long as a run it can neither name nor cancel goes
   * on. Naming the run is what lets the new page adopt it.
   */
  inFlight: string | null;
}

/** Everything a screen is told about a run, as it happens. */
export type AgentEvent =
  | { event: "started" }
  | { event: "progress"; stage: Stage; done: number | null; total: number | null }
  | { event: "token"; text: string }
  | { event: "proposal"; title: string; body: string; sources: string[] }
  | { event: "completed" }
  | { event: "cancelled" }
  | { event: "failed"; code: string };

/** Nothing has been asked and nothing is known. What a failed read falls back to. */
const NOTHING_KNOWN: AgentStatus = {
  state: "notStarted",
  model: null,
  agentVersion: null,
  inFlight: null,
};

/**
 * Where the agent is, right now.
 *
 * A failure is `NOTHING_KNOWN` rather than an error: the question could not be asked, and a
 * screen that drew nothing at all would be worse than one saying the agent has not started.
 */
export async function agentStatus(): Promise<AgentStatus> {
  try {
    return await invoke<AgentStatus>("agent_status");
  } catch {
    return NOTHING_KNOWN;
  }
}

/**
 * Asks the agent something, and calls `onEvent` with everything it produces.
 *
 * Resolves as soon as the question has been handed over — **not** when the run finishes. Every
 * result arrives through `onEvent`, failures included; what this answers is the one thing the
 * channel cannot say, which is whether the question was accepted at all.
 *
 * @param id - This run's identifier, which `cancelAgent` takes to stop it.
 * @param intent - What is being asked for.
 * @param messages - The conversation so far, oldest first.
 * @param onEvent - Called with each event, in the order it was produced.
 * @returns `null` when the question was accepted, or the identifier of the refusal.
 */
export async function askAgent(
  { id, intent, messages }: { id: string; intent: Intent; messages: Turn[] },
  onEvent: (event: AgentEvent) => void,
): Promise<string | null> {
  const channel = new Channel<AgentEvent>();
  channel.onmessage = onEvent;

  try {
    await invoke("agent_ask", { question: { id, intent, messages }, onEvent: channel });
    return null;
  } catch (refusal) {
    return codeOf(refusal);
  }
}

/**
 * Asks for a run to stop.
 *
 * The run ends either way. Whether the agent honoured it or had to be ended is this
 * application's business and stays in its log.
 *
 * @param id - The run to stop.
 * @returns `null` when it was accepted, or the identifier of the refusal.
 */
export async function cancelAgent(id: string): Promise<string | null> {
  try {
    await invoke("agent_cancel", { id });
    return null;
  } catch (refusal) {
    return codeOf(refusal);
  }
}

/** Ends the agent, so that the next question starts a fresh one with whatever is chosen now. */
export async function stopAgent(): Promise<void> {
  try {
    await invoke("agent_stop");
  } catch {
    // Nothing to report and nothing to do: what was asked for was that it stop running, and a
    // command that could not be reached is an application that is going anyway.
  }
}

/**
 * The identifier out of whatever came back from a refused call.
 *
 * The Rust side answers with an object carrying one identifier, and a call that could not be
 * reached at all answers with something else entirely. Both end here, and neither is ever
 * drawn as it arrived: what a person reads is looked up from this in the catalogs.
 */
function codeOf(refusal: unknown): string {
  if (typeof refusal === "object" && refusal !== null && "code" in refusal) {
    const { code } = refusal as { code: unknown };
    if (typeof code === "string") {
      return code;
    }
  }
  return "agent_stopped";
}
