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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::{error, info};

use crate::node::Node;

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
    under: Option<almena_tls::Accepting>,
) -> Option<Listening> {
    let runtime = node.runtime()?;
    let stopping = Arc::new(AtomicBool::new(false));
    let address = address.to_owned();
    let watch = Arc::clone(&stopping);

    // The clock is captured once: an epoch is hours since this network's own beginning, and the
    // node is the only thing that knows when that was.
    let began = node.now();
    let clock = move || began.unwrap_or(almena_node::Epoch::GENESIS);

    let answering = runtime.spawn(async move {
        answering(&address, serving, clock, &watch, under).await;
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
    under: Option<almena_tls::Accepting>,
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
    // Said rather than assumed either way. A node answering in the clear is right on the machine
    // it runs on and wrong anywhere else, and whoever is reading these records is the person who
    // can tell which this is.
    info!(
        "interface_serving address={address} under={}",
        if under.is_some() {
            "a_certificate"
        } else {
            "nothing"
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
            // Two ways in and one node behind them: what is served is decided in one place, and
            // this only chooses what the bytes travelled inside.
            let ended = match under {
                Some(accepting) => match accepting.accept(io).await {
                    Ok(wrapped) => serving.connection(wrapped, clock).await,
                    Err(why) => {
                        // Somebody who could not agree on a certificate. Common, and no reason to
                        // stop answering everybody else.
                        info!("connection_not_secured reason={why}");
                        return;
                    }
                },
                None => serving.connection(io, clock).await,
            };
            if let Err(why) = ended {
                info!("connection_ended reason={why}");
            }
        });
    }
}
