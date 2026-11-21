//! update_engine_sideload, the phone's own program that applies an A/B
//! update package: how it is started, and what it reports while it works.

use crate::package::Update;
use std::process::Stdio;

/// Where the recovery ramdisk keeps it.
pub const PROGRAM: &str = "/system/bin/update_engine_sideload";

/// Its arguments for `update`, read from the file `package`; it writes
/// its status lines to `status_fd`.
pub fn arguments(package: &str, update: &Update, status_fd: i32) -> Vec<String> {
    vec![
        format!("--payload=file://{package}"),
        format!("--offset={}", update.payload_offset),
        format!("--headers={}", update.properties),
        format!("--status_fd={status_fd}"),
    ]
}

/// Its arguments for `update`, with whatever this build of it takes to do
/// the work faster: `tuning` from [`tuning`], which is empty where it takes
/// nothing.
pub fn arguments_with(package: &str, update: &Update, status_fd: i32, tuning: &[String]) -> Vec<String> {
    let mut args = arguments(package, update, status_fd);
    args.extend_from_slice(tuning);
    args
}

/// What this build of update_engine takes to write a payload faster, for
/// the arguments of [`arguments_with`]: asked of the program itself, since
/// the recovery runs the phone's own build and a flag it does not know is
/// a package that does not install at all.
///
/// The one that matters is `--max_threads`, which takes the payload's
/// operations apart on more than one core: a nine-gigabyte package is
/// minutes of one core decompressing it (2026-11-20).
pub fn tuning() -> Vec<String> {
    let help = match std::process::Command::new(PROGRAM)
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(out) => format!("{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)),
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("update_engine's own flags are not asked for: {e}"));
            return Vec::new();
        }
    };
    let mut tuning = Vec::new();
    if help.contains("--max_threads") {
        // One core writing and the rest taking the operations apart; more
        // than the clusters' own cores only costs the reading thread time.
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let threads = cores.clamp(2, 6);
        tuning.push(format!("--max_threads={threads}"));
    }
    let _ = skyward_sys::kmsg(&format!(
        "update_engine takes {} of the flags this recovery knows to ask for",
        match tuning.len() {
            0 => "none".to_string(),
            _ => tuning.join(" "),
        }
    ));
    tuning
}

/// How many cores the phone has, for the log.
pub fn cores() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0)
}

/// Lines that say why a package was written at the speed it was, kept in
/// the recovery's own log because they are written once and the log the
/// program keeps itself goes with the recovery's memory filesystem.
///
/// `Worker thread count:` is how many threads libsnapshot's copy-on-write
/// writer was given -- one, on a phone whose `prop.default` names none,
/// and every write of a nine-gigabyte payload waits behind it; `Not using
/// snapshot` says the payload went straight over the partitions instead
/// (2026-11-21).
pub fn notable(line: &str) -> Option<String> {
    const MARKS: [&str; 6] = [
        "Worker thread count: ",
        "Num verification threads: ",
        "Not using snapshot on VAB device",
        "thread used for compression",
        "multi-threaded compression for VABC",
        "Unable to create snapshot",
    ];
    for mark in MARKS {
        if let Some(at) = line.find(mark) {
            return Some(line[at..].trim().chars().take(160).collect());
        }
    }
    None
}

/// A line it writes to the status descriptor that matters here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Step 1 writes the partitions; step 2 runs the package's last steps,
    /// after the written partitions are read back.
    Step(u32),
    /// It failed with this error code.
    Error(i32),
}

pub fn status(line: &str) -> Option<Status> {
    let text = line.strip_prefix("ui_print ")?;
    if let Some(step) = text.strip_prefix("Step ") {
        return step.split_once('/')?.0.parse().ok().map(Status::Step);
    }
    let code = text.strip_prefix("Error applying update: ")?;
    code.split_whitespace().next()?.parse().ok().map(Status::Error)
}

