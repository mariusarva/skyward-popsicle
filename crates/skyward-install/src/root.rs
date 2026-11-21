//! Keeping root across an update. An update writes the boot images of the
//! slot it goes into over, and the root the phone had lived in one of them:
//! Magisk's and KernelSU's LKM in the ramdisk of `init_boot`, KernelSU's GKI
//! and APatch's in the kernel of `boot`.
//!
//! Both keep the patcher that put them there, behind their own folders on the
//! data partition: Magisk's `boot_patch.sh` with `magiskboot`, and KernelSU's
//! `ksud boot-patch`, which carries the module and the init it writes. The
//! image the update wrote is patched again with its own tool and written back
//! before the phone restarts — the same thing each manager does when it
//! installs to the slot an update is about to start from, done here because
//! the update was done here.

use crate::image;
use anyhow::{bail, ensure, Context, Result};
use skyward_vab::Slot;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where each root solution keeps what its install left, under the data
/// partition.
const MAGISK: &str = "adb/magisk";
const APATCH: &str = "adb/ap";

/// Where KernelSU's own tool is, under the data partition, newer installs
/// first: `ksud` is kept beside the module folders by the manager, and
/// inside them by installs that came before it (2026-11-20).
const KSUD: [&str; 5] = ["adb/ksud", "adb/ksu/bin/ksud", "adb/ksu/ksud", "adb/ksud.real", "adb/ksu/bin/ksud.real"];

/// Where a root's manager keeps what it needs under the data partition,
/// whichever of them is there: a phone with one of these is rooted, and the
/// image it runs from carries that root, whether or not the recovery can
/// name the tool that put it there (2026-11-20).
const ROOT_FOLDERS: [&str; 5] = ["adb/ksu", "adb/ksud", "adb/magisk", "adb/ap", "adb/apd"];

/// Where a patcher's `magiskboot` may be, when the patcher does not carry one.
const MAGISKBOOT: [&str; 3] = ["/data/adb/magisk/magiskboot", "/data/adb/ksu/bin/magiskboot", "/data/adb/ksu/magiskboot"];

/// The shells a recovery has, the first one there used to run a script.
const SHELLS: [&str; 5] = ["/system/bin/sh", "/bin/sh", "/sbin/sh", "/system/xbin/sh", "/data/adb/magisk/busybox"];

/// The partitions root can live in, in the order Android 13 and after puts
/// it: the generic ramdisk in `init_boot` first, a phone without one in the
/// ramdisk of `boot`.
const ROOT_PARTS: [&str; 2] = ["init_boot", "boot"];

/// Where the partition metadata of a phone that keeps preinit data lives,
/// which Magisk's patcher wants named in what it writes.
const METADATA: &str = "/dev/block/by-name/metadata";

/// The root a phone has, by what its data partition `data` holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    /// Magisk, whose tools are in this folder.
    Magisk { tools: PathBuf },
    /// KernelSU, whose tool patches the image by itself: where it is.
    KernelSu { ksud: PathBuf },
    /// APatch, which patches the kernel with a tool its manager keeps.
    Apatch,
    /// None the recovery knows how to name.
    None,
}

impl Root {
    /// What root the phone has, under the data partition `data`.
    pub fn found(data: &Path) -> Root {
        let magisk = data.join(MAGISK);
        if magisk.join("boot_patch.sh").is_file() && magisk.join("magiskboot").is_file() {
            return Root::Magisk { tools: magisk };
        }
        for name in KSUD {
            let ksud = data.join(name);
            if ksud.is_file() {
                return Root::KernelSu { ksud };
            }
        }
        if data.join(APATCH).is_dir() {
            return Root::Apatch;
        }
        Root::None
    }

    /// Whether the phone is rooted at all, by what its manager left under
    /// the data partition, whatever the recovery can make of the tool: the
    /// image the phone runs from carries the root it has, and it goes into
    /// the slot that starts next whether or not a patcher was found
    /// (2026-11-20).
    pub fn rooted(data: &Path) -> bool {
        ROOT_FOLDERS.iter().any(|folder| data.join(folder).exists())
    }

