//! Acts the holder's app composed, admitted here.
//!
//! **The two implementations meeting where they really meet: the bytes.** `client` writes the
//! format a second time — its own CBOR, its own naming, its own signing — in a repository that
//! shares no code with this one. The golden corpus already holds the two to the same bytes for one
//! act. This holds them to something stronger and further along: bytes that program actually
//! produced, put through the admission every act goes through, ending in the account this node
//! computes from them.
//!
//! What it catches that the corpus cannot: an act that is written correctly and **refused** —
//! signed with the wrong key, naming the wrong predecessor, carrying a field in the wrong place,
//! or claiming a kind this build applies differently. The corpus checks the writing; this checks
//! the whole conversation.
//!
//! The constants below were produced by `client`'s `holder::act` from a fixed control key
//! (`[7; 32]`) and a fixed device key (`[9; 32]`) at the genesis epoch, and the twin of this file
//! on that side pins the same hex. **Changing one side without the other is what this fails on.**

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::Operation;
use almena_store::chain::{Admitted, Answer, Holder, Objects, Refused, State};
use almena_store::guardian::{self, SALT_WIDTH, commit};
use almena_time::{Epoch, Epochs};

/// An account, created by the app with the words that govern it.
const ACCOUNT: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02f603010401050006a1015820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c584097b610fb40f5e5e6889430e257a55ead763f080b7adbe7eaf6027eb36ff327a646b7c9a566fc3a019aeeed408812fc81586fbfd65cbb2918e662d0affd960b0d";

/// A device the words then ask for — which, being the words alone, waits.
const ADD_DEVICE: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d66314546636f657a65664e3170595077616d664654516d6d67385735636e59616f614a43534d5a563353785203020401050006a1015821027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a3429617865078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c5840ab0e8d1188ee9e880d2c7e81ad41909f8780e8ea735367a76faf59892e24c5f3da176e656adeced9e6f6ff5576dd654c587d3e12d57fb6294e0bb9804a1a050b";

/// The name the app said that account has — recomputed here from the act rather than trusted.
const DID: &str = "did:almena:dev:zQmNRTfFmHVMgAvrRcnW95iV3DULN5AWn9izPteoR8gMorn";

/// The device key the app asked for, compressed, as the account will carry it.
const DEVICE: &str = "027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a3429617865";

/// An act read back off the bytes the app produced.
fn act(written: &str) -> Operation {
    let bytes: Vec<u8> = (0..written.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&written[at..at + 2], 16).expect("the corpus is hexadecimal"))
        .collect();
    let value = almena_format::cbor::read(&bytes).expect("the app writes canonical bytes");
    almena_format::operation::read(&value).expect("and they read back as an act")
}

#[test]
fn an_account_the_app_made_is_admitted_and_names_itself() {
    // The whole promise, checked from the other side: whoever holds a creation recomputes the
    // identifier from its own bytes and finds it matches — no node asked, and nothing believed
    // because the app said so.
    let account = act(ACCOUNT);
    assert!(account.names_itself());
    assert_eq!(account.object.to_string(), DID);

    let mut objects = Objects::new();
    assert_eq!(
        objects.admit(&account, Epoch::GENESIS),
        Ok(Admitted::Extended),
        "and the node takes it"
    );
}

#[test]
fn a_device_the_words_asked_for_waits_and_then_lands() {
    // The two sides agreeing about §11.12 without ever having spoken: the app composed a request
    // signed by the words alone, and this node — which decided the rule — holds it back for the
    // window and then applies it. A disagreement here would be a person's laptop appearing
    // instantly on one node and in three days on another.
    let mut objects = Objects::new();
    objects
        .admit(&act(ACCOUNT), Epoch::GENESIS)
        .expect("the account");

    let add = act(ADD_DEVICE);
    assert_eq!(
        objects.admit(&add, Epoch::GENESIS),
        Ok(Admitted::Extended),
        "the asking enters the record at once"
    );

    let device: Vec<u8> = (0..DEVICE.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&DEVICE[at..at + 2], 16).expect("hexadecimal"))
        .collect();
    let name = add.object.name().clone();

    let holder = |at: Epoch| match objects.resolve(&name) {
        Answer::Here(State::Holder(holder)) => holder.come_due(at),
        other => panic!("{other:?}"),
    };
    assert!(
        !holder(Epoch::GENESIS).devices.contains(&device),
        "not a device while the wait runs"
    );
    let due = Epoch::GENESIS.plus(Epochs(72)).expect("no overflow");
    assert!(
        holder(due).devices.contains(&device),
        "and one once it is out"
    );
}

