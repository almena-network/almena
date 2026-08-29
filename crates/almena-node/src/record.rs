//! What a node keeps, so that stopping is not forgetting.
//!
//! A node that lost its record when its process ended would be a different node every morning: a
//! new network, a new history, and every proof it had ever handed out worthless. So two files sit
//! beside the key in the node's own directory, and both are append-only.
//!
//! | | What is in it |
//! | --- | --- |
//! | `record.acts` | Every act it accepted, **in the bytes it accepted them in**, in the order it did |
//! | `record.roots` | Every root it published, one per epoch it closed |
//!
//! # Almost nothing is stored, because almost everything is a fold
//!
//! The tree, the chains, what each object resolves to, which network this is, what this node is
//! called, and the instant epoch zero began are all **recomputed** by replaying the acts. Storing
//! them as well would be a second copy of facts the first copy already fixes, and two copies of one
//! fact are two things that can disagree.
//!
//! **The roots are the exception, and they have to be.** A root says where the tree stood *when
//! that epoch closed*, which the finished record no longer shows. A node that came back and worked
//! out a different root for an epoch it had already signed would be producing two signed roots for
//! one epoch — the one piece of misconduct that is provable against a node, committed by accident,
//! against itself.
//!
//! # A record that does not add up stops the node
//!
//! Replay ends by checking the tree it rebuilt against the last root this node signed. If they
//! differ, the node has lost acts it already vouched for, and inclusion proofs it handed out no
//! longer check. **It refuses to open rather than repairing itself**: quietly carrying on would
//! mean serving a history that contradicts what it has already said, which is worse than not
//! coming up.
//!
//! The one thing that is repaired is a **torn tail** — a frame cut short by a machine that stopped
//! mid-write. That act was never answered for, because answering happens after writing, so
//! dropping it takes nothing away from anybody who was told otherwise.

use std::fs::{File, OpenOptions};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use almena_format::entry::Entry;
use almena_format::identifier::Name;
use almena_store::root::Root;

/// What the acts are kept in.
const ACTS: &str = "record.acts";

/// What the roots this node has published are kept in.
const ROOTS: &str = "record.roots";

/// What the log entries are kept in.
///
/// **Separate from the acts, because they are not kept for the same reason.** Every node holds
/// every entry and only the nodes a thing was dealt to hold what its acts said — so a node that
/// kept only the acts would come back from a restart with fewer entries than it had, build a
/// different tree, and contradict roots it had already signed.
const ENTRIES: &str = "record.entries";

/// What every one of these files starts with, so that a file that is not one is not read as one.
const MAGIC: &[u8; 14] = b"almena.record\0";

/// Which layout the frames after the header are in.
///
/// A file written by a build that used a different one is not read. There is no conversion and
/// there does not need to be: a development network is opened again as often as it needs to be,
/// and a production one will be written by a format that is settled before it exists.
const VERSION: u8 = 1;

/// How long the header is: the magic, the version, and one byte spare.
const HEADER: usize = MAGIC.len() + 2;

/// The largest frame that will be read.
///
/// Far above anything an act can be — a node announces a smaller limit than this and holds to it —
/// and here only so that a corrupted length cannot ask for an allocation the size of the machine.
const LARGEST: u32 = 16 * 1024 * 1024;

/// Why a directory's record could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotReadable {
    /// There is a record and it cannot be read, or it is not one of these files.
    ///
    /// **Nothing is written over it.** A directory whose record cannot be read is a question for
    /// whoever owns it, and writing a fresh one would open a second network where there was
    /// already one.
    Unreadable,
    /// The directory could not be made, or the files could not be written.
    NotWritable,
    /// The record replays to a different tree from the one this node already signed.
    ///
    /// It has lost acts it vouched for. Coming up anyway would mean serving a history that
    /// contradicts what this node has already told other people.
    DoesNotAddUp,
    /// What was handed over is not the network it was said to be.
    ///
    /// **The one check that has to happen before anything else.** A node that replayed first and
    /// asked afterwards would already have written somebody else's network to disk and announced
    /// itself on it.
    AnotherNetwork,
    /// An act in the record is one this build will not accept.
    ///
    /// Its own record, refused by itself — which is what happens when a format changed underneath
    /// a directory rather than beside it.
    Refused,
}

