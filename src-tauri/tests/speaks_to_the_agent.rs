//! Drives the staged agent over real pipes, and checks that the two halves of the contract
//! actually meet.
//!
//! Everything else about the protocol is checked on one side or the other: the crate's own
//! tests round-trip every message, the golden corpus pins the exact bytes in both
//! repositories, and the agent's `verify_build.py` drives its binary through the contract from
//! Python. What none of them can answer is whether **this** program and **that** one
//! understand each other over two pipes — which is the one question a person actually has.
//!
//! # Why it is ignored by default
//!
//! It needs an artifact from another repository: the agent, built and staged by
//! `task agent:stage`. A checkout of this repository alone has none, and a test that quietly
//! passed when it could not find one would be worse than no test — it would be green on every
//! machine that had nothing to test with.
//!
//! So it is `#[ignore]`d and run on purpose:
//!
//! ```text
//! task agent:build && task agent:stage && task test:agent
//! ```
//!
//! It reaches no model. Every exchange below is one the agent answers out of its own contract,
//! which is what keeps it a test of the wire rather than of somebody's GPU.

// Every function here is part of a test, the helpers on the struct included, which clippy
// cannot tell because they carry no `#[test]` of their own. A child that will not start, or
// will not speak this contract, is a failing test — which is exactly what a panic here is.
#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

use almena_agent_protocol::framing;
use almena_agent_protocol::message::{Command as Ask, CommandBody, Event, EventBody, Params};
use almena_agent_protocol::vocabulary::{ErrorCode, Intent, Role, Turn};

/// Where `task agent:stage` puts the agent.
fn staged() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/almena-agent")
        .join(format!("almena-agent{}", std::env::consts::EXE_SUFFIX))
}

/// One agent, started the way the application starts it.
struct Agent {
    child: Child,
    reading: BufReader<ChildStdout>,
}

impl Agent {
    /// Starts the staged agent, with its environment cleared as the supervisor clears it.
    fn start() -> Self {
        let binary = staged();
        assert!(
            binary.is_file(),
            "no agent staged at {}: run `task agent:build && task agent:stage`",
            binary.display()
        );

        let mut child = Command::new(&binary)
            .env_clear()
            .envs(std::env::var("TMPDIR").map(|held| ("TMPDIR".to_owned(), held)))
            .envs(std::env::var("SYSTEMROOT").map(|held| ("SYSTEMROOT".to_owned(), held)))
            // Somewhere real and empty, for the same reason the supervisor gives it one: the
            // agent's own default for this is a relative path.
            .env("ALMENA_AGENT_RESOURCES", std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the staged agent starts");

        // Nothing reads stderr in this test, and a pipe nobody drains fills and blocks the
        // writer. Drained and discarded, which is what a test wants and not what the
        // application does — it forwards every line into its own log.
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || for _ in BufReader::new(stderr).lines() {});
        }

        let reading = BufReader::new(child.stdout.take().expect("the agent has a stdout"));
        Self { child, reading }
    }

    /// Writes one command.
    fn ask(&mut self, body: CommandBody) {
        let frame = framing::encode(&Ask::new(body)).expect("a command encodes");
        let stdin = self.child.stdin.as_mut().expect("the agent has a stdin");
        stdin.write_all(&frame).expect("the command is written");
        stdin.flush().expect("the command is flushed");
    }

    /// Reads the next event.
    fn next(&mut self) -> Event {
        let payload = framing::read(&mut self.reading)
            .expect("the agent's output is framed")
            .expect("the agent said something");
        framing::decode_event(&payload).expect("the agent speaks this contract")
    }

    /// Reads events until one satisfies `wanted`, collecting everything on the way.
    fn until(&mut self, wanted: impl Fn(&EventBody) -> bool) -> Vec<EventBody> {
        let mut seen = Vec::new();
        loop {
            let event = self.next();
            let done = wanted(&event.body);
            seen.push(event.body);
            if done {
                return seen;
            }
        }
    }

    /// Ends the agent the way the application ends it: by closing its input.
    fn stop(mut self) {
        drop(self.child.stdin.take());
        let ended = self.child.wait().expect("the agent is waited on");
        assert!(ended.success(), "the agent left cleanly: {ended}");
    }
}

/// One ordinary question.
fn question(id: &str) -> CommandBody {
    CommandBody::Run {
        id: id.to_owned(),
        intent: Intent::Chat,
        params: Params {
            messages: vec![Turn {
                role: Role::Person,
                content: "what is this".to_owned(),
            }],
            resources: Vec::new(),
            tools: Vec::new(),
        },
    }
}

#[test]
#[ignore = "needs the agent staged: task agent:build && task agent:stage"]
fn the_agent_announces_itself_before_it_is_asked_anything() {
    let mut agent = Agent::start();

    let first = agent.next();
    assert!(
        matches!(first.body, EventBody::Ready { .. }),
        "the first thing it says is ready, unprompted: {:?}",
        first.body
    );

    agent.stop();
}

