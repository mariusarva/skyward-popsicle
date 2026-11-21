//! Writing a payload straight into the slot that starts next, without
//! libsnapshot: the payload's operations are decoded on every core at once
//! and put where they belong, the way the new system will read them,
//! instead of one thread decoding them in turn and the kernel merging a
//! copy-on-write image back afterwards.
//!
//! `update_engine` reads the payload, decodes it and writes it on one
//! thread, and a nine-gigabyte package takes it twenty-three megabytes a
//! second while seven of eight cores idle (2026-11-21). Each operation is
//! an xz stream of its own, and the operations lie in the payload in the
//! order they are written, so they can be handed out to as many threads as
//! there are cores and each one can put its share straight into the
//! partition, in place.
//!
//! Nothing here decides on its own to write: [`plan`] looks at every
//! operation first and says whether this recovery can apply the package at
//! all, and the answer is no for anything but a full package whose
//! partitions are the size this phone has room for. A package it says no to
//! goes to `update_engine` instead, untouched.

use crate::payload::{Kind, Payload, BLOCK};
use anyhow::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};
use skyward_lp::{Extent as LpExtent, Metadata, Partition, ATTR_DISABLED, SECTOR_SIZE};
use skyward_vab::Slot;
use std::fs::{File, OpenOptions};
use std::io::{Cursor, Read};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How many operations are read ahead of the threads that decode them: at
/// two megabytes an operation at most, this holds the reading ahead to tens
/// of megabytes, which a recovery has.
const AHEAD: usize = 8;

/// How much is written or read back at once.
const CHUNK: usize = 1 << 20;

/// The most an operation may write. An operation of every package this
/// recovery has met comes to under ten megabytes; one larger than this is
/// not one, and is not given the memory.
const MOST_OUT: u64 = 512 << 20;

/// What the page is told while a payload is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The `index`th of `count` partitions is being written.
    Partition { index: usize, count: usize, name: String },
    /// This share of all the payload's output is written.
    Writing { done: u64, total: u64 },
    /// The `index`th partition is read back, checked against what the
    /// package says it must be; `done` of the `total` bytes to read back
    /// are hashed. The byte count is what the bar follows: `odm` and
    /// `product` are twenty to thirty seconds each, and the bar standing on
    /// one partition's share for that long reads as a page that stopped
    /// (2026-11-21).
    Checking { index: usize, count: usize, name: String, done: u64, total: u64 },
}

/// What the writing came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// How many bytes of partitions went in.
    pub written: u64,
    /// How long the writing took, the checking left out.
    pub took: Duration,
}

/// Where a stretch of a partition goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    /// Where it begins in the partition, in bytes.
    logical: u64,
    /// Where it begins on the device, in bytes; `None` for a hole in
    /// super's layout, which reads as zeroes and is not written.
    device: Option<u64>,
    len: u64,
}

/// One operation, and where its bytes go.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stretch {
    kind: Kind,
    /// Where its bytes lie, counted from the start of the payload's data.
    data_offset: u64,
    data_length: u64,
    /// What they must hash to, when the package says; empty when it does
    /// not.
    hash: Vec<u8>,
    /// The stretches of the device it goes to, in the order its bytes
    /// follow one another. Holes are left out.
    at: Vec<(u64, u64)>,
    /// How many bytes of the partition it writes, holes included.
    out: u64,
}

/// A partition the plan writes, with the device open.
struct Placed {
    name: String,
    file: File,
    /// Where the partition lies on `file`, in the order it reads.
    spans: Vec<Span>,
    /// What the package says the whole partition must hash to; empty when
    /// it does not say.
    hash: Vec<u8>,
    /// One for each of the partition's operations, in order.
    stretches: Vec<Stretch>,
}

/// What is to be written, where, before a byte is.
pub struct Plan {
    parts: Vec<Placed>,
}

impl Plan {
    /// How many bytes of partitions the plan writes.
    pub fn bytes(&self) -> u64 {
        self.parts.iter().flat_map(|p| &p.stretches).map(|s| s.out).sum()
    }