/// What a directory is holding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holding {
    /// Nothing. A network may be opened here.
    Nothing,
    /// A node's record: which network, and how much of it.
    ARecord {
        /// The network it is on, which is the name of its first act.
        network: Name,
        /// How many acts it holds.
        written: u64,
    },
    /// There is something here and it cannot be read.
    Unreadable(NotReadable),
}

/// Where the acts of a node in `directory` live.
#[must_use]
pub fn acts_at(directory: &Path) -> PathBuf {
    directory.join(ACTS)
}

/// Where the log entries of a node in `directory` live.
#[must_use]
pub fn entries_at(directory: &Path) -> PathBuf {
    directory.join(ENTRIES)
}

/// Where the roots of a node in `directory` live.
#[must_use]
pub fn roots_at(directory: &Path) -> PathBuf {
    directory.join(ROOTS)
}

/// What `directory` is holding, without changing any of it.
///
/// Asked before anything is done to a directory, because *open a network* and *come back to the one
/// that is here* are different acts and choosing wrongly opens a second network.
#[must_use]
pub fn holding(directory: &Path) -> Holding {
    let path = acts_at(directory);
    if !path.exists() {
        return Holding::Nothing;
    }
    let Ok(frames) = read_frames(&path) else {
        return Holding::Unreadable(NotReadable::Unreadable);
    };
    let Some(first) = frames.first() else {
        // The header is there and no act is. Nothing was ever accepted here, so there is nothing
        // to come back to and a network may still be opened.
        return Holding::Nothing;
    };
    let Ok(value) = almena_format::cbor::read(first) else {
        return Holding::Unreadable(NotReadable::Unreadable);
    };
    let Some(genesis) = almena_format::operation::read(&value) else {
        return Holding::Unreadable(NotReadable::Unreadable);
    };
    Holding::ARecord {
        network: genesis.object.name().clone(),
        written: frames.len() as u64,
    }
}

/// The two files of one node, held open for as long as it runs.
///
/// Held rather than reopened per act: a handle that is opened for each write is a handle that can
/// fail on the write after the one that was answered for.
#[derive(Debug)]
pub struct Record {
    /// Where the acts go.
    acts: File,
    /// Where the log entries go.
    entries: File,
    /// Where the roots go.
    roots: File,
}

impl Record {
    /// Open the record in `directory`, making the files if they are not there.
    ///
    /// # Errors
    ///
    /// [`NotReadable::NotWritable`] when the directory or the files will not take writing, and
    /// [`NotReadable::Unreadable`] when what is there is not one of these files.
    pub fn open(directory: &Path) -> Result<Self, NotReadable> {
        std::fs::create_dir_all(directory).map_err(|_| NotReadable::NotWritable)?;
        Ok(Self {
            acts: opened(&acts_at(directory))?,
            entries: opened(&entries_at(directory))?,
            roots: opened(&roots_at(directory))?,
        })
    }

    /// Write an act down.
    ///
    /// # Errors
    ///
    /// [`NotReadable::NotWritable`]. It is worth stopping for: a node that answered *taken* for an
    /// act it did not manage to keep has told somebody something that is not true.
    pub fn wrote(&mut self, act: &[u8]) -> Result<(), NotReadable> {
        frame(&mut self.acts, act)
    }

    /// Write a log entry down.
    ///
    /// **Every entry, whether or not this node holds what the act said.** The tree over the entries
    /// is what this node has put its name to; one that came back from a restart missing any of them
    /// would build a different tree and contradict a root it had already published.
    ///
    /// # Errors
    ///
    /// [`NotReadable::NotWritable`].
    pub fn noted(&mut self, entry: &Entry) -> Result<(), NotReadable> {
        frame(&mut self.entries, &entry.to_bytes())
    }

    /// Write a root down.
    ///
    /// # Errors
    ///
    /// [`NotReadable::NotWritable`].
    pub fn published(&mut self, root: &Root) -> Result<(), NotReadable> {
        frame(&mut self.roots, &root.to_bytes())
    }