    /// How the page names it.
    pub fn name(&self) -> &'static str {
        match self {
            Root::Magisk { .. } => "Magisk",
            Root::KernelSu { .. } => "KernelSU",
            Root::Apatch => "APatch",
            // A phone whose root is there but whose tool the recovery cannot
            // name: the image is carried over just the same (2026-11-20).
            Root::None => "root",
        }
    }

    /// Whether the recovery can put this root back itself: Magisk and
    /// KernelSU both leave the tool that patched the image on the phone,
    /// APatch's stays in its manager.
    pub fn patchable(&self) -> bool {
        matches!(self, Root::Magisk { .. } | Root::KernelSu { .. })
    }
}

/// What the work on the boot image of the slot that starts next is doing,
/// for the page to show as it goes: the image the update wrote there is
/// read out, patched with the root's own tool, written back, and read back
/// again to be sure it landed (2026-11-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Story {
    /// The partition is read out into the recovery's own folder.
    Reading,
    /// The root's own patcher runs on it: the only step with no bytes to
    /// count, a second or less of unpacking and packing.
    Patching,
    /// The patched image goes back into the partition.
    Writing,
    /// The head of what was written is read back and compared.
    Checking,
}

/// How the work tells where it is: the step, and how many of its bytes are
/// done of how many there are, both nought where the step has none.
pub type Tell<'a> = &'a mut dyn FnMut(Story, u64, u64);

/// A boot image that root was patched back into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The partition it went into.
    pub partition: &'static str,
    /// The slot that holds it.
    pub slot: Slot,
    /// What root was put back, as the page names it.
    pub what: &'static str,
    /// The slot whose image went in whole, where that is how root was
    /// kept: the one the phone runs from, whose image still has the root
    /// that works. `None` when the image the update wrote was patched
    /// again instead.
    pub from: Option<Slot>,
}