/// The same request the app composed, with one odd field added.
///
/// **Critical, and no build has a meaning for it** (`SPECS.md §4.8`, rule 4). It is here so that
/// the two vocabularies — the node's `holder_vocabulary` and the app's own `speaks` — are held to
/// each other by bytes rather than by two hand-kept lists in two repositories. A drift between
/// them is not a disagreement about a screen: it is an account one side goes on resolving while
/// the other declares it opaque for ever.
const UNREADABLE: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d66314546636f657a65664e3170595077616d664654516d6d67385735636e59616f614a43534d5a563353785203020401050006a2015821027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a34296178650701078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c58409aa60d1951c48b73a2d771450fbd1b89ce0a20691bea82f7d903d88d9a6a81a999c047614e1d65b6f8440142e05ff8234d3d03895501ffdb9b76f9991971b901";

#[test]
fn what_neither_side_can_read_is_kept_and_leaves_the_account_unresolvable() {
    // Not refused: replication does not require understanding, so the act is stored and passed on
    // like any other. What must not happen is this node going on answering for the account as
    // though the act had not been written — *irresoluble, nunca obsoleto*.
    let mut objects = Objects::new();
    objects
        .admit(&act(ACCOUNT), Epoch::GENESIS)
        .expect("the account");

    assert_eq!(
        objects.admit(&act(UNREADABLE), Epoch::GENESIS),
        Ok(Admitted::Extended),
        "kept and propagated"
    );

    let name = act(ACCOUNT).object.name().clone();
    assert!(
        matches!(objects.resolve(&name), Answer::CannotResolve(_)),
        "and the account is unresolvable rather than resolved without it"
    );
}

/// The account as this node computes it at that moment.
///
/// A function of the record rather than a closure over it, because the story below goes on
/// admitting acts between the questions it asks.
fn held(objects: &Objects, name: &Name, at: Epoch) -> Holder {
    match objects.resolve(name) {
        Answer::Here(State::Holder(holder)) => holder.come_due(at),
        other => panic!("{other:?}"),
    }
}

/// The account stopped by the words, following that request.
///
/// **The panic entrance's first act** (`SPECS.md §11.8`): the phone is gone, there is no device
/// left to sign from, and this is the one thing the words ask for that does not wait.
const FREEZE: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d5863346e6e6a77474866465538417148736e6632704a344c325343444852736a7a4243444d4a4a4c334c7a350306040105184806a10140078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c58408926a56e6a8e21a53a0e6f45625f0e45d4958fc8301902f4bd9530cdaf5c871981786dfc7200f27fc41d5b19bfc98952fdcd7a128bfd216fb4eeeb21276de102";

/// And the lost device taken off it afterwards, which does wait.
const LOSE_DEVICE: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d63755163787842426b6e4d4c75455058645a59697a7674664e354a656f5342347a5547573671796d756f6a6b0303040105184806a1015821027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a3429617865078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c584007e7f6b52c50bb3ff6ea80d54e008d3c380ddf5764128a03ebea412ee3ed51fe33458124e46dddaaf5e0b4efc7a857b00775896bf2f7855af647a059c2b34c0d";

