//! The screen lock a user chose to have kept, so that the data partition
//! opens as the recovery starts rather than waiting for it to be typed.
//!
//! It is kept where the recovery keeps its own settings, readable by root
//! alone. A factory reset formats that partition, and so does the way out of
//! the decrypt page that forgets it.

use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// The partition the file is on. The recovery mounts it read-only, as the
/// phone's own recovery does, so it is made writable for as long as this
/// takes and then read-only again.
const METADATA: &str = "/metadata";

/// Where it is kept: beside the recovery's own settings.
pub const FILE: &str = "/metadata/skyward/credential";

/// The screen lock kept from before, where one was kept.
///
/// /metadata is not mounted yet when the pages are built -- the recovery mounts
/// it read-only for the moment it takes to read its settings, and takes it away
/// again -- so it is mounted here the same way. Read without that, the file is
/// never found and the partition waits to be typed every start (2026-11-15).
pub fn read() -> Option<Vec<u8>> {
    let at = Path::new(METADATA);
    let mounted = skyward_block::mount::is_mount_point(at).unwrap_or(false);
    if !mounted && skyward_block::mount::mount_ro(&Path::new(skyward_block::BY_NAME).join("metadata"), at).is_err() {
        return None;
    }
    // A file that is empty, or longer than any credential a phone takes, is
    // not one: tried, it would only be held against the user by the secure
    // world counting it.
    let kept = std::fs::read(FILE).ok().filter(|kept| !kept.is_empty() && kept.len() <= 512);
    if !mounted {
        let _ = skyward_block::mount::umount(at);
    }
    kept
}

/// Do `work` with the partition writable, and read-only again afterwards.
fn writable<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    let at = Path::new(METADATA);
    // The partition is mounted read-only only for the moment its settings
    // are read, and may not be mounted at all by the time this runs: a write
    // or a removal under an unmounted /metadata lands in the empty folder
    // the mount point is, reads as done, and leaves the file on the
    // partition untouched. That is how a screen lock the secure world no
    // longer takes kept being tried at every start (2026-11-19).
    let mounted = skyward_block::mount::is_mount_point(at)?;
    if !mounted {
        skyward_block::mount::mount_ro(&Path::new(skyward_block::BY_NAME).join("metadata"), at)
            .context("mount /metadata to write the recovery's own settings")?;
    }
    let done = (|| {
        skyward_block::mount::remount_writable(at, true).context("make /metadata writable")?;
        work()
    })();
    let _ = skyward_block::mount::remount_writable(at, false);
    if !mounted {
        let _ = skyward_block::mount::umount(at);
    }
    done
}

/// Keep `credential` for the starts that follow.
pub fn write(credential: &[u8]) -> Result<()> {
    writable(|| {
    let at = Path::new(FILE);
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("make {}", dir.display()))?;
    }
    std::fs::write(at, credential).with_context(|| format!("write {}", at.display()))?;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("keep {} to root alone", at.display()))?;
    Ok(())
    })
}

/// Forget the screen lock kept from before, if there is one.
pub fn forget() -> Result<()> {
    writable(|| match std::fs::remove_file(FILE) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("remove {FILE}")),
    })
}

/// Where the mark that the switch was turned off lies: a phone that has one
/// keeps no lock, and keeps none at the next start either.
pub const OPTED_OUT: &str = "/metadata/skyward/no-remember";

/// Where the mark that the kept lock is being tried lies. A start that finds
/// it was cut off while the lock was being tried — the process ended, the
/// phone was restarted, the secure world never answered — and asks for the
/// lock itself rather than try the same one again. Without it, a lock the
/// secure world will not answer holds every start at the same page
/// (2026-11-19).
pub const TRYING: &str = "/metadata/skyward/trying";

/// Whether the start before this one was cut off while the kept lock was
/// being tried. The mark is taken away as it is read, so that it costs one
/// start and no more.
pub fn take_trying() -> bool {
    let at = Path::new(METADATA);
    let mounted = skyward_block::mount::is_mount_point(at).unwrap_or(false);
    if !mounted && skyward_block::mount::mount_ro(&Path::new(skyward_block::BY_NAME).join("metadata"), at).is_err() {
        return false;
    }
    let mark = Path::new(TRYING);
    let was = mark.is_file();
    if was {
        let _ = skyward_block::mount::remount_writable(at, true);
        let _ = std::fs::remove_file(mark);
        let _ = skyward_block::mount::remount_writable(at, false);
    }
    if !mounted {
        let _ = skyward_block::mount::umount(at);
    }
    was
}

/// Leave the mark that the kept lock is being tried.
pub fn mark_trying() -> Result<()> {
    writable(|| std::fs::write(TRYING, b"").with_context(|| format!("write {TRYING}")))
}

/// Take the mark away: the trial ended, opened or not.
pub fn done_trying() {
    let _ = writable(|| match std::fs::remove_file(TRYING) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("remove {TRYING}")),
    });
}

/// Leave the mark that the switch was turned off.
pub fn opt_out() -> std::io::Result<()> {
    let _ = skyward_block::mount::remount_writable(std::path::Path::new("/metadata"), true);
    let written = std::fs::write(OPTED_OUT, b"");
    let _ = skyward_block::mount::remount_writable(std::path::Path::new("/metadata"), false);
    written
}

/// Take the mark away: the switch was turned on.
pub fn opt_in() -> std::io::Result<()> {
    let _ = skyward_block::mount::remount_writable(std::path::Path::new("/metadata"), true);
    let taken = std::fs::remove_file(OPTED_OUT).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) });
    let _ = skyward_block::mount::remount_writable(std::path::Path::new("/metadata"), false);
    taken
}