/// What became of the phone's root as the update was installed, for the page
/// to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rooted {
    /// Root was patched back into the boot image the update wrote.
    Kept(Kept),
    /// The phone's root is one whose manager keeps the patcher: its name,
    /// which has to be installed again from that manager.
    Other { what: &'static str },
    /// It is one the recovery patches, and the patch did not happen: the
    /// log says why, and its manager has to patch the image by hand.
    Missed { what: &'static str },
    /// The phone had no root before the update, so none was taken away.
    None,
}

/// Put root back into the boot image of `slot`: the image the update wrote
/// there is read, patched with the tool the phone's own root left, and
/// written back. `work` is a folder the recovery always has room in.
pub fn keep(dir: &Path, slot: Slot, root: &Root, work: &Path, tell: Tell) -> Result<Kept> {
    let (partition, device) = root_partition(dir, slot)?;
    fs::create_dir_all(work).with_context(|| format!("make {}", work.display()))?;

    let original = work.join(format!("{partition}{}.img", slot.suffix()));
    let holds = size_of(&device).unwrap_or(0);
    copy_partition(&device, &original, holds, &mut *tell)?;
    let have = fs::metadata(&original).with_context(|| format!("look at {}", original.display()))?.len();
    ensure!(have > 0, "{} holds nothing", device.display());
    skyward_sys::kmsg(&format!(
        "root: the {partition} of the {slot:?} slot is {have} bytes; patching it again with {}",
        root.name()
    ))?;

    let patched = match root {
        Root::Magisk { tools } => {
            tell(Story::Patching, 0, 0);
            patch_with_magisk(tools, &original, work)?
        }
        Root::KernelSu { ksud } => {
            tell(Story::Patching, 0, 0);
            patch_with_ksud(ksud, &original, work)?
        }
        other => bail!("{} keeps its patcher in its own manager, not on the phone", other.name()),
    };
    let made = fs::metadata(&patched).with_context(|| format!("look at {}", patched.display()))?.len();
    ensure!(made > 0, "{} wrote an empty image", root.name());
    ensure!(made <= have, "the patched image is {made} bytes, more than the {have} the partition holds");

    // What the patched image leaves of the partition is written over with
    // zeroes: a signature footer left there would have the bootloader check
    // an image that is no longer the one it signs, and stop the phone before
    // it starts (2026-11-20).
    let mut padded = Vec::with_capacity(have as usize);
    {
        let mut file = fs::File::open(&patched).with_context(|| format!("open {}", patched.display()))?;
        io::copy(&mut file, &mut padded).with_context(|| format!("read {}", patched.display()))?;
    }
    padded.resize(have as usize, 0);

    tell(Story::Writing, 0, have);
    image::flash(&mut io::Cursor::new(&padded[..]), have, &[device.clone()], &mut |done, total| {
        tell(Story::Writing, done, total)
    })
    .with_context(|| format!("write the patched image into {}", device.display()))?;
    tell(Story::Checking, 0, have);
    let back = read_back(&device, have)?;
    ensure!(
        back == padded[..back.len().min(padded.len())],
        "what {} holds is not what was written to it",
        device.display()
    );
    tell(Story::Checking, have, have);
    skyward_sys::kmsg(&format!(
        "root: the patched image went into {} ({} bytes), made by {}, and reads back as it was written",
        device.display(),
        have,
        root.name()
    ))?;

    Ok(Kept { partition, slot, what: root.name(), from: None })
}

/// Put the boot image the phone runs from into `slot`: that image holds
/// the root the phone has now, module and init together, and the update
/// wrote the other slot's image over. Copying the partition whole keeps
/// that root as it is, without unpacking and packing the image again and
/// without a patcher having to recognise the system it came from
/// (2026-11-20).
///
/// The image is written over the `slot`'s partition, zeroes to its end: a
/// footer left past the image would have the bootloader check an image
/// that is no longer the one it signs.
pub fn carry(dir: &Path, slot: Slot, root: &Root, tell: Tell) -> Result<Kept> {
    let (partition, from) = root_partition(dir, slot.other())?;
    let (_, into) = root_partition(dir, slot)?;
    // How much the partition holds, asked of the device itself: what a
    // block device says of itself as a file is nothing, so a size read
    // that way is zero for every partition on the phone and nothing is
    // ever carried over (2026-11-21).
    let have = size_of(&into)?;
    ensure!(have > 0, "{} holds nothing", into.display());
    let mut image = Vec::new();
    read_partition(&from, &mut image, &mut *tell)?;
    ensure!(!image.is_empty(), "{} holds nothing", from.display());
    ensure!(
        image.len() as u64 <= have,
        "{} holds {} bytes, more than the {have} of {}",
        from.display(),
        image.len(),
        into.display()
    );
    image.resize(have as usize, 0);
    skyward_sys::kmsg(&format!(
        "root: the {partition} of the {:?} slot goes into the {:?} slot whole, {} bytes, so that {} keeps as it is",
        slot.other(),
        slot,
        have,
        root.name()
    ))?;
    image::flash(&mut io::Cursor::new(&image[..]), have, &[into.clone()], &mut |done, total| {
        tell(Story::Writing, done, total)
    })
    .with_context(|| format!("write {} into {}", from.display(), into.display()))?;
    // A write that the device took and did not keep is the failure this
    // leaves behind: the partition is read back and compared with what was
    // written, so that the page says root is back only where it is
    // (2026-11-20).
    tell(Story::Checking, 0, have);
    let back = read_back(&into, have)?;
    ensure!(
        back == image[..back.len().min(image.len())],
        "what {} holds is not what was written to it",
        into.display()
    );
    tell(Story::Checking, have, have);
    skyward_sys::kmsg(&format!("root: {} reads back as it was written", into.display()))?;
    Ok(Kept { partition, slot, what: root.name(), from: Some(slot.other()) })
}

/// The first `have` bytes of `device`, as the partition holds them now, up
/// to a page of them: enough to tell a write that landed from one that did
/// not, without reading a hundred megabytes back.
fn read_back(device: &Path, have: u64) -> Result<Vec<u8>> {
    /// How much is read back: the image's own head, which every boot image
    /// starts with and no two of them share.
    const BACK: u64 = 1 << 20;
    let mut file = fs::File::open(device).with_context(|| format!("open {}", device.display()))?;
    let mut back = vec![0u8; have.min(BACK) as usize];
    file.read_exact(&mut back).with_context(|| format!("read {} back", device.display()))?;
    Ok(back)
}

/// How many bytes `device` holds. A block device says nothing of itself as
/// a file -- `fs::metadata` gives it no length at all -- so the size is
/// asked of the device, and only a plain file of the same name falls back
/// to what the file says (2026-11-21).
fn size_of(device: &Path) -> Result<u64> {
    if let Ok(size) = skyward_block::size_bytes(device)
        && size > 0
    {
        return Ok(size);
    }
    let said = fs::metadata(device).with_context(|| format!("look at {}", device.display()))?.len();
    ensure!(said > 0, "{} holds nothing", device.display());
    Ok(said)
}

/// Put the root the phone has into the boot image of `slot`, the slot that
/// starts next: the image the update wrote there is patched again with the
/// phone's own tool, and where that cannot be done, the image the phone runs
/// from is carried over whole. `data` is the open data partition, `dir` the
/// folder the phone's partitions are named in, `work` a folder the recovery
/// always has room in, and `tell` hears each step as it goes. What the page
/// says afterwards comes back (2026-11-21).
pub fn put_back(data: &Path, dir: &Path, slot: Slot, work: &Path, tell: Tell) -> Rooted {
    let found = Root::found(data);
    // A manager that left its folders under /data is a rooted phone,
    // whatever the recovery makes of the tool it left: what matters is that
    // the image the phone runs from has the root in it, and that image is
    // the one that goes into the slot that starts next.
    if !(found.patchable() || Root::rooted(data)) {
        let _ = skyward_sys::kmsg("root: the phone has no root the recovery knows of; nothing is put back");
        return Rooted::None;
    }
    let _ = skyward_sys::kmsg(&format!(
        "root: the phone's root is {}, and the {slot:?} slot it goes into holds {}",
        found.name(),
        dir.display()
    ));
    // APatch patches the kernel with a tool its manager keeps, not one the
    // partitions hold: the page names it, for it to be installed again from
    // there.
    if let Root::Apatch = found {
        let what = found.name();
        let _ = skyward_sys::kmsg(&format!(
            "root: the phone's root is {what}, whose patcher its manager keeps; it has to be installed again from there"
        ));
        return Rooted::Other { what };
    }
    // The image the update wrote into the slot that starts next is patched
    // again with the phone's own tool. It is that slot's own image, made for
    // the system that starts from it, so what comes out is that system with
    // the root the phone has now. Carrying the running slot's image over
    // instead puts a ramdisk made for another version of the system in front
    // of it, which is only worth the risk where patching is not possible at
    // all (2026-11-21).
    if found.patchable() {
        match keep(dir, slot, &found, work, &mut *tell) {
            Ok(kept) => {
                let _ = skyward_sys::kmsg(&format!(
                    "root: back in the {} of the {:?} slot, patched by {}",
                    kept.partition, kept.slot, kept.what
                ));
                return Rooted::Kept(kept);
            }
            Err(e) => {
                let _ = skyward_sys::kmsg(&format!(
                    "root: the image the update wrote is not patched again ({e:#}); the one the phone runs from is carried over instead"
                ));
            }
        }
    }
    // The image the phone is running from has this root in it, patched by
    // the manager that installed it: the whole partition goes into the slot
    // that starts next, and the phone starts from it with the root it has
    // now.
    match carry(dir, slot, &found, &mut *tell) {
        Ok(kept) => {
            let _ = skyward_sys::kmsg(&format!(
                "root: the {:?} slot's {} went into the {:?} slot whole, and reads back as it was written",
                kept.from.unwrap_or(slot.other()),
                kept.partition,
                kept.slot
            ));
            Rooted::Kept(kept)
        }
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("root: the image the phone runs from is not carried over either ({e:#})"));
            // Neither way landed: what the update wrote stays, which starts
            // the phone without root, and the page says which manager
            // patches a boot image by hand. Nothing is left on the phone for
            // a cable to flash: a second boot image under Skyward's folder is
            // one more to keep straight, and the image that would be copied
            // is the one the phone already runs from (2026-11-21).
            Rooted::Missed { what: found.name() }
        }
    }
}

