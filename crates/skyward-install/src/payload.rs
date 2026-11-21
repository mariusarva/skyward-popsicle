//! The payload an A/B package carries: which partition each stretch of the
//! package's bytes belongs to, and what those bytes turn into. The whole
//! manifest is read here, before anything is written, so that a package
//! this recovery cannot apply itself is known to be one before a partition
//! is touched.
//!
//! `update_metadata.proto` in Android's `update_engine` is the shape of it:
//! a fixed header, then a protobuf manifest, then the operations' data, in
//! the order the manifest points at it.

use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

/// The four bytes every payload starts with.
const MAGIC: &[u8; 4] = b"CrAU";

/// What a manifest measures both kinds of extent in. Every package this
/// recovery has met says 4096, which is what a page is.
pub const BLOCK: u64 = 4096;

/// The most is read for a manifest: a nine-gigabyte package's is under a
/// megabyte, and one larger than this is not one this recovery reads.
const MOST_MANIFEST: u64 = 32 << 20;

/// How many bytes of a header are read before the manifest is known to be
/// where it says: the four of the magic, and three numbers after them.
const HEADER: usize = 4 + 8 + 8 + 4;

/// How an operation brings its bytes into being, as
/// `InstallOperation.Type` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The bytes as they lie in the payload.
    Replace,
    /// The bytes as a bzip2 stream, one stream of its own per operation.
    /// The small partitions -- bluetooth, dtbo, recovery, the boot images --
    /// are mostly written this way, and they are what a package's first
    /// operations meet (2026-11-21).
    ReplaceBz,
    /// The bytes as an xz stream, one stream of its own per operation.
    ReplaceXz,
    /// No bytes: that much of the partition is written as zeroes.
    Zero,
    /// Nothing is written; the blocks are let go of.
    Discard,
    /// A kind this recovery does not apply itself: a move, a difference
    /// against the installed system, or a compression it has no decoder
    /// for. Named by its number, for the log.
    Other(u64),
}

impl Kind {
    /// The kinds, by the numbers `update_metadata.proto` gives them.
    fn of(number: u64) -> Kind {
        match number {
            0 => Kind::Replace,
            1 => Kind::ReplaceBz,
            8 => Kind::ReplaceXz,
            6 => Kind::Zero,
            7 => Kind::Discard,
            other => Kind::Other(other),
        }
    }

    /// Whether the operation carries bytes of its own in the payload.
    pub fn carries_data(self) -> bool {
        matches!(self, Kind::Replace | Kind::ReplaceBz | Kind::ReplaceXz)
    }

    /// The name `update_metadata.proto` gives it, for the log.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Replace => "REPLACE",
            Kind::ReplaceBz => "REPLACE_BZ",
            Kind::ReplaceXz => "REPLACE_XZ",
            Kind::Zero => "ZERO",
            Kind::Discard => "DISCARD",
            Kind::Other(_) => "another kind",
        }
    }
}

/// A stretch of a partition, in blocks of [`BLOCK`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub start_block: u64,
    pub blocks: u64,
}

impl Extent {
    pub fn bytes(&self) -> u64 {
        self.blocks * BLOCK
    }
}

/// One step of writing a partition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub kind: Kind,
    /// Where its bytes lie, counted from the start of the payload's data.
    pub data_offset: u64,
    pub data_length: u64,
    /// Where they go, in the order they follow one another.
    pub dst: Vec<Extent>,
    /// What the bytes must hash to, when the package says; empty when it
    /// does not.
    pub hash: Vec<u8>,
}

impl Operation {
    /// How many bytes of the partition this operation writes.
    pub fn bytes(&self) -> u64 {
        self.dst.iter().map(Extent::bytes).sum()
    }
}

/// A partition the package writes, and what it must be when all its
/// operations have run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionUpdate {
    pub name: String,
    pub operations: Vec<Operation>,
    /// The size the partition has in the new system, as the package says.
    pub new_size: u64,
    /// The hash of the whole partition when it is written, as the package
    /// says; empty when it does not say.
    pub new_hash: Vec<u8>,
    /// Whether the package asked for a script to run after the writing, on
    /// the new system. A recovery runs none of them.
    pub run_postinstall: bool,
    /// The version of the system this partition belongs to, as the package
    /// says; empty for a partition that has no version of its own.
    pub version: String,
}

impl PartitionUpdate {
    /// How many bytes of the partition its operations write.
    pub fn bytes(&self) -> u64 {
        self.operations.iter().map(Operation::bytes).sum()
    }
}