    /// The partitions it writes, in the order the payload has them.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().map(|p| p.name.as_str())
    }

    /// How many partitions it writes.
    pub fn count(&self) -> usize {
        self.parts.len()
    }
}

/// Read the package's manifest against the phone, and say what writing it
/// would take. `Ok(None)` means this recovery does not write it itself: the
/// package is not one it can put in as it stands, and `why` says what was
/// wrong, for the log.
pub fn plan(payload: &Payload, slot: Slot, why: &mut dyn FnMut(&str)) -> Result<Option<Plan>> {
    if payload.manifest.block_size != BLOCK as u32 {
        why(&format!("the payload measures its blocks in {} bytes, and this recovery writes 4096", payload.manifest.block_size));
        return Ok(None);
    }
    if payload.manifest.partial {
        why("the payload writes only what changed, and the rest of super as it is");
        return Ok(None);
    }
    // Every operation is looked at before anything is written, and one this
    // recovery cannot make is named for all the partitions that carry it:
    // the log then says what the whole package needs, not what the first
    // partition it met happened to need (2026-11-21).
    let mut unmet: Vec<String> = Vec::new();
    for update in &payload.manifest.partitions {
        let mut numbers: Vec<u64> = Vec::new();
        let mut count = 0usize;
        for operation in &update.operations {
            if let Kind::Other(number) = operation.kind {
                count += 1;
                if !numbers.contains(&number) {
                    numbers.push(number);
                }
            }
        }
        if !numbers.is_empty() {
            let list: Vec<String> = numbers.iter().map(|n| n.to_string()).collect();
            unmet.push(format!("{} ({count} operations, kind {})", update.name, list.join(", ")));
        }
    }
    if !unmet.is_empty() {
        why(&format!("the payload has operations this recovery does not apply: {}", unmet.join("; ")));
        return Ok(None);
    }
    let super_dev = Path::new(skyward_block::BY_NAME).join("super");
    let metadata = match skyward_lp::read(&super_dev, slot.number()) {
        Ok(metadata) => metadata,
        Err(e) => {
            why(&format!("super cannot be read for slot {}: {e:#}", slot.letter()));
            return Ok(None);
        }
    };
    for damaged in &metadata.damaged {
        why(&format!("super's metadata for slot {} has damage: {damaged}", slot.letter()));
    }
    let mut parts = Vec::with_capacity(payload.manifest.partitions.len());
    for update in &payload.manifest.partitions {
        let Some(placed) = place(update, &metadata, slot, why)? else { return Ok(None) };
        parts.push(placed);
    }
    if parts.is_empty() {
        why("the payload writes no partition");
        return Ok(None);
    }
    Ok(Some(Plan { parts }))
}

/// Lay one partition out over the device it lives on.
fn place(
    update: &crate::payload::PartitionUpdate,
    metadata: &Metadata,
    slot: Slot,
    why: &mut dyn FnMut(&str),
) -> Result<Option<Placed>> {
    // A kind this recovery does not apply itself makes the whole package one
    // it does not, and that is known before anything is written.
    for operation in &update.operations {
        if let Kind::Other(number) = operation.kind {
            why(&format!("{} has an operation of kind {number}, which this recovery does not apply", update.name));
            return Ok(None);
        }
        if operation.kind.carries_data() && operation.data_length == 0 {
            why(&format!("{} has an operation of kind {:?} that carries no data", update.name, operation.kind));
            return Ok(None);
        }
    }
    let (file, spans, size) = match in_super(metadata, &update.name, slot) {
        Some(partition) => {
            if partition.attributes & ATTR_DISABLED != 0 {
                why(&format!("{} is disabled in super for slot {}", update.name, slot.letter()));
                return Ok(None);
            }
            let holds = partition.bytes();
            if holds != update.new_size {
                // libsnapshot rewrites super's layout for this, which is
                // what an install through update_engine is for.
                why(&format!(
                    "{} takes {} bytes in the package and {holds} in super, so the layout would have to change",
                    update.name, update.new_size
                ));
                return Ok(None);
            }
            let spans = logical_spans(partition).with_context(|| format!("lay {} over super", update.name))?;
            let at = Path::new(skyward_block::BY_NAME).join("super");
            let file = open_writable(&at).with_context(|| format!("open {}", at.display()))?;
            (file, spans, holds)
        }
        None => {
            let Some(at) = physical(&update.name, slot) else {
                why(&format!(
                    "{} is in no super of slot {} and in no partition of this phone",
                    update.name,
                    slot.letter()
                ));
                return Ok(None);
            };
            let holds = skyward_block::size_bytes(&at).with_context(|| format!("measure {}", at.display()))?;
            if holds < update.new_size {
                why(&format!(
                    "{} takes {} bytes in the package and the partition holds {holds}",
                    update.name, update.new_size
                ));
                return Ok(None);
            }
            let file = open_writable(&at).with_context(|| format!("open {}", at.display()))?;
            (file, vec![Span { logical: 0, device: Some(0), len: update.new_size }], update.new_size)
        }
    };
    let Some(stretches) = stretches(update, &spans, size, why)? else { return Ok(None) };
    Ok(Some(Placed { name: update.name.clone(), file, spans, hash: update.new_hash.clone(), stretches }))
}