/// The partition root lives in on this phone, in `slot`, and where it is.
fn root_partition(dir: &Path, slot: Slot) -> Result<(&'static str, PathBuf)> {
    for name in ROOT_PARTS {
        let device = image::device(dir, name, slot);
        if device.exists() {
            return Ok((name, device));
        }
    }
    bail!("this phone has neither init_boot nor boot in the {slot:?} slot")
}

/// Patch `image` with Magisk's own patcher, in `work`, and give the image it
/// wrote.
fn patch_with_magisk(tools: &Path, image: &Path, work: &Path) -> Result<PathBuf> {
    // The tools are copied out of the data partition first: the patcher
    // reads them more than once, and the partition may be let go under it.
    let bin = work.join("magisk");
    let _ = fs::remove_dir_all(&bin);
    copy_tree(tools, &bin).with_context(|| format!("copy {} into {}", tools.display(), bin.display()))?;
    let script = bin.join("boot_patch.sh");
    ensure!(script.is_file(), "{} has no boot_patch.sh", tools.display());

    let patched = run_magisk_patcher(&script, &bin, image)?;
    ensure!(patched.is_file(), "Magisk's patcher wrote no new-boot.img");
    Ok(patched)
}

/// Run Magisk's boot patcher on `image`, in `bin`, the folder its tools were
/// copied into: it leaves the patched image there as `new-boot.img`.
fn run_magisk_patcher(script: &Path, bin: &Path, image: &Path) -> Result<PathBuf> {
    let shell = SHELLS
        .iter()
        .copied()
        .find(|shell| Path::new(shell).exists())
        .with_context(|| format!("the recovery has no shell among {SHELLS:?} to run Magisk's patcher with"))?;
    // Magisk's patcher reads all of its settings from the environment: the
    // phone's verification and the data partition's encryption stay as they
    // are, the recovery's own mode is off, and the preinit device is the one
    // this phone keeps its metadata in. Turning verification off would keep
    // HyperOS from starting at all.
    let preinit = if Path::new(METADATA).exists() { "metadata" } else { "" };
    // The patcher asks the phone what it is through getprop, and a recovery
    // answers little: the answers it needs are given to it, in front of the
    // phone's own programs on the path.
    let shims = bin.join("shims");
    let path = write_shims(Path::new(shell), &shims)?;
    let mut command = Command::new(shell);
    command.arg(script).arg(image);
    command.current_dir(bin);
    command.env("KEEPVERITY", "true");
    command.env("KEEPFORCEENCRYPT", "true");
    command.env("PATCHVBMETAFLAG", "false");
    command.env("RECOVERYMODE", "false");
    command.env("LEGACYSAR", "false");
    command.env("PREINITDEVICE", preinit);
    command.env("MAGISKBIN", bin);
    command.env("LC_ALL", "C");
    command.env("PATH", path);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let out = command.output().with_context(|| format!("run {} with {}", script.display(), shell))?;
    let said = format!(
        "{} {}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let _ = skyward_sys::kmsg(&format!("root: Magisk's patcher ended as {}: {}", out.status, said.trim()));
    ensure!(out.status.success(), "Magisk's patcher ended as {}: {}", out.status, said.trim());
    Ok(bin.join("new-boot.img"))
}

/// Patch `image` with KernelSU's own `ksud`, which carries the module and
/// the init it writes, and give the image it wrote.
fn patch_with_ksud(ksud: &Path, image: &Path, work: &Path) -> Result<PathBuf> {
    // `ksud` takes `-o` for a folder, not for a file, and writes the image
    // inside it under a name of its own. Given a file's name it unpacks and
    // repacks the image and then fails to write it out at all ("write out
    // new boot failed: No such file or directory", 2026-11-21). The folder
    // is emptied first, so what is found in it after is what this run wrote.
    let out = work.join("ksu");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).with_context(|| format!("make {}", out.display()))?;
    let mut command = Command::new(ksud);
    command.arg("boot-patch").arg("-b").arg(image).arg("-o").arg(&out);
    // A `magiskboot` of Magisk's, where `ksud` carries none and takes to be
    // told where it is: only a build that names the flag is given it, since
    // one that does not ends on an argument it does not know.
    if let Some(boot) = MAGISKBOOT.iter().map(Path::new).find(|path| path.is_file())
        && takes_magiskboot(ksud)
    {
        command.arg("--magiskboot").arg(boot);
    }
    command.current_dir(&out);
    command.env("LC_ALL", "C");
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let done = command.output().with_context(|| format!("run {}", ksud.display()))?;
    let said = format!(
        "{} {}",
        String::from_utf8_lossy(&done.stdout).trim(),
        String::from_utf8_lossy(&done.stderr).trim()
    );
    let _ = skyward_sys::kmsg(&format!("root: ksud boot-patch ended as {}: {}", done.status, said.trim()));
    ensure!(done.status.success(), "ksud boot-patch ended as {}: {}", done.status, said.trim());

    // Whatever it named the image, it is the newest one it left in the
    // folder it was given.
    newest_image(&out).with_context(|| format!("ksud boot-patch wrote no image into {}", out.display()))
}

