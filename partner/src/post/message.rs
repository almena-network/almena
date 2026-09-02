//! What travels inside a sealed envelope.
//!
//! The message a counterparty composed, in the shape DIDComm defines: a name, a kind, who it is
//! from and who it is for by their peer identifiers, and a body the kind defines. Nothing here is
//! this project's invention; what is this project's is which kinds it writes and reads.

/// One message, as it travels inside the envelope.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// What this message is called, which says which thing it belongs to.
    pub id: String,
    /// What kind of message it is.
    #[serde(rename = "type")]
    pub kind: String,
    /// Who it is from: their peer identifier for this relationship.
    pub from: String,
    /// Who it is for.
    pub to: Vec<String>,
    /// What it says, which is the kind's to define.
    pub body: serde_json::Value,
}

/// What a first message on an introduction is called.
///
/// It carries nothing. What it is *for* is being the message whose seal names the sender, which is
/// how the end that showed a code learns who took it up. The same string the holder's app writes.
pub const HELLO: &str = "almena/hello";

impl Message {
    /// A message from this end to that one.
    #[must_use]
    pub fn new(id: &str, kind: &str, from: &str, to: &str, body: serde_json::Value) -> Self {
        Self {
            id: id.to_owned(),
            kind: kind.to_owned(),
            from: from.to_owned(),
            to: vec![to.to_owned()],
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HELLO, Message};
    use almena_sdk::errand::{Came, decided, offering, revoked};
    use almena_time::Epoch;

    #[test]
    fn the_bodies_this_sends_are_the_fixtures_the_holder_s_app_reads() {
        // **Copied out of the holder's app's own tests**, which hold these strings as what the
        // other side composed. The two repositories share no code; this is what holds them
        // together.
        assert_eq!(
            offering("a~b~", Came::Unasked, Epoch::new(1_000), None).to_string(),
            r#"{"came":"unasked","credential":"a~b~","renews":null,"until":1000}"#
        );
        assert_eq!(
            revoked(
                "one",
                &almena_credential::Status::Revocable {
                    list: "did:almena:dev:zAList".to_owned(),
                    index: 4242,
                },
                Epoch::new(1_100),
            )
            .expect("a notice")
            .to_string(),
            r#"{"at":1100,"credential":"one","index":4242,"list":"did:almena:dev:zAList"}"#
        );
        assert_eq!(
            decided("one", true).to_string(),
            r#"{"credential":"one","taken":true}"#
        );
    }

    #[test]
    fn a_message_is_written_in_the_shape_the_holder_s_app_reads() {
        let written = serde_json::to_value(Message::new(
            "one",
            HELLO,
            "did:peer:2.a",
            "did:peer:2.b",
            serde_json::json!({}),
        ))
        .expect("json");
        assert_eq!(written["type"], "almena/hello");
        assert_eq!(written["to"][0], "did:peer:2.b");
        assert!(written.get("created_time").is_none(), "nothing invented");
    }
}