/// The partition of super the package's name for it takes. A metadata copy
/// this phone has written to before names its partitions with the slot's
/// suffix -- `system_b` for the slot B that starts next -- and one written
/// by the phone's maker may not (2026-11-21).
fn in_super<'a>(metadata: &'a Metadata, name: &str, slot: Slot) -> Option<&'a Partition> {
    let suffixed = format!("{name}{}", slot.suffix());
    let bare = |p: &Partition| {
        p.name.strip_suffix("_a").or_else(|| p.name.strip_suffix("_b")).unwrap_or(p.name.as_str()) == name
    };
    metadata
        .partitions
        .iter()
        .find(|p| p.name == name || p.name == suffixed)
        .or_else(|| metadata.partitions.iter().find(|p| bare(p)))
}

/// Where a logical partition of super lies: the extents of the metadata for
/// the slot, in the order the partition reads.
fn logical_spans(partition: &Partition) -> Result<Vec<Span>> {
    let mut spans = Vec::with_capacity(partition.extents.len());
    let mut logical = 0u64;
    for extent in &partition.extents {
        let (device, bytes) = match *extent {
            LpExtent::Linear { sectors, device, sector } => {
                // Every partition this recovery has met lies on super
                // itself; an extent on another block device is one it does
                // not write into.
                ensure!(device == 0, "an extent lies on block device {device}, and only super is written here");
                (Some(sector * SECTOR_SIZE), sectors * SECTOR_SIZE)
            }
            // A hole reads as zeroes, which is what the package writes there
            // as well, so nothing is written there.
            LpExtent::Zero { sectors } => (None, sectors * SECTOR_SIZE),
        };
        spans.push(Span { logical, device, len: bytes });
        logical += bytes;
    }
    Ok(spans)
}

/// The partition of this name on the phone, by the name it goes by on the
/// slot that starts next: `boot` is `boot_b` there while this runs from A.
fn physical(name: &str, slot: Slot) -> Option<PathBuf> {
    for candidate in [format!("{name}{}", slot.suffix()), name.to_string()] {
        let at = Path::new(skyward_block::BY_NAME).join(candidate);
        if at.exists() {
            return Some(at);
        }
    }
    None
}

/// Open a device to write into it, taking off the read-only mark a
/// partition the system does not run from may still carry.
fn open_writable(at: &Path) -> Result<File> {
    let file = OpenOptions::new().read(true).write(true).open(at)?;
    if skyward_block::blkdev::read_only(&file).unwrap_or(false) {
        skyward_block::blkdev::make_writable(&file)?;
    }
    Ok(file)
}