/// Whether this build of `ksud` takes `--magiskboot`: asked of it, since a
/// flag it does not know ends the run before it starts.
fn takes_magiskboot(ksud: &Path) -> bool {
    let help = Command::new(ksud)
        .args(["boot-patch", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match help {
        Ok(out) => {
            let said = format!("{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            said.contains("--magiskboot")
        }
        Err(_) => false,
    }
}

/// The image in `dir` written last.
fn newest_image(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "img"))
        .max_by_key(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok())
}

/// What a recovery's own programs do not answer, put where the patcher looks
/// first: the shell it runs with is `shell`, and what comes back is the path
/// the patcher is run with.
fn write_shims(shell: &Path, at: &Path) -> Result<String> {
    const ANSWERS: [(&str, &str); 8] = [
        ("ro.build.version.sdk", "36"),
        ("ro.build.version.release", "16"),
        ("ro.build.type", "user"),
        ("ro.product.cpu.abi", "arm64-v8a"),
        ("ro.product.cpu.abilist", "arm64-v8a,armeabi-v7a,armeabi"),
        ("ro.product.cpu.abilist64", "arm64-v8a"),
        ("ro.product.cpu.abilist32", "armeabi-v7a,armeabi"),
        ("ro.product.device", "popsicle"),
    ];
    let _ = fs::remove_dir_all(at);
    fs::create_dir_all(at).with_context(|| format!("make {}", at.display()))?;
    let mut script = format!("#!{}\ncase \"$1\" in\n", shell.display());
    for (name, answer) in ANSWERS {
        script.push_str(&format!("  {name}) echo {answer} ;;\n"));
    }
    // Anything else is answered as a recovery's own would be: with nothing,
    // which the patcher takes for what the phone does not say.
    script.push_str("  *) exit 1 ;;\nesac\n");
    let getprop = at.join("getprop");
    fs::write(&getprop, script.as_bytes()).with_context(|| format!("write {}", getprop.display()))?;
    make_runnable(&getprop)?;
    Ok(format!("{}:/sbin:/system/bin:/system/xbin", at.display()))
}

/// Make `at` something the shell runs.
#[cfg(unix)]
fn make_runnable(at: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(at, fs::Permissions::from_mode(0o755)).with_context(|| format!("make {} runnable", at.display()))
}

/// Nothing: a system that keeps no such thing as a file's mode.
#[cfg(not(unix))]
fn make_runnable(_at: &Path) -> Result<()> {
    Ok(())
}

/// Copy the files under `from` into `to`, keeping what each can be run as.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("make {}", to.display()))?;
    for entry in fs::read_dir(from).with_context(|| format!("read {}", from.display()))? {
        let entry = entry.with_context(|| format!("read {}", from.display()))?;
        let path = entry.path();
        let into = to.join(entry.file_name());
        let kind = entry.file_type().with_context(|| format!("look at {}", path.display()))?;
        if kind.is_dir() {
            copy_tree(&path, &into)?;
        } else if kind.is_file() {
            fs::copy(&path, &into).with_context(|| format!("copy {} to {}", path.display(), into.display()))?;
            keep_mode(&path, &into)?;
        }
    }
    Ok(())
}