    /// The acts in this record, in order.
    ///
    /// # Errors
    ///
    /// [`NotReadable::Unreadable`].
    pub fn acts(directory: &Path) -> Result<Vec<Vec<u8>>, NotReadable> {
        read_frames(&acts_at(directory))
    }

    /// The log entries in this record, in order.
    ///
    /// Empty for a record written before entries were kept — which is a record whose entries are
    /// all derivable from its acts, because letting go of one was not yet possible.
    ///
    /// # Errors
    ///
    /// [`NotReadable::Unreadable`].
    pub fn entries(directory: &Path) -> Result<Vec<Vec<u8>>, NotReadable> {
        let path = entries_at(directory);
        if path.exists() {
            read_frames(&path)
        } else {
            Ok(Vec::new())
        }
    }

    /// The roots this node has already published, in order.
    ///
    /// # Errors
    ///
    /// [`NotReadable::Unreadable`].
    pub fn roots(directory: &Path) -> Result<Vec<Vec<u8>>, NotReadable> {
        let path = roots_at(directory);
        if path.exists() {
            read_frames(&path)
        } else {
            Ok(Vec::new())
        }
    }
}

/// A file of this kind, opened for appending, made with its header if it is new.
fn opened(path: &Path) -> Result<File, NotReadable> {
    let existed = path.exists();
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)
        .map_err(|_| NotReadable::NotWritable)?;

    if existed {
        let mut header = [0u8; HEADER];
        let mut reading = &file;
        reading
            .read_exact(&mut header)
            .map_err(|_| NotReadable::Unreadable)?;
        if &header[..MAGIC.len()] != MAGIC || header[MAGIC.len()] != VERSION {
            return Err(NotReadable::Unreadable);
        }
        return Ok(file);
    }

    let mut header = Vec::with_capacity(HEADER);
    header.extend_from_slice(MAGIC);
    header.push(VERSION);
    header.push(0);
    file.write_all(&header)
        .map_err(|_| NotReadable::NotWritable)?;
    Ok(file)
}

/// Append one frame, and make sure it is on the disk before saying so.
///
/// **The flush is the point.** Answering *taken* for an act that is only in a buffer is answering
/// for something a power cut takes away, and the node that answered would be the only one who ever
/// knew.
fn frame(file: &mut File, bytes: &[u8]) -> Result<(), NotReadable> {
    let length = u32::try_from(bytes.len()).map_err(|_| NotReadable::NotWritable)?;
    let mut framed = Vec::with_capacity(4 + bytes.len());
    framed.extend_from_slice(&length.to_le_bytes());
    framed.extend_from_slice(bytes);

    file.write_all(&framed)
        .map_err(|_| NotReadable::NotWritable)?;
    file.sync_data().map_err(|_| NotReadable::NotWritable)
}