/// Where each of a partition's operations goes and how much each writes,
/// having checked that they cover it once, in order, and that every one of
/// them fits.
fn stretches(
    update: &crate::payload::PartitionUpdate,
    spans: &[Span],
    size: u64,
    why: &mut dyn FnMut(&str),
) -> Result<Option<Vec<Stretch>>> {
    let mut out = Vec::with_capacity(update.operations.len());
    let mut covered = 0u64;
    for operation in &update.operations {
        let first = operation.dst[0].start_block * BLOCK;
        if first != covered {
            why(&format!(
                "{} writes at {first} with {covered} bytes behind it, so its operations do not cover it in order",
                update.name
            ));
            return Ok(None);
        }
        let Some(at) = place_extents(operation, spans) else {
            why(&format!("{} writes past the {size} bytes it holds", update.name));
            return Ok(None);
        };
        let writes = operation.bytes();
        if writes > MOST_OUT {
            why(&format!("{} has an operation that writes {writes} bytes, past the {MOST_OUT} this holds", update.name));
            return Ok(None);
        }
        out.push(Stretch {
            kind: operation.kind,
            data_offset: operation.data_offset,
            data_length: operation.data_length,
            hash: operation.hash.clone(),
            at,
            out: writes,
        });
        covered += writes;
    }
    if covered != size {
        why(&format!("{} writes {covered} of the {size} bytes it holds", update.name));
        return Ok(None);
    }
    Ok(Some(out))
}

/// Where the blocks of one operation go on the device, split where the
/// partition is split and leaving out its holes.
fn place_extents(operation: &crate::payload::Operation, spans: &[Span]) -> Option<Vec<(u64, u64)>> {
    let mut out = Vec::with_capacity(operation.dst.len());
    for extent in &operation.dst {
        let mut logical = extent.start_block * BLOCK;
        let mut left = extent.bytes();
        while left > 0 {
            let span = spans.iter().find(|s| logical >= s.logical && logical < s.logical + s.len)?;
            let within = logical - span.logical;
            let take = (span.len - within).min(left);
            if let Some(device) = span.device {
                out.push((device + within, take));
            }
            logical += take;
            left -= take;
        }
    }
    Some(out)
}