/// Give `into` what `from` can be run as, where the system has such a thing.
#[cfg(unix)]
fn keep_mode(from: &Path, into: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(from).with_context(|| format!("look at {}", from.display()))?.permissions().mode();
    fs::set_permissions(into, fs::Permissions::from_mode(mode))
        .with_context(|| format!("keep what {} can be run as", into.display()))
}

/// Nothing: a system that keeps no such thing as a file's mode.
#[cfg(not(unix))]
fn keep_mode(_from: &Path, _into: &Path) -> Result<()> {
    Ok(())
}

/// Read `device` whole into `image`, telling how many of its bytes are read
/// as they come: an image is a few megabytes and the page says so while it
/// is read (2026-11-21).
fn read_partition(device: &Path, image: &mut Vec<u8>, tell: Tell) -> Result<()> {
    let total = size_of(device).unwrap_or(0);
    tell(Story::Reading, 0, total);
    let mut file = fs::File::open(device).with_context(|| format!("open {}", device.display()))?;
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut chunk).with_context(|| format!("read {}", device.display()))?;
        if read == 0 {
            return Ok(());
        }
        image.extend_from_slice(&chunk[..read]);
        tell(Story::Reading, image.len() as u64, total.max(image.len() as u64));
    }
}

/// Read `device` into `at`, as many bytes as it holds, telling how many are
/// copied as they come.
fn copy_partition(device: &Path, at: &Path, total: u64, tell: Tell) -> Result<()> {
    tell(Story::Reading, 0, total);
    let mut from = fs::File::open(device).with_context(|| format!("open {}", device.display()))?;
    let mut to = fs::File::create(at).with_context(|| format!("create {}", at.display()))?;
    let mut chunk = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        let read = from.read(&mut chunk).with_context(|| format!("read {}", device.display()))?;
        if read == 0 {
            break;
        }
        to.write_all(&chunk[..read]).with_context(|| format!("write {}", at.display()))?;
        done += read as u64;
        tell(Story::Reading, done, total.max(done));
    }
    to.flush().with_context(|| format!("flush {}", at.display()))?;
    to.sync_all().with_context(|| format!("flush {}", at.display()))
}