/// Every complete frame in a file, stopping at the first one that is not.
///
/// A short tail is a machine that stopped mid-write. What it was writing was never answered for —
/// the answer comes after the write — so leaving it out takes nothing from anybody who was told
/// otherwise.
fn read_frames(path: &Path) -> Result<Vec<Vec<u8>>, NotReadable> {
    let mut file = File::open(path).map_err(|_| NotReadable::Unreadable)?;

    let mut header = [0u8; HEADER];
    file.read_exact(&mut header)
        .map_err(|_| NotReadable::Unreadable)?;
    if &header[..MAGIC.len()] != MAGIC || header[MAGIC.len()] != VERSION {
        return Err(NotReadable::Unreadable);
    }

    let mut rest = Vec::new();
    file.seek(SeekFrom::Start(HEADER as u64))
        .map_err(|_| NotReadable::Unreadable)?;
    file.read_to_end(&mut rest)
        .map_err(|_| NotReadable::Unreadable)?;

    let mut frames = Vec::new();
    let mut at = 0usize;
    while at + 4 <= rest.len() {
        let length = u32::from_le_bytes([rest[at], rest[at + 1], rest[at + 2], rest[at + 3]]);
        if length > LARGEST {
            return Err(NotReadable::Unreadable);
        }
        let from = at + 4;
        let Some(to) = from.checked_add(length as usize) else {
            return Err(NotReadable::Unreadable);
        };
        if to > rest.len() {
            // Cut short. Everything before it is whole and stays.
            break;
        }
        frames.push(rest[from..to].to_vec());
        at = to;
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::{HEADER, Holding, NotReadable, Record, acts_at, holding};
    use std::path::PathBuf;

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-record-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn what_went_in_comes_back_in_the_order_it_went_in() {
        // The whole point: `seq` is this node's own position and everything about a proof of
        // inclusion is counted against it, so an order that shifted would break every proof.
        let scratch = Scratch::new("order");
        let mut record = Record::open(&scratch.0).expect("a record");
        for act in [b"one".as_slice(), b"two", b"three"] {
            record.wrote(act).expect("written");
        }

        assert_eq!(
            Record::acts(&scratch.0).expect("read back"),
            vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
    }

    #[test]
    fn an_empty_directory_is_holding_nothing() {
        let scratch = Scratch::new("empty");
        assert_eq!(holding(&scratch.0), Holding::Nothing);
    }

    #[test]
    fn a_record_with_a_header_and_no_acts_may_still_be_opened_on() {
        // Told apart from a record with acts in it. A file that exists is not the same as a node
        // that got as far as accepting anything.
        let scratch = Scratch::new("headeronly");
        Record::open(&scratch.0).expect("a record");
        assert_eq!(holding(&scratch.0), Holding::Nothing);
    }

    #[test]
    fn something_that_is_not_one_of_these_files_is_not_read_as_one() {
        let scratch = Scratch::new("foreign");
        std::fs::create_dir_all(&scratch.0).expect("the directory");
        std::fs::write(acts_at(&scratch.0), b"this is somebody else's file").expect("written");

        assert_eq!(
            holding(&scratch.0),
            Holding::Unreadable(NotReadable::Unreadable),
            "and it is not written over"
        );
        assert!(Record::open(&scratch.0).is_err());
    }

    #[test]
    fn a_tail_cut_short_by_a_stopped_machine_is_left_out_and_the_rest_stays() {
        // What a crash mid-write looks like. The act being written was never answered for, because
        // the answer comes after the write.
        let scratch = Scratch::new("torn");
        let mut record = Record::open(&scratch.0).expect("a record");
        record.wrote(b"whole").expect("written");
        drop(record);

        let mut bytes = std::fs::read(acts_at(&scratch.0)).expect("the file");
        bytes.extend_from_slice(&7u32.to_le_bytes());
        bytes.extend_from_slice(b"cut");
        std::fs::write(acts_at(&scratch.0), &bytes).expect("written");

        assert_eq!(
            Record::acts(&scratch.0).expect("read back"),
            vec![b"whole".to_vec()],
            "the whole one survives and the cut one is not invented"
        );
    }

    #[test]
    fn a_length_bigger_than_anything_stops_the_read() {
        // A corrupted length must not be an instruction to allocate the machine.
        let scratch = Scratch::new("huge");
        Record::open(&scratch.0).expect("a record");
        let mut bytes = std::fs::read(acts_at(&scratch.0)).expect("the file");
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(acts_at(&scratch.0), &bytes).expect("written");

        assert_eq!(Record::acts(&scratch.0), Err(NotReadable::Unreadable));
    }

    #[test]
    fn a_file_written_by_another_format_is_refused_rather_than_guessed_at() {
        let scratch = Scratch::new("version");
        Record::open(&scratch.0).expect("a record");
        let mut bytes = std::fs::read(acts_at(&scratch.0)).expect("the file");
        bytes[HEADER - 2] = 99;
        std::fs::write(acts_at(&scratch.0), &bytes).expect("written");

        assert_eq!(Record::acts(&scratch.0), Err(NotReadable::Unreadable));
    }

    #[test]
    fn opening_a_record_twice_carries_on_where_it_left_off() {
        // A node that started its file again on every run would lose everything it had, which is
        // the whole thing this exists to stop.
        let scratch = Scratch::new("again");
        Record::open(&scratch.0)
            .expect("a record")
            .wrote(b"before")
            .expect("written");
        Record::open(&scratch.0)
            .expect("the same record")
            .wrote(b"after")
            .expect("written");

        assert_eq!(
            Record::acts(&scratch.0).expect("read back"),
            vec![b"before".to_vec(), b"after".to_vec()]
        );
    }
}
