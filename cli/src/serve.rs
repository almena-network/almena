//! Answering, while this face carries on drawing.
//!
//! **Serving is not instead of anything.** A node that had to stop being drawn in order to answer
//! questions would be one of the two ways of running a node able to do something the other cannot,
//! which is exactly what the two-face arrangement refuses. So the interface runs on a thread of
//! its own and the node stays where it was.
//!
//! Nothing here decides anything about an answer: it binds a socket and hands connections over.
//! **It does not keep the clock** — that belongs to being on a network, not to answering, and a
//! node whose epochs only closed while its interface was up would leave gaps that mean *nothing
//! happened* and *I was not here* at the same time.
//!
//! Every connection is wrapped: under the node's own key unless an operator named a certificate,
//! and never in the clear. Which of the two is in the records as `under=own_key` or
//! `under=a_certificate`, because whoever dials this node pins a key and the person running it is
//! the one who can say which.
//!
//! Serving is also said in the record: once the socket is bound the node announces `Interface` on
//! its own chain, the way a mediator announces that it holds post, because what the network has is
//! counted from what its nodes say and an interface nobody announced is one nobody can count. The
//! records carry it as `interface_offered written=now` or `written=before`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::{error, info};

use crate::node::Node;

/// What the interface is served under, for the records to say.
///
/// **Said rather than assumed either way.** Whoever dials this node pins the key the zone told
/// them, so which of the two it is matters to them: a node under its own key is one the zone's
/// `peer=` vouches for on its own, and one under an operator's certificate is one whose name
/// somebody else's authority vouches for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Under {
    /// A certificate made from the node's own key and signed by it. The ordinary case.
    OwnKey,
    /// A certificate and key an operator named as two PEM files.
    ACertificate,
}

impl Under {
    /// The word the records carry.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::OwnKey => "own_key",
            Self::ACertificate => "a_certificate",
        }
    }
}

/// The interface, running.
///
/// Holding one means it is up. Dropping it does not stop it — [`Listening::stop`] does, and says
/// so in the records.
pub struct Listening {
    stopping: Arc<AtomicBool>,
    answering: Option<tokio::task::JoinHandle<()>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl Listening {
    /// Takes the interface down, saying so.
    pub fn stop(mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(answering) = self.answering.take() {
            // Waited for rather than abandoned, so that the address is free by the time this
            // returns and somebody starting the interface again does not meet their own socket.
            // A task that has already ended is the normal way this goes when something else took
            // the process down first.
            let _ = self.runtime.block_on(answering);
        }
        info!("interface_stopped");
    }
}

/// Start answering on `address`, on the work a node on a network already has.
///
/// It runs beside whatever this face is doing — drawing, or waiting for the operating system —
/// rather than in place of it. There is no second runtime: a node has one, and answering questions
/// is one of the things it does with it.
#[must_use]
pub fn start(
    address: &str,
    serving: almena_serve::Serving,
    node: &Node,
    under: almena_tls::Accepting,
    how: Under,
) -> Option<Listening> {
    let runtime = node.runtime()?;
    let stopping = Arc::new(AtomicBool::new(false));
    let address = address.to_owned();
    let watch = Arc::clone(&stopping);

    // **The node's own clock, live, and not a reading of it.** Every act that arrives over this
    // socket is placed at the epoch the clock says, and an interface that read the clock once at
    // start would place everything a week's uptime brought in at the hour it opened. The node is
    // the only thing that knows when its network began and what is added to the wall's count.
    let clock = node.clock()?;

    let answering = runtime.spawn(async move {
        answering(&address, serving, clock, &watch, (under, how)).await;
    });

    Some(Listening {
        stopping,
        answering: Some(answering),
        runtime: Arc::clone(runtime),
    })
}

/// Accept connections until told to stop, keeping the epochs closed while it does.
async fn answering<C>(
    address: &str,
    serving: almena_serve::Serving,
    clock: C,
    stopping: &AtomicBool,
    under: (almena_tls::Accepting, Under),
) where
    C: Fn() -> almena_node::Epoch + Clone + Send + Sync + 'static,
{
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(why) => {
            error!("interface_not_served address={address} reason={why}");
            return;
        }
    };
    // Said rather than assumed either way: whoever dials this node pins a key, and these records
    // are where the person running it reads which key that is.
    let (under, how) = under;
    info!("interface_serving address={address} under={}", how.word());

    // **Said in the record, where it is counted, once the door is open.** What a network has is
    // counted from what its nodes say they offer, and a node answering on an interface it never
    // announced would be a service the network could not see. Said after the bind and not before,
    // so that nothing is claimed for a socket that was refused; and said once — the core writes
    // nothing when the record already says it.
    let said = serving
        .node()
        .write()
        .await
        .also_offering(almena_node::Capability::Interface, clock());
    info!(
        "interface_offered {}",
        if said {
            "written=now"
        } else {
            "written=before"
        }
    );

    while !stopping.load(Ordering::Relaxed) {
        let accepted =
            tokio::time::timeout(std::time::Duration::from_millis(200), listener.accept()).await;

        let Ok(Ok((io, _))) = accepted else {
            // Either nothing arrived in that moment, or the connection failed on the way in.
            // Neither is a reason to stop answering everybody else.
            continue;
        };

        // A node that is full closes the connection rather than queueing it, and the number it is
        // keeping to is published where anybody can read it.
        let Some(room) = serving.room() else {
            continue;
        };

        let serving = serving.clone();
        let clock = clock.clone();
        let under = under.clone();
        tokio::spawn(async move {
            let _room = room;
            // One node behind every connection: what is served is decided in one place, and this
            // only wraps what the bytes travel inside.
            let wrapped = match under.accept(io).await {
                Ok(wrapped) => wrapped,
                Err(why) => {
                    // Somebody who could not agree on a certificate. Common, and no reason to
                    // stop answering everybody else.
                    info!("connection_not_secured reason={why}");
                    return;
                }
            };
            if let Err(why) = serving.connection(wrapped, clock).await {
                info!("connection_ended reason={why}");
            }
        });
    }
}