/// What the payload says to write, where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub partitions: Vec<PartitionUpdate>,
    /// What the manifest measures extents in, as it says: 4096 unless a
    /// package written for a phone with another page says otherwise.
    pub block_size: u32,
    /// `partial_update`: a package that writes only what changed, leaving
    /// the rest of super as it was, which this recovery does not put in
    /// itself.
    pub partial: bool,
    /// `dynamic_partition_metadata`, as the package wrote it: the layout of
    /// super the update asks for, when it asks for one. Read by the caller
    /// to tell whether the layout this phone has is the one the package was
    /// cut for.
    pub dynamic: Option<Vec<u8>>,
}

impl Manifest {
    /// The partition of this name, when the package writes it.
    pub fn partition(&self, name: &str) -> Option<&PartitionUpdate> {
        self.partitions.iter().find(|p| p.name == name)
    }

    /// How many bytes of partitions the whole payload writes.
    pub fn bytes(&self) -> u64 {
        self.partitions.iter().map(PartitionUpdate::bytes).sum()
    }
}

/// A package's payload, open for its operations' bytes to be read.
pub struct Payload {
    file: Mutex<File>,
    /// Where the payload's data begins: past the header, the manifest and
    /// the signature over it.
    data_start: u64,
    /// Where the payload ends in the file it lies in.
    end: u64,
    pub manifest: Manifest,
}

impl Payload {
    /// Read the payload of `len` bytes that begins at `at` in `path`: its
    /// header and its manifest, leaving the file open for the operations'
    /// bytes. Nothing is written.
    pub fn open(path: &Path, at: u64, len: u64) -> Result<Payload> {
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let end = at.checked_add(len).with_context(|| format!("{} ends beyond the file", path.display()))?;
        let mut header = [0u8; HEADER];
        file.seek(SeekFrom::Start(at)).with_context(|| format!("seek in {}", path.display()))?;
        file.read_exact(&mut header).with_context(|| format!("read the payload header of {}", path.display()))?;
        if &header[..4] != MAGIC {
            bail!("the payload does not start with CrAU");
        }
        let version = u64::from_be_bytes(header[4..12].try_into().expect("eight bytes"));
        if version != 2 {
            bail!("the payload is of format {version}, and only 2 is read here");
        }
        let manifest_size = u64::from_be_bytes(header[12..20].try_into().expect("eight bytes"));
        let signature_size = u64::from(u32::from_be_bytes(header[20..24].try_into().expect("four bytes")));
        if manifest_size > MOST_MANIFEST {
            bail!("the manifest is {manifest_size} bytes, past the {MOST_MANIFEST} this reads");
        }
        let mut manifest = vec![0u8; manifest_size as usize];
        file.read_exact(&mut manifest).with_context(|| format!("read the {manifest_size} bytes of the manifest"))?;
        let data_start = at + HEADER as u64 + manifest_size + signature_size;
        if data_start > end {
            bail!(
                "the manifest and its signature end at {data_start}, past the {len} bytes the payload has"
            );
        }
        Ok(Payload { file: Mutex::new(file), data_start, end, manifest: parse_manifest(&manifest)? })
    }

    /// Where the operations' bytes begin in the file.
    pub fn data_start(&self) -> u64 {
        self.data_start
    }

    /// Read `len` bytes of an operation's data, counted from the start of
    /// the payload's data, and give them back.
    pub fn read(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        let at = self.data_start + offset;
        if at + len > self.end {
            bail!("{} bytes at {offset} end at {}, past the payload's {}", len, at + len, self.end);
        }
        let mut buf = vec![0u8; len as usize];
        let mut file = self.file.lock().expect("the payload's file is not poisoned");
        file.seek(SeekFrom::Start(at)).with_context(|| format!("seek to {at} in the payload"))?;
        file.read_exact(&mut buf).with_context(|| format!("read {len} bytes at {at} in the payload"))?;
        Ok(buf)
    }
}

/// Read a payload's manifest, as `DeltaArchiveManifest` writes it.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    let mut manifest = Manifest { partitions: Vec::new(), block_size: BLOCK as u32, partial: false, dynamic: None };
    let mut at = 0;
    while let Some((field, value)) = field(bytes, &mut at)? {
        match (field, value) {
            (3, Value::Varint(n)) => manifest.block_size = u32::try_from(n).context("a block size past 32 bits")?,
            (13, Value::Bytes(body)) => manifest.partitions.push(parse_partition(body)?),
            (15, Value::Bytes(body)) => manifest.dynamic = Some(body.to_vec()),
            (16, Value::Varint(n)) => manifest.partial = n != 0,
            _ => {}
        }
    }
    Ok(manifest)
}