/// Keep the patched image on internal storage as well, where a computer can
/// flash it back if what went into the partition does not start.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_without_root_is_named_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Root::found(dir.path()), Root::None);
        assert!(!Root::None.patchable());
    }

    #[test]
    fn magisk_is_the_tools_its_install_left() {
        let dir = tempfile::tempdir().unwrap();
        let tools = dir.path().join("adb/magisk");
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("boot_patch.sh"), b"#!/system/bin/sh\n").unwrap();
        std::fs::write(tools.join("magiskboot"), b"not really\n").unwrap();
        assert_eq!(Root::found(dir.path()), Root::Magisk { tools: tools.clone() });
        assert_eq!(Root::Magisk { tools }.name(), "Magisk");
    }

    #[test]
    fn magisk_without_its_patcher_is_no_magisk_the_recovery_can_use() {
        let dir = tempfile::tempdir().unwrap();
        let tools = dir.path().join("adb/magisk");
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("magiskboot"), b"not really\n").unwrap();
        assert_eq!(Root::found(dir.path()), Root::None);
    }

    #[test]
    fn kernelsu_is_the_tool_it_patches_with() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("adb/ksu/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("ksud"), b"not really\n").unwrap();
        let found = Root::found(dir.path());
        assert_eq!(found, Root::KernelSu { ksud: bin.join("ksud") });
        assert_eq!(found.name(), "KernelSU");
        assert!(found.patchable());
    }

    #[test]
    fn a_kernelsu_older_than_the_bin_folder_is_found_too() {
        let dir = tempfile::tempdir().unwrap();
        let ksu = dir.path().join("adb/ksu");
        std::fs::create_dir_all(&ksu).unwrap();
        std::fs::write(ksu.join("ksud"), b"not really\n").unwrap();
        assert_eq!(Root::found(dir.path()), Root::KernelSu { ksud: ksu.join("ksud") });
    }

    #[test]
    fn apatch_is_named_by_the_folder_it_keeps_and_its_manager_patches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("adb/ap")).unwrap();
        let found = Root::found(dir.path());
        assert_eq!(found, Root::Apatch);
        assert_eq!(found.name(), "APatch");
        assert!(!found.patchable());
    }

    #[test]
    fn the_partition_root_lives_in_is_init_boot_where_there_is_one() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("init_boot_b"), b"init\n").unwrap();
        std::fs::write(dir.path().join("boot_b"), b"boot\n").unwrap();
        let (name, at) = root_partition(dir.path(), Slot::B).unwrap();
        assert_eq!(name, "init_boot");
        assert_eq!(at, dir.path().join("init_boot_b"));
    }

    #[test]
    fn a_phone_without_init_boot_has_root_in_boot() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("boot_a"), b"boot\n").unwrap();
        let (name, at) = root_partition(dir.path(), Slot::A).unwrap();
        assert_eq!(name, "boot");
        assert_eq!(at, dir.path().join("boot_a"));
    }

    #[test]
    fn a_phone_with_neither_has_nowhere_to_put_root() {
        let dir = tempfile::tempdir().unwrap();
        assert!(root_partition(dir.path(), Slot::A).is_err());
    }

    #[test]
    fn a_plain_file_says_how_much_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().join("init_boot_b.img");
        std::fs::write(&at, vec![0u8; 4096]).unwrap();
        assert_eq!(size_of(&at).unwrap(), 4096);
    }

    #[test]
    fn a_file_with_nothing_in_it_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().join("init_boot_b.img");
        std::fs::write(&at, b"").unwrap();
        assert!(size_of(&at).is_err());
    }

    #[test]
    fn the_shim_answers_what_a_recovery_leaves_out() {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().join("shims");
        let path = write_shims(Path::new("/system/bin/sh"), &at).unwrap();
        assert!(path.starts_with(at.to_str().unwrap()));
        let script = std::fs::read_to_string(at.join("getprop")).unwrap();
        assert!(script.starts_with("#!/system/bin/sh\n"));
        assert!(script.contains("ro.product.cpu.abi) echo arm64-v8a"));
        assert!(script.contains("ro.build.version.sdk) echo 36"));
    }
}
