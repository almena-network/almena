//! Where the partner starts, and nothing else: the command line, parsed, and one errand run.
//!
//! Everything it does lives in the library beside this file, so that a test walks the same
//! errands against a node it holds in the same process. What is printed on standard output is
//! what a shell would take — an identifier, a link, an outcome — and everything else is a record
//! on standard error.

use std::collections::BTreeMap;
use std::path::PathBuf;

use almena_format::identifier::{Did, Name};
use almena_partner::commands::{Partner, collect, issue, keys, relate, revoke, show};
use almena_partner::directory::Directory;
use almena_partner::failed::Failed;
use almena_partner::node::Node;
use almena_partner::verifying::{self, Asking, Under};
use almena_sdk::errand::Came;
use clap::{Parser, Subcommand};

/// The reference issuer and verifier, run against a node.
#[derive(Debug, Parser)]
#[command(name = "almena-partner", version, about, long_about = None)]
struct Arguments {
    /// Where this partner keeps its keys and its memory.
    #[arg(long, global = true, default_value = "partner")]
    directory: PathBuf,

    /// The node every read goes through and every act is handed to: `https://host:port`.
    #[arg(long, global = true, default_value = "https://127.0.0.1:8790")]
    node: String,

    /// That node's identity, as the zone publishes it: `12D3KooW…`.
    #[arg(long, global = true)]
    peer: Option<String>,

    #[command(subcommand)]
    errand: Errand,
}

/// What the partner is asked to do.
#[derive(Debug, Subcommand)]
enum Errand {
    /// Make or load the keys — the account's and the issuer element's — and put the partner's
    /// own account on the record.
    Keys,
    /// Print what the directory holds, to copy from: identifiers, public keys, relationships.
    Show,
    /// Take up a relationship with a holder who showed a code.
    Relate {
        /// The `almena://meet?who=…` link, or the `did:peer:2…` it carries.
        #[arg(long)]
        link: String,
        /// Where this partner's end of the relationship is delivered to: `host:port 12D3KooW…`
        /// (the address and the identity of the node running the mediator, quoted), or `host:port`
        /// for a mediator on the node given by `--node`; that node itself by default.
        #[arg(long)]
        mediator: Vec<String>,
    },
    /// Sign a credential against a template and offer it to a holder.
    Issue {
        /// The far end of the relationship, by its `did:peer:2…`.
        #[arg(long)]
        to: String,
        /// The issuer element the credential is issued by; remembered, so the one named last
        /// time is the default.
        #[arg(long)]
        issuer: Option<String>,
        /// A file holding the P-256 secret the element emits with, as hexadecimal;
        /// `issuance.key` in the directory by default.
        #[arg(long)]
        issuance_key: Option<PathBuf>,
        /// A file holding the element's own Ed25519 secret, which signs status list acts;
        /// `element.key` in the directory by default.
        #[arg(long)]
        issuer_key: Option<PathBuf>,
        /// The template version, by the hash of the act that published it.
        #[arg(long)]
        template: String,
        /// One attribute, `name=value`; the value is JSON where it is JSON and text otherwise.
        #[arg(long)]
        attribute: Vec<String>,
        /// A JSON file of attributes, an object of name to value.
        #[arg(long)]
        attributes: Option<PathBuf>,
        /// The epoch the credential stops being valid in.
        #[arg(long)]
        expires: u64,
        /// Whether it can be revoked, which puts its bit in a list.
        #[arg(long)]
        revocable: bool,
        /// The credential's own identifier; drawn at random when not given.
        #[arg(long)]
        credential: Option<String>,
        /// The holder asked for it.
        #[arg(long)]
        asked: bool,
        /// It renews that credential.
        #[arg(long)]
        renews: Option<String>,
    },
    /// Bring in the post: what holders decided, and anything else.
    Collect,
    /// Revoke a credential issued here, and tell its holder.
    Revoke {
        /// The credential, by its identifier.
        #[arg(long)]
        credential: String,
        /// A file holding the element's own Ed25519 secret, which signs status list acts;
        /// `element.key` in the directory by default.
        #[arg(long)]
        issuer_key: Option<PathBuf>,
    },
    /// Serve a request, print the link, and judge the presentation that answers it.
    Verify {
        /// The verifier, which is who the presentation is for.
        #[arg(long)]
        verifier: String,
        /// The request template version, by hash.
        #[arg(long)]
        template: String,
        /// A credential shape taken, by template version hash; none means any.
        #[arg(long)]
        accepts: Vec<String>,
        /// One thing asked for, `attribute:purpose`.
        #[arg(long)]
        ask: Vec<String>,
        /// Where to listen, `host:port`.
        #[arg(long, default_value = "127.0.0.1:8899")]
        serve: String,
        /// The path the wallet talks to.
        #[arg(long, default_value = "/present")]
        path: String,
        /// Serve under the partner's own key, as a node does.
        #[arg(long)]
        own_key: bool,
        /// An operator's certificate, PEM.
        #[arg(long, requires = "private_key")]
        certificate: Option<PathBuf>,
        /// Its key, PEM.
        #[arg(long, requires = "certificate")]
        private_key: Option<PathBuf>,
        /// Refuse a credential that says it cannot be revoked.
        #[arg(long)]
        require_revocable: bool,
    },
}

fn main() {
    almena_partner::records::install();
    let arguments = Arguments::parse();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            log::error!("partner_failed reason=no_runtime");
            std::process::exit(1);
        }
    };
    if let Err(why) = runtime.block_on(run(arguments)) {
        log::error!("partner_failed reason={why}");
        std::process::exit(1);
    }
}