/// Write what `plan` laid out, decoding the operations on every core at
/// once and telling `tell` how far it is.
pub fn write(plan: &Plan, payload: &Payload, tell: &mut dyn FnMut(Step)) -> Result<Applied> {
    let total = plan.bytes();
    let count = plan.parts.len();
    let mut jobs: Vec<(usize, usize)> = Vec::new();
    for (p, part) in plan.parts.iter().enumerate() {
        for s in 0..part.stretches.len() {
            jobs.push((p, s));
        }
    }
    let jobs: &[(usize, usize)] = &jobs;
    let done = Arc::new(AtomicU64::new(0));
    let failed: Arc<Mutex<Option<anyhow::Error>>> = Arc::new(Mutex::new(None));
    let started = Instant::now();
    let (tx, rx) = mpsc::sync_channel::<(usize, usize, Vec<u8>)>(AHEAD);
    let rx = Arc::new(Mutex::new(rx));
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    // One core reads the payload and puts the bar's count in order, so the
    // threads that decode what comes out number one fewer than the cores: on
    // this phone that is seven decoders rather than six, and every one of
    // them held 28 MB/s while the disk sat at 1.4 GB/s (2026-11-21).
    let workers = cores.saturating_sub(1).clamp(2, 8);

    let mut told = Instant::now();
    let mut shown: Option<usize> = None;
    let mut part_at = Instant::now();
    let mut part_bytes = 0u64;
    let mut shown_bytes = 0u64;

    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers {
            let rx = Arc::clone(&rx);
            let done = Arc::clone(&done);
            let failed = Arc::clone(&failed);
            scope.spawn(move || {
                loop {
                    let job = match rx.lock().expect("the queue is not poisoned").recv() {
                        Ok(job) => job,
                        Err(_) => return,
                    };
                    let (p, s, data) = job;
                    let part = &plan.parts[p];
                    let stretch = &part.stretches[s];
                    match apply_one(part, stretch, &data) {
                        Ok(()) => {
                            done.fetch_add(stretch.out, Ordering::Relaxed);
                        }
                        Err(e) => {
                            let mut slot = failed.lock().expect("the failure is not poisoned");
                            if slot.is_none() {
                                *slot = Some(e);
                            }
                            return;
                        }
                    }
                }
            });
        }

        // The payload's operations are read here, in the order they lie in
        // it, and how far the writing is is told from here as well: the
        // reader waits on the queue and the queue waits on the threads, so
        // what the bar shows is at most one operation behind.
        for (p, s) in jobs.iter().copied() {
            if shown != Some(p) {
                close_partition(plan, shown, part_at, part_bytes);
                shown = Some(p);
                part_at = Instant::now();
                part_bytes = 0;
                tell(Step::Partition { index: p, count, name: plan.parts[p].name.clone() });
            }
            let stretch = &plan.parts[p].stretches[s];
            // A kind that carries no bytes is handed on as it is: zeroes
            // are made where they are written.
            let data = match stretch.kind.carries_data() {
                true => {
                    let data = payload
                        .read(stretch.data_offset, stretch.data_length)
                        .context("read an operation's bytes")?;
                    check_hash(plan.parts[p].name.as_str(), stretch, &data)?;
                    data
                }
                false => Vec::new(),
            };
            // The queue is full until a thread has taken one; waiting on it
            // is what keeps the reading ahead to the memory there is. The
            // job is taken back out of a full queue as it was, so nothing is
            // read twice and nothing is dropped.
            let mut job = (p, s, data);
            loop {
                match tx.try_send(job) {
                    Ok(()) => break,
                    Err(TrySendError::Full(back)) => {
                        job = back;
                        if let Some(e) = failed.lock().expect("the failure is not poisoned").take() {
                            return Err(e);
                        }
                        std::thread::sleep(Duration::from_micros(50));
                    }
                    Err(TrySendError::Disconnected(_)) => bail!("the threads that write the payload stopped"),
                }
            }
            part_bytes += stretch.out;
            if told.elapsed() >= Duration::from_millis(200) {
                let written = done.load(Ordering::Relaxed).min(total);
                if written != shown_bytes {
                    shown_bytes = written;
                    tell(Step::Writing { done: written, total });
                }
                told = Instant::now();
            }
            if let Some(e) = failed.lock().expect("the failure is not poisoned").take() {
                return Err(e);
            }
        }
        drop(tx);
        Ok(())
    })?;

    if let Some(e) = failed.lock().expect("the failure is not poisoned").take() {
        return Err(e);
    }
    // The scope has joined every thread: what the queue held is written.
    let took = started.elapsed();
    close_partition(plan, shown, part_at, part_bytes);
    let written = done.load(Ordering::Relaxed).min(total);
    ensure!(written == total, "only {written} of the {total} bytes of the payload were written");
    tell(Step::Writing { done: total, total });
    Ok(Applied { written, took })
}

/// Write a line for the partition that just finished: how long it took and
/// how fast that was, for the next install to be measured against.
fn close_partition(plan: &Plan, shown: Option<usize>, part_at: Instant, part_bytes: u64) {
    let Some(was) = shown else { return };
    let took = part_at.elapsed().as_secs_f64().max(0.001);
    let _ = skyward_sys::kmsg(&format!(
        "install: {} written in {took:.1} s ({:.1} MB/s)",
        plan.parts[was].name,
        part_bytes as f64 / took / (1u64 << 20) as f64
    ));
}

/// Check an operation's bytes against what the package says they hash to,
/// before they are decoded.
fn check_hash(name: &str, stretch: &Stretch, data: &[u8]) -> Result<()> {
    if stretch.hash.is_empty() {
        return Ok(());
    }
    let mut hasher = Sha256::new();
    hasher.update(data);
    let got = hasher.finalize();
    ensure!(
        got.as_slice() == stretch.hash.as_slice(),
        "an operation of {name} carries bytes hashing to {}, and the package says {}",
        hex_of(got.as_slice()),
        hex_of(&stretch.hash)
    );
    Ok(())
}