#[test]
fn the_words_alone_stop_an_account_at_once_and_take_the_lost_device_off_it_afterwards() {
    // **The worst hour of using this, and the two programs have to agree about it.** Somebody whose
    // phone has been taken types their words into a clean install; the app composes these two acts
    // and this node applies them. Stopping is immediate, so the stolen device is inert now; the
    // removal waits the same seventy-two epochs as everything else the words sign alone, which does
    // not matter because what was doing harm has already stopped.
    let mut objects = Objects::new();
    objects
        .admit(&act(ACCOUNT), Epoch::GENESIS)
        .expect("the account");
    objects
        .admit(&act(ADD_DEVICE), Epoch::GENESIS)
        .expect("the device it asks for");

    let name = act(ACCOUNT).object.name().clone();

    // Dated where somebody could actually lose the device: once the first asking has come due and
    // the phone is on the account.
    let operative = Epoch::GENESIS.plus(Epochs(72)).expect("no overflow");
    let device: Vec<u8> = (0..DEVICE.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&DEVICE[at..at + 2], 16).expect("hexadecimal"))
        .collect();
    assert!(held(&objects, &name, operative).devices.contains(&device));

    assert_eq!(
        objects.admit(&act(FREEZE), operative),
        Ok(Admitted::Extended),
        "stopping it is taken"
    );
    assert!(
        held(&objects, &name, operative).frozen,
        "and it is in force at once, not in three days"
    );

    assert_eq!(
        objects.admit(&act(LOSE_DEVICE), operative),
        Ok(Admitted::Extended),
        "and a stopped account still takes what the words ask for, into the window"
    );
    assert!(
        held(&objects, &name, operative).devices.contains(&device),
        "still on it while the wait runs"
    );

    let due = operative.plus(Epochs(72)).expect("no overflow");
    assert!(
        !held(&objects, &name, due).devices.contains(&device),
        "and off it once the wait is out"
    );
    assert!(
        held(&objects, &name, due).frozen,
        "on an account that is still stopped"
    );
}

#[test]
fn the_app_follows_the_head_the_node_computes() {
    // Two implementations naming one act. The app said this addition follows the creation; the
    // node names that creation itself, from its own bytes. If the two named acts differently the
    // second would be an act following nothing, and every account would stop after one line.
    let account = act(ACCOUNT);
    let add = act(ADD_DEVICE);
    assert_eq!(
        add.previous.as_ref(),
        Some(&account.called()),
        "and they agree on what the first act is called"
    );
}

/// The device that first one then added, at once, because a device acts immediately.
const ADD_SECOND: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d5863346e6e6a77474866465538417148736e6632704a344c325343444852736a7a4243444d4a4a4c334c7a350302040105184806a101582103fe53b8e41729ab52deb45cee0a0e27ca771c5910d990e6dfdaf808bf2b97fddb078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5821027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a3429617865584091b392fe3c367f7d866a6c4694929a03db4a13c208e09d2c32d324d9c814776e436a837554f0e0ec0e142a3290fe1995a2e97623baa26885d382c9be12010610";

/// A removal the words then asked for, which waits.
const REMOVE_SECOND: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d50574847716b3351345a3938344337684e4d61364d54546d316a34506b61656b616b326744554646695754350303040105184806a101582103fe53b8e41729ab52deb45cee0a0e27ca771c5910d990e6dfdaf808bf2b97fddb078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5820ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c5840b52cb049b38c6d55ada4d6c8b58c40a2ca0032af037a20c7e3a6c65fe4b279fe190ee7abb64b7f228da601971a7ecad579ab63e743103fa4a347752190cde10f";

/// And the first device saying no to it, inside the window.
const CANCEL: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d52724371793675623372567753776467586e4e776e35534b706d4a69706b6b5555473372685335647a324e6b0309040105184806a101782f7a516d52724371793675623372567753776467586e4e776e35534b706d4a69706b6b5555473372685335647a324e6b078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5821027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a34296178655840392554503613c41397d129e186fe9a850d9a959f82dde58be05328bc7e0aaa1a97668bbd3222b2c8feec7ac0a32c1eb22b17e22706c09ae2e3c44ed2d0666b65";