/// The errand, run.
async fn run(arguments: Arguments) -> Result<(), Failed> {
    let directory = Directory::at(&arguments.directory)?;
    if matches!(arguments.errand, Errand::Show) {
        // Read off the disk alone, so it needs no node and no peer.
        for line in show::run(&directory)?.lines() {
            println!("{line}");
        }
        return Ok(());
    }
    let peer = arguments
        .peer
        .as_deref()
        .ok_or_else(|| Failed::new("partner_no_peer_given"))?;
    let partner = Partner {
        directory,
        node: Node::at(&arguments.node, peer)?,
    };
    match arguments.errand {
        Errand::Keys => {
            let made = keys::run(&partner).await?;
            println!("account={}", made.account);
            println!("device={}", made.device);
            println!("element={}", made.element);
            println!("issuance={}", made.issuance);
        }
        Errand::Show => return Err(Failed::new("partner_no_such_errand")),
        Errand::Relate { link, mediator } => {
            let related = relate::run(&partner, &link, mediator).await?;
            println!("mine={}", related.mine);
            println!("theirs={}", related.theirs);
        }
        Errand::Issue { .. } => issuing(&partner, arguments.errand).await?,
        Errand::Collect => collecting(&partner).await?,
        Errand::Revoke {
            credential,
            issuer_key,
        } => {
            let issuer_key = partner.directory.element_secret(issuer_key.as_deref())?;
            let revoked = revoke::run(&partner, &credential, issuer_key).await?;
            println!(
                "list={} index={} told={}",
                revoked.list, revoked.index, revoked.told
            );
        }
        Errand::Verify { .. } => verifying_(&partner, arguments.errand).await?,
    }
    Ok(())
}

/// `collect`, printed one line per message.
async fn collecting(partner: &Partner) -> Result<(), Failed> {
    for arrived in collect::run(partner).await? {
        match arrived.said {
            Some(said) => println!(
                "{} {} from={} body={}",
                arrived.called, said.kind, said.from, said.body
            ),
            None => println!(
                "{} set_aside={}",
                arrived.called,
                arrived.set_aside.unwrap_or_default()
            ),
        }
    }
    Ok(())
}

/// `issue`, with its arguments read into what the errand takes.
async fn issuing(partner: &Partner, errand: Errand) -> Result<(), Failed> {
    let Errand::Issue {
        to,
        issuer,
        issuance_key,
        issuer_key,
        template,
        attribute,
        attributes,
        expires,
        revocable,
        credential,
        asked,
        renews,
    } = errand
    else {
        return Err(Failed::new("partner_no_such_errand"));
    };
    let came = match (asked, &renews) {
        (_, Some(_)) => Came::Renewal,
        (true, None) => Came::Asked,
        (false, None) => Came::Unasked,
    };
    let offered = issue::run(
        partner,
        &issue::Asked {
            to,
            issuer: issue::issuer_of(&partner.directory, issuer.as_deref())?,
            issuance_key: partner.directory.issuance_secret(issuance_key.as_deref())?,
            issuer_key: partner.directory.element_secret(issuer_key.as_deref())?,
            template: named(&template, "issue_not_a_name", "template")?,
            attributes: attributes_of(&attribute, attributes.as_deref())?,
            expires,
            revocable,
            identifier: credential,
            came,
            renews,
        },
    )
    .await?;
    println!("credential={}", offered.identifier);
    Ok(())
}

/// The attributes an operator gave: the file first, then each `name=value`, the later winning.
fn attributes_of(
    attribute: &[String],
    attributes: Option<&std::path::Path>,
) -> Result<BTreeMap<Name, serde_json::Value>, Failed> {
    let mut held = match attributes {
        Some(path) => issue::attributes_in(
            &std::fs::read_to_string(path)
                .map_err(|_| Failed::new("issue_attributes_file_unreadable"))?,
        )?,
        None => BTreeMap::new(),
    };
    for one in attribute {
        let (name, value) = issue::attribute(one)?;
        held.insert(name, value);
    }
    Ok(held)
}

/// A name off the command line, or the word for its not being one.
fn named(text: &str, word: &str, key: &str) -> Result<Name, Failed> {
    Name::parse(text).map_err(|_| Failed::with(word, key, text))
}

/// `verify`, with its arguments read into what the errand takes.
async fn verifying_(partner: &Partner, errand: Errand) -> Result<(), Failed> {
    let Errand::Verify {
        verifier,
        template,
        accepts,
        ask,
        serve,
        path,
        own_key,
        certificate,
        private_key,
        require_revocable,
    } = errand
    else {
        return Err(Failed::new("partner_no_such_errand"));
    };
    let under = match (own_key, certificate, private_key) {
        (_, Some(certificate), Some(key)) => Under::Certificate { certificate, key },
        (true, _, _) => Under::OwnKey,
        _ => Under::Nothing,
    };
    let asking = Asking {
        verifier: Did::parse(&verifier)
            .map_err(|_| Failed::with("verify_not_a_did", "verifier", &verifier))?,
        template: named(&template, "verify_not_a_name", "template")?,
        accepts: accepts
            .iter()
            .map(|one| named(one, "verify_not_a_name", "accepts"))
            .collect::<Result<_, _>>()?,
        asks: ask
            .iter()
            .map(|one| verifying::ask(one))
            .collect::<Result<_, _>>()?,
        serve,
        path,
        under,
        require_revocable,
    };
    let started = verifying::start(partner, &asking).await?;
    println!("{}", started.link);
    let judged = started.judged().await?;
    println!(
        "outcome={} why={}",
        judged.outcome.word(),
        judged.why.unwrap_or_default()
    );
    Ok(())
}