fn parse_partition(bytes: &[u8]) -> Result<PartitionUpdate> {
    let mut partition = PartitionUpdate {
        name: String::new(),
        operations: Vec::new(),
        new_size: 0,
        new_hash: Vec::new(),
        run_postinstall: false,
        version: String::new(),
    };
    let mut at = 0;
    while let Some((field, value)) = field(bytes, &mut at)? {
        match (field, value) {
            (1, Value::Bytes(body)) => partition.name = String::from_utf8_lossy(body).into_owned(),
            (2, Value::Varint(n)) => partition.run_postinstall = n != 0,
            (7, Value::Bytes(body)) => {
                let (size, hash) = parse_info(body)?;
                partition.new_size = size;
                partition.new_hash = hash;
            }
            (8, Value::Bytes(body)) => partition.operations.push(parse_operation(body)?),
            (17, Value::Bytes(body)) => partition.version = String::from_utf8_lossy(body).into_owned(),
            _ => {}
        }
    }
    if partition.name.is_empty() {
        bail!("a partition of the manifest has no name");
    }
    Ok(partition)
}

/// `PartitionInfo`: the size the partition has, and the hash of all of it.
fn parse_info(bytes: &[u8]) -> Result<(u64, Vec<u8>)> {
    let (mut size, mut hash) = (0u64, Vec::new());
    let mut at = 0;
    while let Some((field, value)) = field(bytes, &mut at)? {
        match (field, value) {
            (1, Value::Varint(n)) => size = n,
            (2, Value::Bytes(body)) => hash = body.to_vec(),
            _ => {}
        }
    }
    Ok((size, hash))
}

fn parse_operation(bytes: &[u8]) -> Result<Operation> {
    let mut operation = Operation { kind: Kind::Other(u64::MAX), data_offset: 0, data_length: 0, dst: Vec::new(), hash: Vec::new() };
    let mut number = 0u64;
    let mut at = 0;
    while let Some((field, value)) = field(bytes, &mut at)? {
        match (field, value) {
            (1, Value::Varint(n)) => number = n,
            (2, Value::Varint(n)) => operation.data_offset = n,
            (3, Value::Varint(n)) => operation.data_length = n,
            (6, Value::Bytes(body)) => operation.dst.push(parse_extent(body)?),
            // `data_sha256_hash` is the eighth field of an operation: the
            // hash of the bytes as they lie in the payload, before they are
            // decoded.
            (8, Value::Bytes(body)) => operation.hash = body.to_vec(),
            _ => {}
        }
    }
    operation.kind = Kind::of(number);
    if operation.dst.is_empty() {
        bail!("an operation writes no blocks");
    }
    Ok(operation)
}

fn parse_extent(bytes: &[u8]) -> Result<Extent> {
    let (mut start_block, mut blocks) = (0u64, 0u64);
    let mut at = 0;
    while let Some((field, value)) = field(bytes, &mut at)? {
        match (field, value) {
            (1, Value::Varint(n)) => start_block = n,
            (2, Value::Varint(n)) => blocks = n,
            _ => {}
        }
    }
    Ok(Extent { start_block, blocks })
}

/// What one field of a protobuf message holds.
enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed32,
    Fixed64,
}

/// The next field of a protobuf message, or `None` at its end. Fields of a
/// kind nothing here reads are stepped over, so a manifest written by a
/// newer update_engine still parses.
fn field<'a>(bytes: &'a [u8], at: &mut usize) -> Result<Option<(u64, Value<'a>)>> {
    if *at >= bytes.len() {
        return Ok(None);
    }
    let key = varint(bytes, at)?;
    let (number, wire) = (key >> 3, key & 0x7);
    if number == 0 {
        bail!("a field of the manifest is numbered 0");
    }
    let value = match wire {
        0 => Value::Varint(varint(bytes, at)?),
        1 => {
            let slice = take(bytes, at, 8)?;
            let _ = slice;
            Value::Fixed64
        }
        2 => {
            let len = varint(bytes, at)? as usize;
            Value::Bytes(take(bytes, at, len)?)
        }
        5 => {
            let slice = take(bytes, at, 4)?;
            let _ = slice;
            Value::Fixed32
        }
        other => bail!("a field of the manifest is of wire kind {other}"),
    };
    Ok(Some((number, value)))
}

fn take<'a>(bytes: &'a [u8], at: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = at.checked_add(len).context("a field runs past the end of the manifest")?;
    let slice = bytes.get(*at..end).context("a field runs past the end of the manifest")?;
    *at = end;
    Ok(slice)
}