#[test]
#[ignore = "needs the agent staged: task agent:build && task agent:stage"]
fn a_run_is_answered_with_a_started_and_exactly_one_terminal() {
    let mut agent = Agent::start();
    agent.until(|body| matches!(body, EventBody::Ready { .. }));

    agent.ask(question("1"));
    // No model is serving on a build machine, so this ends in a failure — which is the point:
    // the failure has to arrive as a *terminal event of this contract*, not as a hang or a
    // traceback down stderr.
    let seen = agent.until(|body| {
        matches!(
            body,
            EventBody::Completed { .. } | EventBody::Failed { .. } | EventBody::Cancelled { .. }
        )
    });

    assert!(
        matches!(seen.first(), Some(EventBody::Started { .. })),
        "a started comes first, always: {seen:?}"
    );

    let terminals = seen
        .iter()
        .filter(|body| {
            matches!(
                body,
                EventBody::Completed { .. }
                    | EventBody::Failed { .. }
                    | EventBody::Cancelled { .. }
            )
        })
        .count();
    assert_eq!(terminals, 1, "exactly one terminal event: {seen:?}");

    agent.stop();
}

#[test]
#[ignore = "needs the agent staged: task agent:build && task agent:stage"]
fn a_resource_the_agent_does_not_hold_is_a_failure_this_side_can_read() {
    let mut agent = Agent::start();
    agent.until(|body| matches!(body, EventBody::Ready { .. }));

    agent.ask(CommandBody::Run {
        id: "1".to_owned(),
        intent: Intent::Chat,
        params: Params {
            resources: vec!["nothing-is-held-under-this-name.txt".to_owned()],
            ..Params::default()
        },
    });

    let seen = agent.until(|body| matches!(body, EventBody::Failed { .. }));
    let Some(EventBody::Failed { code, id, .. }) = seen.last() else {
        panic!("the run failed: {seen:?}");
    };

    assert_eq!(*code, ErrorCode::RESOURCE_UNKNOWN);
    assert_eq!(id.as_deref(), Some("1"), "attributed to the run that asked");

    agent.stop();
}

#[test]
#[ignore = "needs the agent staged: task agent:build && task agent:stage"]
fn a_second_run_while_one_is_in_flight_is_refused_and_names_the_new_one() {
    let mut agent = Agent::start();
    agent.until(|body| matches!(body, EventBody::Ready { .. }));

    agent.ask(question("first"));
    agent.ask(question("second"));

    let seen = agent.until(|body| {
        matches!(body, EventBody::Failed { code, .. } if *code == ErrorCode::RUN_ALREADY_IN_FLIGHT)
    });

    let Some(EventBody::Failed { id, .. }) = seen.last() else {
        panic!("the second run was refused: {seen:?}");
    };
    // The identifier of the **new** run, so a caller knows which of its two was refused.
    assert_eq!(id.as_deref(), Some("second"));

    agent.stop();
}

#[test]
#[ignore = "needs the agent staged: task agent:build && task agent:stage"]
fn a_frame_this_build_would_not_read_is_refused_by_the_agent_too() {
    let mut agent = Agent::start();
    agent.until(|body| matches!(body, EventBody::Ready { .. }));

    // A well-formed prefix over a payload that is not MessagePack. The framing has to carry it
    // as far as the decoder, which is the half a hand-written line would have skipped.
    let payload = b"not messagepack at all";
    let mut frame = u32::try_from(payload.len())
        .expect("a small frame")
        .to_be_bytes()
        .to_vec();
    frame.extend_from_slice(payload);

    let stdin = agent.child.stdin.as_mut().expect("the agent has a stdin");
    stdin.write_all(&frame).expect("the frame is written");
    stdin.flush().expect("the frame is flushed");

    let seen = agent.until(|body| matches!(body, EventBody::Failed { .. }));
    let Some(EventBody::Failed { code, .. }) = seen.last() else {
        panic!("the frame was refused: {seen:?}");
    };
    assert_eq!(*code, ErrorCode::MESSAGE_NOT_DECODABLE);

    agent.stop();
}

#[test]
#[ignore = "needs the agent staged: task agent:build && task agent:stage"]
fn closing_the_input_is_how_the_agent_is_asked_to_go() {
    // The whole of this application's shutdown, and the reason no orphan is possible: nothing
    // signals the child but the pipe, so a desktop that is killed outright takes the agent
    // with it whether or not anything got to run.
    let mut agent = Agent::start();
    agent.until(|body| matches!(body, EventBody::Ready { .. }));

    drop(agent.child.stdin.take());

    let since = std::time::Instant::now();
    loop {
        match agent.child.try_wait().expect("the agent is waited on") {
            Some(status) => {
                assert!(status.success(), "it left cleanly: {status}");
                return;
            }
            None => assert!(
                since.elapsed() < Duration::from_secs(10),
                "the agent went when its input closed"
            ),
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
