//! Block devices as the kernel holds them: whether one is marked
//! read-only, and marking it writable again so that a write does not come
//! back `EPERM`.
//!
//! A partition is marked read-only by `BLKROSET`, not by its mount: an
//! update that wrote the other slot leaves the partitions it touched with
//! that flag on some phones, and a plain `open(O_RDWR)` then succeeds
//! while every `write` fails with `Operation not permitted`.

use anyhow::{Context, Result};
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// `_IO(0x12, 93)`: mark the device read-only, or writable with 0.
/// From `<linux/fs.h>`, the same on every Android ABI.
const BLKROSET: libc::Ioctl = 0x125d;
/// `_IO(0x12, 94)`: 1 when the device is read-only, 0 when writable.
const BLKROGET: libc::Ioctl = 0x125e;

fn ioctl(file: &File, request: libc::Ioctl, arg: *mut libc::c_void) -> io::Result<()> {
    // `libc::Ioctl` is the request type each C library declares: bionic and
    // musl take a c_int, glibc an unsigned long. The bits are the same; only
    // the width differs, so the constants above are typed by it rather than
    // cast at every call, which is what lets this crate build on a Linux host
    // as well as a phone.
    if unsafe { libc::ioctl(file.as_raw_fd(), request, arg) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Whether `file` is a block device the kernel has marked read-only.
pub fn read_only(file: &File) -> Result<bool> {
    let mut ro: libc::c_int = 0;
    ioctl(file, BLKROGET, &mut ro as *mut libc::c_int as *mut libc::c_void)
        .context("ask whether the device is marked read-only")?;
    Ok(ro != 0)
}

/// Mark the block device `file` writable; `true` when it had been marked
/// read-only and is writable now, `false` when it was writable already.
pub fn make_writable(file: &File) -> Result<bool> {
    if !read_only(file)? {
        return Ok(false);
    }
    let mut ro: libc::c_int = 0;
    ioctl(file, BLKROSET, &mut ro as *mut libc::c_int as *mut libc::c_void)
        .context("mark the device writable again")?;
    Ok(true)
}

/// Where the block device at `path` really is, for a log line: a link in
/// `/dev/block/by-name` leads to the partition node or to a mapped device.
pub fn real_path(path: &Path) -> String {
    std::fs::canonicalize(path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string())
}

/// Where a partition node lies on its disk, for a write that the
/// partition itself refuses: the disk's node, and the first byte of the
/// partition in it. None when `path` is no partition of a disk -- a whole
/// disk, or a device `dm` maps, which has no place of its own to write.
pub fn on_disk(path: &Path) -> Result<Option<(PathBuf, u64)>> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).with_context(|| format!("look at {}", path.display()))?;
    let dev = meta.rdev();
    let (major, minor) = (libc::major(dev), libc::minor(dev));
    let entry = match std::fs::canonicalize(format!("/sys/dev/block/{major}:{minor}")) {
        Ok(entry) => entry,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("look up {major}:{minor} under /sys/dev/block")),
    };
    // Only a partition has this file; a whole disk and a device `dm` maps
    // do not.
    if !entry.join("partition").exists() {
        return Ok(None);
    }
    let start: u64 = std::fs::read_to_string(entry.join("start"))
        .with_context(|| format!("read the start of {major}:{minor}"))?
        .trim()
        .parse()
        .with_context(|| format!("read the start of {major}:{minor}"))?;
    let disk = entry.parent().with_context(|| format!("find the disk of {major}:{minor}"))?;
    let ddev = std::fs::read_to_string(disk.join("dev")).with_context(|| format!("read the device number of {}", disk.display()))?;
    let (dmaj, dmin) = ddev
        .trim()
        .split_once(':')
        .with_context(|| format!("read the device number of {}", disk.display()))?;
    let node = PathBuf::from(format!("/dev/block/{dmaj}:{dmin}"));
    // The kernel counts partition starts in 512-byte sectors, whatever the
    // disk's own sector size is.
    Ok(Some((node, start * 512)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of the test's own, named so that runs do not collide.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("skyward-block-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn a_plain_file_is_no_block_device() {
        let path = scratch("blkdev-plain");
        let file = File::open(&path).unwrap();
        // A regular file has no block device behind it, so the kernel
        // refuses the request rather than answering it.
        assert!(read_only(&file).is_err());
        assert!(make_writable(&file).is_err());
    }

    #[test]
    fn the_real_path_of_a_plain_file_is_the_file() {
        let path = scratch("blkdev-path");
        assert_eq!(real_path(&path), std::fs::canonicalize(&path).unwrap().display().to_string());
        let missing = path.with_file_name("blkdev-not-there");
        assert_eq!(real_path(&missing), missing.display().to_string());
    }
}