/// What its log on stderr tells beyond the status lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Log {
    /// A partition the payload writes, listed once before writing starts.
    Partition(String),
    /// Super has no room for snapshots, so it writes over the partitions
    /// of the slot the phone runs from.
    Overwriting,
    /// It started reading back the `index`th partition it wrote, from 0.
    Hashing { index: usize, partition: String },
    /// It has applied `done` of the payload's `all` operations: the finest
    /// count it gives, and the one the page follows line by line.
    Applied { done: u32, all: u32 },
    /// It has taken `done` of the payload's `total` bytes, as it counts
    /// them itself -- every thirty seconds or so, and not once more.
    Moved { done: u64, total: u64 },
    /// How far it says the whole payload has come, in percent: the one
    /// count of its own that weighs every operation by what it writes, so
    /// a package of small partitions and one of nine gigabytes both read
    /// right. The bytes downloaded reach their end long before the work
    /// does, which is why the bar follows this instead (2026-11-20).
    Overall(u32),
}

/// Everything one line tells. The operation count and the byte count of
/// all of it come on the same line, and the page follows both, so a line
/// may give more than one thing.
pub fn logs(line: &str) -> Vec<Log> {
    let mut said = Vec::new();
    if let Some(rest) = after(line, "PartitionInfo new ") {
        if let Some((name, _)) = rest.split_once(' ') {
            said.push(Log::Partition(name.to_string()));
        }
    }
    if line.contains("Will overwrite existing partitions.") {
        said.push(Log::Overwriting);
    }
    if let Some(rest) = after(line, "Hashing partition ") {
        if let Some((index, rest)) = rest.split_once(" (")
            && let Some((name, _)) = rest.split_once(')')
            && let Ok(index) = index.parse()
        {
            said.push(Log::Hashing { index, partition: name.to_string() });
        }
    }
    // `Completed 631/5820 operations (10%), 814596096/9199913504 bytes
    // downloaded (8%), overall progress 9%`: how many of the payload's
    // operations are applied, and how many of its bytes are taken. A count
    // of `?` operations says nothing of how many there are.
    if let Some(rest) = after(line, "Completed ") {
        if let Some((pair, _)) = rest.split_once(" operations") {
            if let Some((done, all)) = pair.split_once('/') {
                if let (Ok(done), Ok(all)) = (done.trim().parse(), all.trim().parse()) {
                    said.push(Log::Applied { done, all });
                }
            }
        }
    }
    if let Some((head, _)) = line.split_once(" bytes downloaded") {
        if let Some((pair, _)) = head.rsplit_once(", ") {
            if let Some((done, total)) = pair.split_once('/') {
                if let (Ok(done), Ok(total)) = (done.trim().parse(), total.trim().parse()) {
                    said.push(Log::Moved { done, total });
                }
            }
        }
    }
    // `overall progress 9%`: how far the payload has come, weighed by what
    // each operation writes. It comes on the same line as the two counts
    // above, and is the one the bar follows.
    if let Some(rest) = after(line, "overall progress ") {
        if let Some((percent, _)) = rest.split_once('%') {
            if let Ok(percent) = percent.trim().parse() {
                said.push(Log::Overall(percent));
            }
        }
    }
    said
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker).map(|at| &line[at + marker.len()..])
}

/// What an update_engine error code comes down to for whoever installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// The package is not signed with the keys this phone trusts.
    Signature,
    /// The package is damaged.
    Damaged,
    /// It is older than what the recovery allows.
    Older,
    /// Super has no room for the new partitions.
    NoSpace,
    /// Reading the package failed: the computer stopped sending.
    Transfer,
    /// Writing a partition failed.
    Write,
    /// The written partitions read back wrong.
    Verify,
    /// The package's own last step failed.
    PostInstall,
    /// The package is in a format this update_engine does not know.
    Unsupported,
    Other,
}