fn varint(bytes: &[u8], at: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*at).context("a number runs past the end of the manifest")?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            bail!("a number of the manifest is longer than 64 bits");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a field with its tag, as protobuf does.
    fn tag(number: u64, wire: u64) -> Vec<u8> {
        let mut out = Vec::new();
        let mut key = (number << 3) | wire;
        while key >= 0x80 {
            out.push((key as u8 & 0x7f) | 0x80);
            key >>= 7;
        }
        out.push(key as u8);
        out
    }

    fn varint_field(number: u64, value: u64) -> Vec<u8> {
        let mut out = tag(number, 0);
        let mut v = value;
        while v >= 0x80 {
            out.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
        out
    }

    fn bytes_field(number: u64, body: &[u8]) -> Vec<u8> {
        let mut out = tag(number, 2);
        let mut len = body.len() as u64;
        while len >= 0x80 {
            out.push((len as u8 & 0x7f) | 0x80);
            len >>= 7;
        }
        out.push(len as u8);
        out.extend_from_slice(body);
        out
    }

    fn extent(start_block: u64, blocks: u64) -> Vec<u8> {
        let mut out = varint_field(1, start_block);
        out.extend(varint_field(2, blocks));
        out
    }

    #[test]
    fn a_manifest_comes_down_to_its_operations() {
        // Two operations: one xz over two extents, one zero over one.
        let mut xz = varint_field(1, 8);
        xz.extend(varint_field(2, 100));
        xz.extend(varint_field(3, 512));
        xz.extend(bytes_field(6, &extent(0, 2)));
        xz.extend(bytes_field(6, &extent(8, 1)));
        xz.extend(bytes_field(7, &[0xaa; 32]));

        let mut zero = varint_field(1, 6);
        zero.extend(bytes_field(6, &extent(16, 4)));

        let mut info = varint_field(1, 4096 * 7);
        info.extend(bytes_field(2, &[0xbb; 32]));

        let mut partition = bytes_field(1, b"odm");
        partition.extend(bytes_field(7, &info));
        partition.extend(bytes_field(8, &xz));
        partition.extend(bytes_field(8, &zero));
        partition.extend(bytes_field(17, b"4.0.8.0"));

        let manifest = bytes_field(13, &partition);
        let read = parse_manifest(&manifest).expect("parses");
        assert_eq!(read.partitions.len(), 1);
        let odm = &read.partitions[0];
        assert_eq!(odm.name, "odm");
        assert_eq!(odm.new_size, 4096 * 7);
        assert_eq!(odm.new_hash, vec![0xbb; 32]);
        assert_eq!(odm.version, "4.0.8.0");
        assert_eq!(odm.operations.len(), 2);

        let first = &odm.operations[0];
        assert_eq!(first.kind, Kind::ReplaceXz);
        assert_eq!(first.data_offset, 100);
        assert_eq!(first.data_length, 512);
        assert_eq!(first.dst, vec![Extent { start_block: 0, blocks: 2 }, Extent { start_block: 8, blocks: 1 }]);
        assert_eq!(first.bytes(), 3 * BLOCK);
        assert!(first.kind.carries_data());

        let second = &odm.operations[1];
        assert_eq!(second.kind, Kind::Zero);
        assert_eq!(second.bytes(), 4 * BLOCK);
        assert!(!second.kind.carries_data());
        assert_eq!(odm.bytes(), 7 * BLOCK);
        assert_eq!(read.bytes(), 7 * BLOCK);
    }

    #[test]
    fn a_kind_nothing_here_applies_is_kept_by_its_number() {
        let mut move_op = varint_field(1, 2);
        move_op.extend(bytes_field(6, &extent(0, 1)));
        let mut partition = bytes_field(1, b"vendor");
        partition.extend(bytes_field(8, &move_op));
        let read = parse_manifest(&bytes_field(13, &partition)).expect("parses");
        assert_eq!(read.partitions[0].operations[0].kind, Kind::Other(2));
    }

    #[test]
    fn a_manifest_of_no_partitions_is_read_as_none() {
        let read = parse_manifest(b"").expect("parses");
        assert!(read.partitions.is_empty());
        assert_eq!(read.bytes(), 0);
    }

    #[test]
    fn a_field_nothing_reads_is_stepped_over() {
        let mut partition = bytes_field(1, b"system");
        partition.extend(bytes_field(4, b"ext4"));
        partition.extend(varint_field(2, 1));
        partition.extend(bytes_field(99, &[1, 2, 3, 4]));
        let read = parse_manifest(&bytes_field(13, &partition)).expect("parses");
        assert_eq!(read.partitions[0].name, "system");
        assert!(read.partitions[0].run_postinstall);
    }

    #[test]
    fn a_manifest_that_runs_past_its_end_is_refused() {
        let mut broken = bytes_field(13, b"");
        broken.pop();
        assert!(parse_manifest(&broken).is_err());
    }
}