/// The second device key, as the account carries it.
const SECOND: &str = "03fe53b8e41729ab52deb45cee0a0e27ca771c5910d990e6dfdaf808bf2b97fddb";

#[test]
fn a_whole_story_comes_out_the_same_here_as_it_does_in_the_app() {
    // **Three programs applying one set of rules, and this is where two of them would be caught
    // disagreeing.** The app folds these same five acts for itself — it must, because a device
    // that could not work out its own account could not show somebody an asking made with their
    // stolen words. The twin of this assertion is in the app's `holder::account`.
    let mut objects = Objects::new();
    let due = Epoch::GENESIS.plus(Epochs(72)).expect("no overflow");
    for (written, at) in [
        (ACCOUNT, Epoch::GENESIS),
        (ADD_DEVICE, Epoch::GENESIS),
        (ADD_SECOND, due),
        (REMOVE_SECOND, due),
        (CANCEL, due),
    ] {
        assert_eq!(
            objects.admit(&act(written), at),
            Ok(Admitted::Extended),
            "every act of the story is taken"
        );
    }

    let name = act(ACCOUNT).object.name().clone();
    let long_after = due.plus(Epochs(1_000)).expect("no overflow");
    let Answer::Here(State::Holder(holder)) = objects.resolve(&name) else {
        panic!("it resolves")
    };
    let holder = holder.come_due(long_after);

    let device = |written: &str| -> Vec<u8> {
        (0..written.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&written[at..at + 2], 16).expect("hexadecimal"))
            .collect()
    };
    assert_eq!(
        holder.devices,
        [device(DEVICE), device(SECOND)].into_iter().collect(),
        "both devices, and the removal struck out before it could land"
    );
    assert!(holder.waiting.is_empty(), "nothing left in flight");
    assert!(!holder.frozen);
}

/// The guardians the app named, in the commitment's order, with the salt each leaf was made with.
///
/// Three identifiers the app made up for its twin — hashes of eight bytes, not accounts anybody
/// created — which is what the two tests below turn on.
fn guardians_named() -> Vec<(Did, Vec<u8>)> {
    (1..=3u8)
        .map(|seed| {
            (
                Did::new(Network::Development, Name::of(&[seed; 8])),
                vec![seed; SALT_WIDTH],
            )
        })
        .collect()
}

/// The device naming three guardians of whom two are enough, issued once the device has landed.
const SET_GUARDIANS: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d5863346e6e6a77474866465538417148736e6632704a344c325343444852736a7a4243444d4a4a4c334c7a350308040105184806a30b58203a2a99ebe10e622657a9051dbd4648d2651e43832d435c41ece8eafdecb978a40d030f02078183783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e5821027135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a34296178655840bca0cb60dbb66192dd1aed62ebcbbc219eeafd3b2b09ca2d7a20dff0f2054694416aa8ad9fcb036d211b247ae558597efc53be076494455380605662f7c08c02";

