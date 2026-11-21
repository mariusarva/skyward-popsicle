//! Merging an update into the slot the phone runs from, as libsnapshot
//! follows a merge: started where it has not begun, then looked at every
//! two seconds until it ends, the bootloader told where it stands through
//! the phone's own boot control. On a thread of its own for the screen.

use anyhow::{Context, Result};
use skyward_binder::driver::Binder;
use skyward_sys::worker::Worker;
use skyward_vab::boot::BootControl;
use skyward_vab::{Merge, MergeStatus, Slot};
use std::os::fd::RawFd;
use std::thread;
use std::time::Duration;

/// How often a merge under way is looked at, as libsnapshot looks.
const LOOK_EVERY: Duration = Duration::from_secs(2);

/// Merge the update into `slot`, starting the merge first where `start`,
/// and follow it to its end, `each` told how every look at it went.
/// /metadata is writable while it runs, and read-only again after.
pub fn run(slot: Slot, start: bool, each: &mut dyn FnMut(&Merge)) -> Result<Merge> {
    // The bootloader is told where the update stands through the phone's
    // boot control where it runs; where none runs, the same word goes into
    // misc, which is where that control keeps it. A merge that cannot say
    // where it stands is not left undone for that (2026-11-20).
    let binder = Binder::open();
    let hardware = Binder::hardware();
    let control = match (binder.as_ref(), hardware.as_ref()) {
        (Ok(binder), Ok(hardware)) => BootControl::open(binder, hardware).ok(),
        _ => None,
    };
    let misc = skyward_sys::bcb::misc();
    let tell = |status: MergeStatus| -> Result<()> {
        if let Some(control) = &control
            && control.set_merge_status(status).is_ok()
        {
            return Ok(());
        }
        skyward_sys::bcb::set_merge_status(&misc, status as u8)
    };
    skyward_vab::metadata_writable(true).context("make /metadata writable to merge the update")?;
    let merged = (|| -> Result<Merge> {
        if start {
            // A merge takes every snapshot of the slot at once, and one
            // that was never mapped cannot be merged at all: a phone that
            // restarted before the merge has mapped none of them, so they
            // are mapped here rather than after a page has mounted
            // something (2026-11-20).
            skyward_vab::map_snapshots(slot).context("map the update's snapshots to merge them")?;
            skyward_vab::start_merge(slot, &tell)?;
        }
        loop {
            let merge = skyward_vab::check_merge(slot, &tell)?;
            each(&merge);
            if !matches!(merge, Merge::Going(_)) {
                return Ok(merge);
            }
            thread::sleep(LOOK_EVERY);
        }
    })();
    let back = skyward_vab::metadata_writable(false).context("make /metadata read-only again");
    let merge = merged?;
    back?;
    Ok(merge)
}

/// Give the update into `slot` up, as [`run`] tells the bootloader where a
/// merge stands: the slot starts from its own partitions again, without
/// what the update wrote, rather than never starting at all (2026-11-20).
pub fn cancel(slot: Slot) -> Result<()> {
    let binder = Binder::open();
    let hardware = Binder::hardware();
    let control = match (binder.as_ref(), hardware.as_ref()) {
        (Ok(binder), Ok(hardware)) => BootControl::open(binder, hardware).ok(),
        _ => None,
    };
    let misc = skyward_sys::bcb::misc();
    let tell = |status: MergeStatus| -> Result<()> {
        if let Some(control) = &control
            && control.set_merge_status(status).is_ok()
        {
            return Ok(());
        }
        skyward_sys::bcb::set_merge_status(&misc, status as u8)
    };
    skyward_vab::metadata_writable(true).context("make /metadata writable to give the update up")?;
    let given = skyward_vab::cancel(slot, &tell);
    let back = skyward_vab::metadata_writable(false).context("make /metadata read-only again");
    given?;
    back
}

pub enum Event {
    /// How a look at the merge went.
    Looked(Merge),
    /// The merge could not be started or followed, and why.
    Failed(String),
}/// A merge followed on a thread of its own.
pub struct Merger {
    /// The screen lets it go once the merge has ended.
    worker: Worker<Event>,
}

impl Merger {
    /// Follow the merge into `slot` as [`run`] does, starting it first where
    /// `start`.
    pub fn start(slot: Slot, start: bool) -> Result<Merger> {
        let worker = Worker::start("merge", move |tell| {
            if let Err(e) = run(slot, start, &mut |merge| {
                tell.tell(Event::Looked(*merge));
            }) {
                tell.tell(Event::Failed(format!("{e:#}")));
            }
        })?;
        Ok(Merger { worker })
    }

    /// Readable while events wait.
    pub fn fd(&self) -> RawFd {
        self.worker.fd()
    }

    /// The events since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.worker.events()
    }

    /// Whether the merge is no longer followed: it ended, or failed.
    pub fn ended(&self) -> bool {
        self.worker.ended()
    }
}