impl Trouble {
    pub fn of(code: i32) -> Trouble {
        match code {
            12 | 18 | 22 | 24 | 25 | 26 | 33 | 39 => Trouble::Signature,
            10 | 11 | 21 | 23 | 27 | 28 | 29 | 32 | 38 => Trouble::Damaged,
            51 => Trouble::Older,
            60 => Trouble::NoSpace,
            9 => Trouble::Transfer,
            7 | 14 | 20 => Trouble::Write,
            15 | 16 | 47 | 56 => Trouble::Verify,
            5 | 63 => Trouble::PostInstall,
            6 | 17 | 44 | 45 => Trouble::Unsupported,
            _ => Trouble::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::Metadata;

    #[test]
    fn the_command_line_is_the_one_recovery_builds() {
        let update = Update {
            payload_offset: 2365,
            payload_size: 7_103_041_280,
            properties: "FILE_HASH=abc=\nFILE_SIZE=7103041280\n".into(),
            metadata: Metadata::default(),
        };
        assert_eq!(
            arguments("/sideload/package.zip", &update, 9),
            [
                "--payload=file:///sideload/package.zip",
                "--offset=2365",
                "--headers=FILE_HASH=abc=\nFILE_SIZE=7103041280\n",
                "--status_fd=9",
            ]
        );
    }

    #[test]
    fn status_lines_give_steps_and_errors() {
        assert_eq!(status("ui_print Step 1/2"), Some(Status::Step(1)));
        assert_eq!(status("ui_print Step 2/2"), Some(Status::Step(2)));
        assert_eq!(
            status("ui_print Error applying update: 51 (ErrorCode::kPayloadTimestampError)"),
            Some(Status::Error(51))
        );
        assert_eq!(status("progress 0.5 0"), None);
        assert_eq!(status("set_progress 1"), None);
    }

    #[test]
    fn the_log_names_partitions_and_the_one_being_read_back() {
        assert_eq!(
            logs("[INFO:delta_performer.cc(267)] PartitionInfo new vendor_boot sha256: 3f8a size: 100663296"),
            [Log::Partition("vendor_boot".into())]
        );
        assert_eq!(
            logs("[INFO:filesystem_verifier_action.cc(377)] Hashing partition 3 (odm) on device /dev/block/dm-12"),
            [Log::Hashing { index: 3, partition: "odm".into() }]
        );
        assert_eq!(
            logs("[INFO:dynamic_partition_control_android.cc(1349)] Will overwrite existing partitions. Slot B may be unbootable until update finishes!"),
            [Log::Overwriting]
        );
        assert_eq!(logs("[INFO:delta_performer.cc(267)] PartitionInfo old odm sha256: 00 size: 4"), []);
        assert_eq!(
            logs("[INFO:delta_performer.cc(108)] Completed 10/400 operations (2%)"),
            [Log::Applied { done: 10, all: 400 }]
        );
        // The same line gives the operations applied, the bytes taken and
        // how far the whole payload has come: the bar follows the last, the
        // lines under it the operations (2026-11-20).
        assert_eq!(
            logs("[INFO:delta_performer.cc(108)] Completed 631/5820 operations (10%), 814596096/9199913504 bytes downloaded (8%), overall progress 9%"),
            [Log::Applied { done: 631, all: 5820 }, Log::Moved { done: 814_596_096, total: 9_199_913_504 }, Log::Overall(9)]
        );
        assert_eq!(
            logs("[INFO:delta_performer.cc(108)] Completed 0/? operations, 16384/9199913504 bytes downloaded (0%), overall progress 0%"),
            [Log::Moved { done: 16_384, total: 9_199_913_504 }, Log::Overall(0)]
        );
    }

    #[test]
    fn error_codes_come_down_to_their_trouble() {
        assert_eq!(Trouble::of(26), Trouble::Signature);
        assert_eq!(Trouble::of(51), Trouble::Older);
        assert_eq!(Trouble::of(9), Trouble::Transfer);
        assert_eq!(Trouble::of(60), Trouble::NoSpace);
        assert_eq!(Trouble::of(1), Trouble::Other);
    }

    #[test]
    fn the_lines_that_say_why_it_was_slow_are_picked_out() {
        assert_eq!(
            notable("[INFO:snapshot_writer.cc(120)] Worker thread count: 8"),
            Some("Worker thread count: 8".to_string())
        );
        assert_eq!(
            notable("[INFO:snapshot.cpp(1)] Not using snapshot on VAB device because sideloading."),
            Some("Not using snapshot on VAB device because sideloading.".to_string())
        );
        assert_eq!(notable("[INFO:delta_performer.cc(108)] Completed 10/400 operations (2%)"), None);
        assert_eq!(notable(""), None);
    }
}