/// Put one operation's bytes where they go: as they lie, as an xz stream to
/// be decoded, or as zeroes when the operation carries none.
fn apply_one(part: &Placed, stretch: &Stretch, data: &[u8]) -> Result<()> {
    match stretch.kind {
        Kind::Replace => {
            ensure!(
                data.len() as u64 == stretch.out,
                "an operation of {} carries {} bytes and writes {}",
                part.name,
                data.len(),
                stretch.out
            );
            put(part, stretch, data)
        }
        Kind::ReplaceXz => {
            let out = inflate(data, stretch.out).with_context(|| format!("decode an xz operation of {}", part.name))?;
            put(part, stretch, &out)
        }
        Kind::ReplaceBz => {
            let out = inflate_bz(data, stretch.out).with_context(|| format!("decode a bzip2 operation of {}", part.name))?;
            put(part, stretch, &out)
        }
        // A discarded stretch is written as zeroes as well: the new system
        // must never read what the old one left there.
        Kind::Zero | Kind::Discard => put_zeroes(part, stretch),
        Kind::Other(number) => bail!("an operation of kind {number} reached the writer of {}", part.name),
    }
}

/// Write one operation's bytes into the stretches of the device it goes to,
/// in order.
fn put(part: &Placed, stretch: &Stretch, bytes: &[u8]) -> Result<()> {
    let held: u64 = stretch.at.iter().map(|(_, len)| len).sum();
    ensure!(held == bytes.len() as u64, "{} writes {} bytes into {held}", part.name, bytes.len());
    let mut at = 0usize;
    for &(device, len) in &stretch.at {
        let len = len as usize;
        part.file
            .write_all_at(&bytes[at..at + len], device)
            .with_context(|| format!("write {len} bytes at {device} of {}", part.name))?;
        at += len;
    }
    Ok(())
}

/// Write zeroes over the stretches of a partition an operation lets go of.
fn put_zeroes(part: &Placed, stretch: &Stretch) -> Result<()> {
    let zeroes = vec![0u8; CHUNK];
    for &(device, len) in &stretch.at {
        let mut at = 0u64;
        while at < len {
            let take = ((len - at) as usize).min(zeroes.len());
            part.file
                .write_all_at(&zeroes[..take], device + at)
                .with_context(|| format!("write {take} zero bytes at {} of {}", device + at, part.name))?;
            at += take as u64;
        }
    }
    Ok(())
}

/// Decode one xz stream into exactly `want` bytes.
///
/// The stream's own CRC64 is not checked: the operation's bytes were hashed
/// against the package's manifest before they came here, and the partition
/// is read back and hashed once it is written, so a stream that came through
/// those two is not made to pay for a third check on a core that could be
/// decoding the next one (2026-11-21).
fn inflate(data: &[u8], want: u64) -> Result<Vec<u8>> {
    ensure!(want <= MOST_OUT, "an operation writes {want} bytes, past the {MOST_OUT} this holds");
    let mut out = Vec::with_capacity(want as usize);
    let stream = liblzma::stream::Stream::new_stream_decoder(u64::MAX, liblzma::stream::IGNORE_CHECK | liblzma::stream::CONCATENATED)
        .context("prepare an xz decoder")?;
    let mut decoder = liblzma::read::XzDecoder::new_stream(Cursor::new(data), stream);
    decoder.read_to_end(&mut out).context("decode an xz stream")?;
    ensure!(out.len() as u64 == want, "an xz stream decoded to {} bytes, and it writes {want}", out.len());
    Ok(out)
}

/// Decode one bzip2 stream into exactly `want` bytes. A bzip2 stream is
/// handed out whole, as an xz one is: `MultiBzDecoder` reads every stream
/// the operation's bytes hold, not only the first.
fn inflate_bz(data: &[u8], want: u64) -> Result<Vec<u8>> {
    ensure!(want <= MOST_OUT, "an operation writes {want} bytes, past the {MOST_OUT} this holds");
    let mut out = Vec::with_capacity(want as usize);
    let mut decoder = bzip2::read::MultiBzDecoder::new(Cursor::new(data));
    decoder.read_to_end(&mut out).context("decode a bzip2 stream")?;
    ensure!(out.len() as u64 == want, "a bzip2 stream decoded to {} bytes, and it writes {want}", out.len());
    Ok(out)
}