/// Two of those guardians stopping the account, each showing their leaf, signed from their devices
/// (`[31; 32]` and `[32; 32]`) and by nobody on the account.
const GUARDIANS_FREEZE: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d6365654a464c61437461716b43574e3131576343764164324844414576416b6a345348437350673539636d670306040105184806a201401183a401783e6469643a616c6d656e613a6465763a7a516d4e65744751346243755a4c55334431566a5668385650714679506b59574c584e56766445624147507246794a035001010101010101010101010101010101050007825820162b3727bf4473b7a92101a60d12c77edd36002b10d22b38f8a5ac631bcd36ae582033e1ba02101cd16532c720e9cdbf00e67816e921968f615f2ba4334cd7c5f5a8a401783e6469643a616c6d656e613a6465763a7a516d50546d4233567557733971577a655a755543637a4741416831625466374c704e716b7243767434355644616603500202020202020202020202020202020205010782582021d0a08fba5e9f47ad976a2b601c4f1859dff887a8a1c7114a7e312708bda133582033e1ba02101cd16532c720e9cdbf00e67816e921968f615f2ba4334cd7c5f5a8a401783e6469643a616c6d656e613a6465763a7a516d63526f70785255744143373679456777486733737833443247475451627a6767535053555a464c524441653203500303030303030303030303030303030305020781582089933ca25aa0c322b0f4a9928806ea5b528f53a4c154fc60557247b337d9e8d4078283783e6469643a616c6d656e613a6465763a7a516d4e65744751346243755a4c55334431566a5668385650714679506b59574c584e56766445624147507246794a5821036bf85d5fe84598b10ca6199ec09ccd7d35dcd7195fdd1dc7737a0c67006ef25158403cdbc6e1bf04718f2947488b6cf17c7582ca96d7e52b6275ed10a47a2a019a3b72babce2bdba8274914ca88ccc57ba7accaa67e4a06fe8099e0ffd3c0fb8c1bf83783e6469643a616c6d656e613a6465763a7a516d50546d4233567557733971577a655a755543637a4741416831625466374c704e716b724376743435564461665821020e539e011304a06ad552227a677a388cee251cfe15486e5ae02230a79d590ee05840709515182b8422f587e44bcc4232c9e9a041c527048dddd31e3877dd38c23c30166f657a54176895a43068ee3c3dd2ba1d3cbbb8079c51da8d390e9b01c6958c";

/// The account taken back with new words (`[11; 32]`) and a new device (`[21; 32]`), signed by the
/// new control key first and then by two guardians (`[32; 32]` and `[33; 32]`).
const RECOVER: &str = "a701783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e02782f7a516d50546475456a6e79333672375554487755717139527942345851536844706b4e5934796369423938556548750305040105184806a301582066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a1183a401783e6469643a616c6d656e613a6465763a7a516d4e65744751346243755a4c55334431566a5668385650714679506b59574c584e56766445624147507246794a035001010101010101010101010101010101050007825820162b3727bf4473b7a92101a60d12c77edd36002b10d22b38f8a5ac631bcd36ae582033e1ba02101cd16532c720e9cdbf00e67816e921968f615f2ba4334cd7c5f5a8a401783e6469643a616c6d656e613a6465763a7a516d50546d4233567557733971577a655a755543637a4741416831625466374c704e716b7243767434355644616603500202020202020202020202020202020205010782582021d0a08fba5e9f47ad976a2b601c4f1859dff887a8a1c7114a7e312708bda133582033e1ba02101cd16532c720e9cdbf00e67816e921968f615f2ba4334cd7c5f5a8a401783e6469643a616c6d656e613a6465763a7a516d63526f70785255744143373679456777486733737833443247475451627a6767535053555a464c524441653203500303030303030303030303030303030305020781582089933ca25aa0c322b0f4a9928806ea5b528f53a4c154fc60557247b337d9e8d413582103fe53b8e41729ab52deb45cee0a0e27ca771c5910d990e6dfdaf808bf2b97fddb078383783e6469643a616c6d656e613a6465763a7a516d4e525466466d48564d6741767252636e57393569563344554c4e3541576e39697a5074656f5238674d6f726e582066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a58400a9de01090bf72457ca07470ff377a9abd107c3103055abe2f058551339bb01fcd618f8816cfb6df0874d72eca1c58649f7c53f5e32f9581b7593733cf3ad30a83783e6469643a616c6d656e613a6465763a7a516d50546d4233567557733971577a655a755543637a4741416831625466374c704e716b724376743435564461665821020e539e011304a06ad552227a677a388cee251cfe15486e5ae02230a79d590ee05840ad689be7f47203fcb898ac8d28a722b6d6251d436e802353460ddebab18f5abb43340d0d01d25d24dceb99277a3ffae128833781547ca8261121101f476efea083783e6469643a616c6d656e613a6465763a7a516d63526f70785255744143373679456777486733737833443247475451627a6767535053555a464c5244416532582103462dba1ae4fc1a968b4dacf20cdd6dbe1fae34aa971514a63d3405c3d1cfd383584003e4ce54977e82a9bb08861bc0e71fca09c080fb7dc5a9d5d8cf906822239983b354a3ba25f8e86e49f8a406c9d68cfa7580c7e439a260233f54aa7e9b451ebf";

