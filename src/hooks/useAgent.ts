/**
 * The conversation with the agent, and the one run that may be in flight.
 *
 * All the state the AI screen has is here, so that what is drawn is a function of what has
 * happened and nothing more. A component that held a growing answer in a ref and repainted
 * itself would be the version of this that goes wrong.
 *
 * # Three facts about an answer, kept apart
 *
 * *Nothing has been asked*, *something was asked and nothing has come back yet*, and *this is
 * the answer* are three states, and the screen draws a different thing for each
 * (`.agents/rules/honest-emptiness.md`). They are `turns` being empty, `running` with an empty
 * `saying`, and `saying` having something in it.
 *
 * A cancelled run **keeps what it already said**. `cancelled` means no further event is coming;
 * it does not withdraw the tokens that arrived, and clearing the line would be throwing away
 * an answer somebody has already read.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import {
  agentStatus,
  askAgent,
  cancelAgent,
  type AgentEvent,
  type AgentStatus,
  type Stage,
  type Turn,
} from "@/lib/agent";

/** One finished turn of the conversation, as the screen draws it. */
export interface Said extends Turn {
  /** Whether this turn is what a run said before it was stopped part-way. */
  cut?: true;
}

/** The conversation, what is happening to it, and the two things that can be done to it. */
interface Agent {
  /** Where the agent is, or `null` before the first look has come back. */
  status: AgentStatus | null;
  /** Every finished turn, oldest first. */
  turns: Said[];
  /** What the run in flight has said so far. Empty while it has said nothing. */
  saying: string;
  /** Which stage the run in flight has reached, or `null` while none has been reported. */
  stage: Stage | null;
  /** Whether a run is in flight. */
  running: boolean;
  /** The identifier of the last failure, or `null` when the last run did not fail. */
  failure: string | null;
  /** Asks the agent something. Does nothing while a run is in flight. */
  ask: (asked: string) => void;
  /** Asks the run in flight to stop. */
  stop: () => void;
}

/** Holds one conversation with the agent, and keeps what is known about it current. */
export function useAgent(): Agent {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [turns, setTurns] = useState<Said[]>([]);
  const [saying, setSaying] = useState("");
  const [stage, setStage] = useState<Stage | null>(null);
  const [running, setRunning] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  // The run in flight, so that `stop` names the right one. A ref rather than state: nothing
  // draws it, and a callback that closed over a stale one would cancel the wrong run.
  const inFlight = useRef<string | null>(null);
  // What the run has said so far, for the same reason: the finished turn is assembled when a
  // terminal event arrives, and by then the state setter has not necessarily run.
  const collected = useRef("");
  const nextRun = useRef(1);

  const look = useCallback(() => {
    void agentStatus().then(setStatus);
  }, []);

  useEffect(look, [look]);

  /** Ends the run in flight, keeping whatever it managed to say. */
  const settle = useCallback((cut?: true) => {
    const said = collected.current;
    collected.current = "";
    inFlight.current = null;
    setRunning(false);
    setStage(null);
    setSaying("");
    if (said) {
      setTurns((held) => [...held, { role: "agent", content: said, ...(cut ? { cut } : {}) }]);
    }
  }, []);

  const receive = useCallback(
    (event: AgentEvent) => {
      switch (event.event) {
        case "started":
          setStage(null);
          break;
        case "progress":
          setStage(event.stage);
          break;
        case "token":
          collected.current += event.text;
          setSaying(collected.current);
          break;
        case "proposal":
          collected.current += `${event.title}\n\n${event.body}`;
          setSaying(collected.current);
          break;
        case "completed":
          settle();
          look();
          break;
        case "cancelled":
          settle(true);
          look();
          break;
        case "failed":
          setFailure(event.code);
          settle();
          look();
          break;
      }
    },
    [look, settle],
  );

  const ask = useCallback(
    (asked: string) => {
      if (running || !asked.trim()) {
        return;
      }

      const id = String(nextRun.current);
      nextRun.current += 1;
      inFlight.current = id;
      collected.current = "";

      const said: Said = { role: "person", content: asked.trim() };
      const conversation = [...turns, said];

      setTurns(conversation);
      setSaying("");
      setStage(null);
      setFailure(null);
      setRunning(true);

      void askAgent({ id, intent: "chat", messages: conversation }, receive).then((refused) => {
        if (refused !== null) {
          // Refused before anything was started, so no terminal event is coming: nothing on
          // the channel will end this run, and this is the only place that can.
          setFailure(refused);
          settle();
          look();
        }
      });
    },
    [look, receive, running, settle, turns],
  );

  const stop = useCallback(() => {
    const id = inFlight.current;
    if (id !== null) {
      void cancelAgent(id);
    }
  }, []);

  return { status, turns, saying, stage, running, failure, ask, stop };
}