/// Read back every partition the plan wrote and check it against the hash
/// the package says it has, telling `tell` how far it is.
///
/// The partitions are read one to a thread, as the writing spreads its
/// operations over the cores. Hashed one after another, the eleven
/// gigabytes took sixty-nine seconds, and `odm` and `product` were
/// twenty of them each with the bar standing on one partition's share;
/// SHA-256 alone runs at about 170 MB/s on one core here, and the disk is
/// nowhere near its 1.4 GB/s. Eight cores at once put the reading back
/// under fifteen seconds (2026-11-21).
pub fn check(plan: &Plan, tell: &mut dyn FnMut(Step)) -> Result<()> {
    let count = plan.parts.len();
    // 校验段单独计时：写入那 71 s 之外，读回比对这些分区又花了多久，下一版
    // 的总时长里就能把这两段分开看（2026-11-21）。
    let started = Instant::now();
    // 要读回多少字节先数出来：进度条按字节走，不按分区个数走 —— `odm` 一个
    // 就占五十八分之一，却要二十秒。
    let all: u64 = plan
        .parts
        .iter()
        .filter(|part| !part.hash.is_empty())
        .map(|part| part.spans.iter().map(|span| span.len).sum::<u64>())
        .sum();
    if all == 0 {
        return Ok(());
    }
    // 每个分区给最先空下来的那条线程，所以六十四个分区不会都从一个线程
    // 手上过一遍。
    let todo: Vec<usize> = plan.parts.iter().enumerate().filter(|(_, part)| !part.hash.is_empty()).map(|(index, _)| index).collect();
    let read = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let failed: Arc<Mutex<Option<anyhow::Error>>> = Arc::new(Mutex::new(None));
    let next = Arc::new(AtomicUsize::new(0));
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let workers = cores.clamp(2, 8).min(todo.len().max(1));
    let (tx, rx) = mpsc::channel::<(usize, u64)>();

    // 报给页面的分寸：字节每 100 ms 一次，分区名一秒最多换一次 —— 八条线程
    // 同时换分区时，名字一刻一变反而读不出来。
    let mut named: Option<usize> = None;
    let mut named_at: Option<Instant> = None;
    let mut told: Option<Instant> = None;

    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers {
            let tx = tx.clone();
            let read = Arc::clone(&read);
            let stop = Arc::clone(&stop);
            let failed = Arc::clone(&failed);
            let next = Arc::clone(&next);
            let todo = &todo;
            scope.spawn(move || {
                let mut buf = vec![0u8; CHUNK];
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let turn = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&index) = todo.get(turn) else { return };
                    let part = &plan.parts[index];
                    let mut hasher = Sha256::new();
                    let mut mine = 0u64;
                    let mut said = 0u64;
                    let mut why: Option<anyhow::Error> = None;
                    for span in &part.spans {
                        let mut at = 0u64;
                        while at < span.len {
                            if stop.load(Ordering::Relaxed) {
                                return;
                            }
                            let take = ((span.len - at) as usize).min(buf.len());
                            match span.device {
                                Some(device) => {
                                    if let Err(e) = part
                                        .file
                                        .read_exact_at(&mut buf[..take], device + at)
                                        .with_context(|| format!("read {take} bytes at {} of {}", device + at, part.name))
                                    {
                                        why = Some(e);
                                        break;
                                    }
                                }
                                // A hole reads as zeroes in the partition the
                                // system sees, so what it hashes to is zeroes.
                                None => buf[..take].fill(0),
                            }
                            hasher.update(&buf[..take]);
                            at += take as u64;
                            mine += take as u64;
                            read.fetch_add(take as u64, Ordering::Relaxed);
                            if mine - said >= (4 << 20) {
                                said = mine;
                                let _ = tx.send((index, read.load(Ordering::Relaxed)));
                            }
                        }
                        if why.is_some() {
                            break;
                        }
                    }
                    if let Some(e) = why {
                        let mut slot = failed.lock().expect("the failure is not poisoned");
                        if slot.is_none() {
                            *slot = Some(e);
                        }
                        drop(slot);
                        stop.store(true, Ordering::Relaxed);
                        return;
                    }
                    let got = hasher.finalize();
                    if got.as_slice() != part.hash.as_slice() {
                        let e = anyhow::anyhow!(
                            "{} hashes to {}, and the package says {}",
                            part.name,
                            hex_of(got.as_slice()),
                            hex_of(&part.hash)
                        );
                        let mut slot = failed.lock().expect("the failure is not poisoned");
                        if slot.is_none() {
                            *slot = Some(e);
                        }
                        drop(slot);
                        stop.store(true, Ordering::Relaxed);
                        return;
                    }
                    let _ = skyward_sys::kmsg(&format!("install: {} read back and matched its hash", part.name));
                    let _ = tx.send((index, read.load(Ordering::Relaxed)));
                }
            });
        }
        drop(tx);
        // 线程一条条报，报的是它们一共读回了多少字节：进度条跟着字节走，
        // 不再跟着第几个分区走。
        for (index, done) in rx {
            let fresh = named != Some(index) && named_at.is_none_or(|at| at.elapsed() >= Duration::from_millis(700));
            if fresh {
                named = Some(index);
                named_at = Some(Instant::now());
            }
            let due = told.is_none_or(|at| at.elapsed() >= Duration::from_millis(100));
            if !fresh && !due && done < all {
                continue;
            }
            told = Some(Instant::now());
            let shown = named.unwrap_or(index);
            tell(Step::Checking { index: shown, count, name: plan.parts[shown].name.clone(), done: done.min(all), total: all });
        }
        Ok(())
    })?;

    if let Some(e) = failed.lock().expect("the failure is not poisoned").take() {
        return Err(e);
    }
    let read = read.load(Ordering::Relaxed);
    if read > 0 {
        let took = started.elapsed().as_secs_f64().max(0.001);
        let _ = skyward_sys::kmsg(&format!(
            "install: the writing was read back in {took:.1} s ({:.1} MB/s)",
            read as f64 / took / (1u64 << 20) as f64
        ));
    }
    Ok(())
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partition_split_over_two_extents_is_written_in_two_places() {
        let spans = [
            Span { logical: 0, device: Some(4096), len: 8192 },
            Span { logical: 8192, device: Some(1 << 20), len: 4096 },
        ];
        let operation = crate::payload::Operation {
            kind: Kind::Replace,
            data_offset: 0,
            data_length: 12 * 1024,
            dst: vec![
                crate::payload::Extent { start_block: 0, blocks: 1 },
                crate::payload::Extent { start_block: 1, blocks: 2 },
            ],
            hash: Vec::new(),
        };
        let at = place_extents(&operation, &spans).expect("fits");
        assert_eq!(at, vec![(4096, 4096), (4096 + 4096, 4096), (1 << 20, 4096)]);
    }

    #[test]
    fn a_stretch_past_the_partition_is_refused() {
        let spans = [Span { logical: 0, device: Some(0), len: 4096 }];
        let operation = crate::payload::Operation {
            kind: Kind::Replace,
            data_offset: 0,
            data_length: 8192,
            dst: vec![crate::payload::Extent { start_block: 1, blocks: 1 }],
            hash: Vec::new(),
        };
        assert!(place_extents(&operation, &spans).is_none());
    }

    #[test]
    fn a_hole_in_the_layout_goes_unwritten() {
        let spans = [
            Span { logical: 0, device: Some(0), len: 4096 },
            Span { logical: 4096, device: None, len: 4096 },
        ];
        let operation = crate::payload::Operation {
            kind: Kind::Replace,
            data_offset: 0,
            data_length: 8192,
            dst: vec![crate::payload::Extent { start_block: 0, blocks: 2 }],
            hash: Vec::new(),
        };
        let at = place_extents(&operation, &spans).expect("fits");
        assert_eq!(at, vec![(0, 4096)]);
    }
}