/// The record with the account, its device landed, and the guardians named.
fn with_guardians() -> (Objects, Name, Epoch) {
    let mut objects = Objects::new();
    objects
        .admit(&act(ACCOUNT), Epoch::GENESIS)
        .expect("the account");
    objects
        .admit(&act(ADD_DEVICE), Epoch::GENESIS)
        .expect("the device it asks for");
    let due = Epoch::GENESIS.plus(Epochs(72)).expect("no overflow");
    assert_eq!(
        objects.admit(&act(SET_GUARDIANS), due),
        Ok(Admitted::Extended),
        "a device names guardians"
    );
    (objects, act(ACCOUNT).object.name().clone(), due)
}

#[test]
fn the_guardians_the_app_names_are_the_commitment_this_node_reads() {
    // **Two Merkle trees agreeing on one root.** The app committed to three guardians with its
    // own tree; this node recomputes the commitment from the same leaves with the tree it keeps
    // its record in, and finds the same thirty-two bytes. A disagreement here would be a guardian
    // every node refuses because their proof leads to a root nobody wrote.
    let (objects, name, due) = with_guardians();
    let guardians = held(&objects, &name, due)
        .guardians
        .expect("named, and at once: a device names guardians");
    assert_eq!(guardians.commitment, commit(&guardians_named()));
    assert_eq!(guardians.how_many, 3);
    assert_eq!(guardians.enough, 2);
    assert!(
        !held(&objects, &name, due).frozen,
        "naming guardians stops nothing"
    );
}

#[test]
fn the_proofs_the_app_composes_hold_and_the_guardians_it_names_are_nobody_on_this_record() {
    // **Half of each act meets the record, and the other half cannot.** The proofs are the app's
    // own arithmetic — a salt, a leaf, a path — and every one of them lands on the commitment,
    // so that half is held here. The signatures are made in the name of three identifiers the
    // app invented for its twin: hashes of eight bytes, which no creation act could ever be
    // called, so no account exists under them and no key is authorised for them. A guardian is
    // counted only when their own chain says the signing key is theirs, which is what stops a
    // stranger with a valid-looking proof from freezing anybody — so both acts are refused,
    // rightly, and the app's bytes cannot be carried further than this on any record.
    let (mut objects, name, due) = with_guardians();
    let guardians = held(&objects, &name, due).guardians.expect("named");

    let freeze = act(GUARDIANS_FREEZE);
    let proofs = guardian::proofs(&freeze);
    assert_eq!(proofs.len(), 3, "one proof per guardian named");
    for (proof, (who, salt)) in proofs.iter().zip(guardians_named()) {
        assert_eq!(proof.guardian, who);
        assert_eq!(proof.salt, salt);
        assert!(
            guardians.holds(proof),
            "the app's path for {who} leads to the commitment"
        );
    }
    assert_eq!(
        objects.admit(&freeze, due),
        Err(Refused::NotAuthorised),
        "two signatures in the name of accounts that do not exist count as nobody"
    );
    assert!(
        !held(&objects, &name, due).frozen,
        "so the account is not stopped"
    );

    let recover = act(RECOVER);
    assert_eq!(recover.previous.as_ref(), Some(&freeze.called()));
    for proof in guardian::proofs(&recover) {
        assert!(
            guardians.holds(&proof),
            "the same proofs, and they still hold"
        );
    }
    assert_eq!(
        objects.admit(&recover, due),
        Err(Refused::NoSuchPredecessor),
        "and a recovery following a freeze that was never written down follows nothing"
    );
}
