use crate::backup;
use crate::decrypt::{self, Decryptor};
use crate::filework;
use crate::haptics::Haptics;
use crate::relabel;
use crate::input::{monotonic_ms, Event, Inputs};
use crate::merge;
use crate::partwork;
use crate::pty;
use crate::readings::{Reading, Readings};
use crate::secure;
use crate::services;
use crate::settings;
use crate::systems;
use crate::updates;
use crate::wipe::{self, Eraser, Step};
use crate::fetch;
use crate::wlan;
use anyhow::{anyhow, ensure, Context, Result};
use skyward_block::fsck::Fixed;
use skyward_block::{fstab, mount, probe};
use skyward_config::{DeviceConfig, Service};
use skyward_crypto::data::{Closed, Opened};
use skyward_install::asked::{self, Needs};
use skyward_install::{Detach, Installer, Options};
use skyward_sys::bcb::{self, Work, REFORMATS};
use skyward_sys::control::{Control, Request};
use skyward_sys::Power;
use skyward_files::Ownership;
use skyward_block::usb::{self, Drive};
use skyward_net::download::{self, Failure};
use skyward_net::http::Servers;
use skyward_net::release::{Installed, Release, Unanswered};
use skyward_net::wlan::{Power as WlanPower, Stage as Joining, Wlan};
use skyward_sys::sleep::{Sleeper, Slept};
use skyward_sys::uevent;
use skyward_systems::Who;
use skyward_ui::app::{
    App, Asked, BackupAt, BackupEnd, BackupEntry, BackupRefusal, ClashChoice, Data, DownloadEnd, Effect, EraseStop, FileKind, FileOp,
    FileRow, FileTask, FsAction, FsRow, Have, Held, ItemEnd, ItemState, JobEnd, Keys, MountFailure, Part, PartKind, PartStop, Place,
    RenameRefused, Screen, Shot, SkywardEnd, SkywardStage, Target, UsbDrive, UsbLink, Wipe, DATA_OPENING, DATA_PARTITION, DOWNLOADS,
    INSTALL_UNFINISHED, OPEN_LOST, REMEMBER_SLOW, USB_DRIVE,
};
use skyward_ui::settings::Keeper;
use skyward_ui::surface::kms::KmsSurface;
use skyward_ui::{avatar, Canvas, Fonts, Lang, Surface};
use skyward_vab::{Merge, Standing};
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long init may take to hand the USB port to another function: the
/// program behind the new one has to start and get ready first.
const USB_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the page shows that the work the system asked for is done
/// before the phone starts the system or powers off.
const LEAVE_AFTER_MS: u64 = 1500;

/// How long the page shows what became of an update once it has merged:
/// the merge runs for minutes on the page that followed it, and the page
/// that says the phone can start is read before it does (2026-11-20).

/// How long after the screen starts a touch panel may still come up; with
/// none by then, the pages are worked with the keys, their light shown.
/// The panel took 5.1 s to appear on this phone, and a wait of 5 s said the
/// pages were to be worked with the keys a fiftieth of a second before it
/// came (v50's log, 2026-11-19): it is given room to spare now.
const TOUCH_WAIT: Duration = Duration::from_secs(10);

/// How long a panel gone dark rests, ready to light again at once, before
/// the display turns off; the phone's own system keeps its panel dozing
/// about as long after the screen goes off.
const REST: Duration = Duration::from_secs(5);

/// How long the screen lock kept from before may take in the secure world
/// before the page stops waiting on it. A right one comes back in a moment;
/// one the phone counts as wrong is held for half a minute or longer, and a
/// page waiting on that is a page that never gets in (2026-11-19).
const REMEMBER_WAIT: Duration = Duration::from_secs(15);

/// How often a job that takes long says in the kernel log where it stands.
const OPENING_SAID_EVERY: Duration = Duration::from_secs(10);

/// Where logs saved to the temporary folder go, in the recovery service's
/// own folder, and where in internal storage the others do.
const TMP_LOGS: &str = "/skyward/run/logs";
const STORAGE_LOGS: &str = "Skyward/logs";
/// Where a system app's package goes when it is exported to internal storage.
const STORAGE_APKS: &str = "Skyward/apps";

/// The folder of internal storage the system saves its own screenshots in,
/// which the gallery shows as its screenshots, and where in the recovery
/// service's own folder a screenshot waits for internal storage to open.
const SCREENSHOTS: &str = "DCIM/Screenshots";
/// 录屏写在内部存储的这个文件夹里，和相册里的截图并排。
const RECORDINGS: &str = "DCIM/Screenrecords";
const HELD_SCREENSHOTS: &str = "/skyward/run/screenshots";

/// The panel backlight under /sys/class/backlight.
struct Backlight {
    path: PathBuf,
    /// The raw level at the top of the slider: the panel's normal maximum.
    top: u32,
    /// The raw level last written; at first the one the bootloader left.
    level: u32,
}

impl Backlight {
    fn open() -> Result<Self> {
        let mut dirs: Vec<PathBuf> = fs::read_dir("/sys/class/backlight")
            .context("read /sys/class/backlight")?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<_>>()?;
        dirs.sort();
        // Qualcomm panels expose panel0-backlight; take it when present.
        let dir = dirs
            .iter()
            .find(|d| d.file_name().is_some_and(|n| n == "panel0-backlight"))
            .or(dirs.first())
            .ok_or_else(|| anyhow!("no backlight device"))?
            .clone();
        let read = |name: &str| -> Result<u32> {
            fs::read_to_string(dir.join(name))
                .with_context(|| format!("read {}/{name}", dir.display()))?
                .trim()
                .parse()
                .with_context(|| format!("parse {name}"))
        };
        let top = normal_max(read("max_brightness")?)?;
        let level = read("brightness")?;
        Ok(Backlight { path: dir.join("brightness"), top, level })
    }

    /// Light the panel at `level` of its normal maximum, or turn it off
    /// with `None`. The file is written only when the raw level changes.
    fn show(&mut self, level: Option<f32>) -> Result<()> {
        let raw = level.map_or(0, |v| ((v.clamp(0.0, 1.0) * self.top as f32).round() as u32).max(1));
        if raw != self.level {
            fs::write(&self.path, raw.to_string()).with_context(|| format!("write {}", self.path.display()))?;
            eprintln!("backlight {raw}");
            self.level = raw;
        }
        Ok(())
    }
}

/// How a panel's node in the device tree may give its normal maximum: the
/// property holding it, and the one holding the whole range on the same
/// scale. The 13 Ultra's panel gives both on a finer scale of its own that
/// spans the same light as the backlight's range; the 17 Ultra's gives them
/// on the backlight's own scale. The panels of the Xiaomi 13 and 15, of the
/// 14 and the K80 series and one of the K90's give it only as the level
/// their factory tests drive the panel to, on the backlight's own scale:
/// the 13 Ultra's panels give that level as well, and it comes to the same
/// half of their range as the normal maximum they give.
const NORMAL_MAX: [(&str, &str); 3] = [
    ("mi,normal-max-brightness-clone", "mi,max-brightness-clone"),
    ("mi,max-brightness-normal", "qcom,mdss-dsi-bl-max-level"),
    ("mi,mdss-dsi-fac-bl-max-level", "qcom,mdss-dsi-bl-max-level"),
];

/// The raw backlight level, out of `max`, at the panel's normal maximum.
/// Above it the panel runs in high brightness mode, which the stock system
/// keeps for strong sunlight and its slider does not reach. The panel the
/// display driver booted with gives that maximum in the device tree.
fn normal_max(max: u32) -> Result<u32> {
    let param = "/sys/module/msm_drm/parameters/dsi_display0";
    let boot = fs::read_to_string(param).with_context(|| format!("read {param}"))?;
    // The panel's node, then its configuration after a colon, as in
    // "qcom,mdss_dsi_m1_42_02_0a_dsc_cmd:"; the driver takes no name
    // without the colon either.
    let (name, _) = boot.trim().split_once(':').with_context(|| format!("{param} names no panel: {boot:?}"))?;
    let soc = Path::new("/proc/device-tree/soc");
    let node = fs::read_dir(soc)
        .with_context(|| format!("read {}", soc.display()))?
        .map(|e| e.map(|e| e.path().join(name)))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .find(|p| p.is_dir())
        .ok_or_else(|| anyhow!("panel {name} is not in the device tree"))?;
    let cell = |prop: &str| -> Result<u32> {
        let path = node.join(prop);
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let cell: [u8; 4] = bytes.as_slice().try_into().map_err(|_| anyhow!("{} is not one cell", path.display()))?;
        Ok(u32::from_be_bytes(cell))
    };
    let (normal, full) = NORMAL_MAX
        .iter()
        .find(|(normal, _)| node.join(normal).exists())
        .with_context(|| format!("panel {name} gives its normal maximum as none of {NORMAL_MAX:?}"))?;
    let (normal, full) = (cell(normal)?, cell(full)?);
    ensure!(normal > 0 && normal <= full, "panel {name}: normal maximum {normal} out of {full}");
    Ok((f64::from(max) * f64::from(normal) / f64::from(full)).round() as u32)
}

/// Leave recovery for the chosen target.
fn power(target: Target) -> Result<()> {
    skyward_sys::power(match target {
        Target::System => Power::System,
        Target::Recovery => Power::Recovery,
        Target::Fastboot => Power::Fastboot,
        Target::Bootloader => Power::Bootloader,
        Target::Edl => Power::Edl,
        Target::PowerOff => Power::Off,
    })
}

/// File transfer: the service offering internal storage to a computer over
/// USB, whether it offers the storage now, and the zone, by its place in
/// the settings, it tells the times of files in.
struct Transfer {
    service: skyward_mtp::Service,
    offered: bool,
    zone: usize,
}

impl Transfer {
    /// Serve file transfer; the port offers it from when it next rests.
    fn start(cfg: &DeviceConfig, info: &skyward_info::DeviceInfo, zone: usize) -> Result<Transfer> {
        let device = skyward_mtp::Device {
            manufacturer: cfg.device.brand.clone(),
            model: info.model.clone(),
            version: format!("Skyward {}", skyward_info::RUNNING),
            serial: skyward_sys::prop("ro.serialno").unwrap_or_default(),
            name: info.model.clone(),
            battery: Box::new(|| skyward_info::battery_now().ok().flatten().map(|b| b.level)),
        };
        let service = skyward_mtp::Service::start(device, skyward_ui::fmt::rules(zone).clone())?;
        skyward_sys::usb::set_mtp(true);
        Ok(Transfer { service, offered: false, zone })
    }

    /// Offer internal storage, named for the computer in `lang`, or take it
    /// back. Once this returns, nothing the computer had under way in it
    /// goes on.
    fn offer(&mut self, offer: bool, lang: Lang) -> Result<()> {
        if offer != self.offered {
            let description = skyward_ui::fmt::storage_name(lang).into();
            let storage = offer.then(|| skyward_mtp::Storage { top: PathBuf::from(skyward_crypto::data::SDCARD), description });
            self.service.offer(storage)?;
            self.offered = offer;
        }
        Ok(())
    }

    /// Stop serving: a port at rest goes back to adb alone first.
    fn stop(self, rests: bool) -> Result<()> {
        skyward_sys::usb::set_mtp(false);
        let switched = if rests { skyward_sys::usb::switch("adb", USB_TIMEOUT) } else { Ok(()) };
        let stopped = self.service.stop();
        switched.and(stopped)
    }
}

/// Take internal storage back from file transfer, if it is on, before work
/// that erases or rewrites the data partition, or installs; whether that
/// work can start, which the page is told otherwise.
fn withhold(transfer: &mut Option<Transfer>, app: &mut App, now: u64) -> Result<bool> {
    let Some(t) = transfer else { return Ok(true) };
    match t.offer(false, app.state.lang) {
        Ok(()) => Ok(true),
        Err(e) => {
            skyward_sys::kmsg(&format!("file transfer kept internal storage: {e:#}"))?;
            app.report(&skyward_ui::fmt::transfer_kept(app.state.lang, &format!("{e:#}")), now);
            Ok(false)
        }
    }
}

/// Bring the port to offer file transfer beside adb, as `resting` asks for.
/// A port that will not take it is given back to adb alone: the switch takes
/// it through "none" first, so adb goes with it, and a port left there has
/// neither. The page says that in its own words, and the whole of why -- with
/// what the port's properties hold -- goes to the log, where it belongs: it
/// stood under the bar of the decrypt page in English until now (2026-11-19).
fn take_file_transfer(app: &mut App, now: u64) -> Result<()> {
    let Err(e) = skyward_sys::usb::switch(skyward_sys::usb::resting(), USB_TIMEOUT) else { return Ok(()) };
    skyward_sys::kmsg(&format!(
        "the port did not take file transfer: {e:#}; config={:?} state={:?} ffs.ready={:?} ffs.mtp.ready={:?} configfs={:?}",
        skyward_sys::prop("sys.usb.config"),
        skyward_sys::prop("sys.usb.state"),
        skyward_sys::prop("sys.usb.ffs.ready"),
        skyward_sys::prop("sys.usb.ffs.mtp.ready"),
        skyward_sys::prop("sys.usb.configfs"),
    ))?;
    if let Err(back) = skyward_sys::usb::switch("adb", USB_TIMEOUT) {
        skyward_sys::kmsg(&format!("the port did not come back to adb either: {back:#}"))?;
    }
    app.report(&skyward_ui::fmt::transfer_port_kept(app.state.lang), now);
    Ok(())
}

/// Whether the phone's fastbootd runs, as init tells it, and how far a
/// computer has the port, as the USB controller tells it.
fn fastboot_status() -> Result<(bool, UsbLink)> {
    let running = skyward_sys::prop("init.svc.fastbootd").as_deref() == Some("running");
    Ok((running, usb_link()?))
}

/// Light the panel, as on a cable going in or out, when the battery as it
/// `was` and as it `is` disagree on whether a charger or a computer powers
/// the phone.
fn plug(app: &mut App, was: &Option<skyward_info::Battery>, is: &Option<skyward_info::Battery>, now: u64) -> Result<()> {
    let plugged = |b: &Option<skyward_info::Battery>| b.as_ref().is_some_and(|b| b.plugged);
    if plugged(was) != plugged(is) {
        skyward_sys::kmsg(if plugged(is) { "a charger or a computer powers the phone" } else { "the phone runs on its battery" })?;
        app.plugged(now, plugged(is));
    }
    Ok(())
}

/// The phone's readings that could not be taken, and why the log was last
/// told each could not: the battery's, read each second, and all the
/// phone's details, read less often and after what changes them.
#[derive(Default)]
struct Untaken {
    battery: skyward_sys::ToldOnce,
    details: skyward_sys::ToldOnce,
}

/// Take in how reading `what` went, `why` it failed where it did. A reading
/// that cannot be taken leaves the pages with the one they have, and what
/// is under way goes on; the log is told why, once for as long as the
/// reason stays the same.
fn untaken(told: &mut skyward_sys::ToldOnce, what: &str, why: Option<&anyhow::Error>) -> Result<()> {
    let why = why.map(|e| format!("{e:#}"));
    if told.untold(why.as_deref()) {
        skyward_sys::kmsg(&format!("{what} could not be read, and the pages keep what they have: {}", why.unwrap_or_default()))?;
    }
    Ok(())
}

/// Read all the phone's details anew into `info`, which keeps what it has
/// where they cannot be read.
fn read_details(cfg: &DeviceConfig, info: &mut skyward_info::DeviceInfo, told: &mut skyward_sys::ToldOnce) -> Result<()> {
    let read = skyward_info::collect(cfg);
    untaken(told, "the phone's details", read.as_ref().err())?;
    if let Ok(read) = read {
        *info = read;
    }
    Ok(())
}

/// How far a computer has the USB port, as the port's controller tells.
///
/// The controller goes away while a dual-role port serves an OTG peripheral: a
/// drive plugged into the phone takes the port for itself and the gadget is
/// unregistered. That is read as nothing attached rather than an error, and a
/// phone whose init names no controller is read the same way.
fn usb_link() -> Result<UsbLink> {
    let controller = skyward_sys::prop("sys.usb.controller");
    let state = skyward_sys::usb::gadget_state(Path::new("/sys/class/udc"), controller.as_deref())?;
    Ok(match state.as_deref().map(str::trim) {
        Some("configured") => UsbLink::Connected,
        Some("not attached") | None => UsbLink::Unplugged,
        Some(_) => UsbLink::Connecting,
    })
}

/// What a request from misc shows on the pages that answer it.
fn asked(request: &bcb::Request) -> Asked {
    Asked {
        reason: request.reason.clone(),
        shutdown_after: request.shutdown_after,
        unhandled: match &request.work {
            Work::Unsupported(what) => Some(what.clone()),
            _ => None,
        },
    }
}

/// Read a folder for the file manager, as it is now.
fn list(app: &mut App, folder: String) -> Result<()> {
    let files = read_folder(Path::new(&folder)).map_err(|e| format!("{e:#}"));
    app.set_files(folder, files, unix_now()?);
    Ok(())
}

/// Read a folder of `place` to choose a file to install in, as it is now;
/// `folder` is its path from the place's top.
/// Read the folder at `folder` on the OTG drive, for the page that shows it.
fn list_otg(app: &mut App, folder: String) -> Result<()> {
    let files = read_folder(&in_place(Place::UsbDrive, &folder)).map_err(|e| format!("{e:#}"));
    app.set_otg(folder, files, unix_now()?);
    Ok(())
}

fn list_place(app: &mut App, place: Place, folder: String) -> Result<()> {
    let files = read_folder(&in_place(place, &folder)).map_err(|e| format!("{e:#}"));
    app.set_place(place, folder, files, unix_now()?);
    Ok(())
}

/// Where `path`, from the top of `place`, is: internal storage is the
/// owner's shared storage, where the data partition shows it.
fn in_place(place: Place, path: &str) -> PathBuf {
    let top = match place {
        Place::Storage => Path::new(skyward_crypto::data::SDCARD),
        Place::UsbDrive => Path::new(USB_DRIVE),
    };
    if path.is_empty() {
        top.to_path_buf()
    } else {
        top.join(path)
    }
}

/// Save the logs chosen into a folder of their own in `into`, named by when
/// they were saved in the zone the settings chose: the recovery's own, with
/// what update_engine wrote while it last installed an update and what the
/// last install script wrote, and the kernel's. Each file is owned as a
/// copy into the folder would be. Gives the folder.
fn export_logs(recovery: bool, kernel: bool, into: &Path, zone: usize) -> Result<PathBuf> {
    let there = into.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    let folder = into.join(skyward_ui::fmt::stamp(unix_now()?, zone));
    skyward_files::make_folders(&folder, ownership)?;
    let save = |name: &str, contents: &[u8]| -> Result<()> {
        let path = folder.join(name);
        let mut file = skyward_files::make_file(&path, ownership)?;
        file.write_all(contents).with_context(|| format!("write {}", path.display()))?;
        file.sync_all().with_context(|| format!("write {}", path.display()))
    };
    if recovery {
        save("recovery.log", &fs::read(skyward_sys::LOG).with_context(|| format!("read {}", skyward_sys::LOG))?)?;
        // What update_engine wrote is kept in the recovery's own folder
        // while it runs, and on the partition that outlives the restart
        // once it ends: an export taken after the phone has started the
        // new system still has it (2026-11-21).
        match [skyward_install::ENGINE_LOG, skyward_install::ENGINE_SAVED]
            .iter()
            .find_map(|path| fs::read(path).ok().map(|log| (*path, log)))
        {
            Some((path, log)) => {
                let _ = skyward_sys::kmsg(&format!("the logs are taken from {path}"));
                save("update_engine.log", &log)?;
            }
            None => {}
        }
        match fs::read(skyward_install::SCRIPT_LOG) {
            Ok(log) => save("install-script.log", &log)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("read {}", skyward_install::SCRIPT_LOG)),
        }
    }
    if kernel {
        save("kernel.log", skyward_sys::kernel_log()?.as_bytes())?;
    }
    Ok(folder)
}

/// Save `frame`, what the panel shows, as a picture named for when it was
/// taken: into the gallery's screenshots where internal storage is open, as
/// `storage` says, and otherwise into the recovery's own memory until it
/// opens. Each is made as the system makes what it saves there.
fn save_screenshot(frame: &Canvas, storage: bool, zone: usize) -> Result<Shot> {
    let (folder, shot) = match storage {
        true => (Path::new(skyward_crypto::data::SDCARD).join(SCREENSHOTS), Shot::Album),
        false => (PathBuf::from(HELD_SCREENSHOTS), Shot::Held),
    };
    let there = folder.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    skyward_files::make_folders(&folder, ownership)?;
    let path = folder.join(skyward_ui::fmt::screenshot_name(unix_ms_now()?, zone));
    let file = skyward_files::make_file(&path, ownership)?;
    let mut out = std::io::BufWriter::new(&file);
    frame.write_png(&mut out).with_context(|| format!("write {}", path.display()))?;
    out.flush().with_context(|| format!("write {}", path.display()))?;
    drop(out);
    file.sync_all().with_context(|| format!("write {}", path.display()))?;
    skyward_sys::kmsg(&format!("screenshot saved as {}", path.display()))?;
    Ok(shot)
}

/// Move the screenshots held while internal storage was closed into the
/// gallery's, now that it is open: each keeps its name, and is made there
/// as the system makes what it saves.
fn keep_held_screenshots() -> Result<()> {
    let held = Path::new(HELD_SCREENSHOTS);
    let shots = match fs::read_dir(held) {
        Ok(entries) => entries.map(|e| e.map(|e| e.path())).collect::<std::io::Result<Vec<_>>>().with_context(|| format!("read {}", held.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("read {}", held.display())),
    };
    if shots.is_empty() {
        return Ok(());
    }
    let album = Path::new(skyward_crypto::data::SDCARD).join(SCREENSHOTS);
    let there = album.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    skyward_files::make_folders(&album, ownership)?;
    let stop = std::sync::atomic::AtomicBool::new(false);
    skyward_files::move_into(&shots, &album, skyward_files::Clash::KeepBoth, ownership, &stop, &mut |_| {})?;
    skyward_sys::kmsg(&format!("{} screenshots held in {} moved into {}", shots.len(), held.display(), album.display()))
}

/// The folder backups go in and are found in, in `place`.
fn backups_in(place: Place) -> PathBuf {
    in_place(place, skyward_backup::FOLDER)
}

/// The backups in `place`: each folder in its backups folder, with what its
/// manifest records or why that cannot be read. A place with no backups
/// folder has none.
fn backups_at(place: Place) -> Result<Vec<BackupEntry>> {
    let folder = backups_in(place);
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", folder.display())),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("read {}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !entry.file_type().with_context(|| format!("look at {}", entry.path().display()))?.is_dir() {
            continue;
        }
        let held = skyward_backup::Manifest::read(&entry.path()).map(|m| held(&m)).map_err(|e| format!("{e:#}"));
        found.push(BackupEntry { place, name, held });
    }
    Ok(found)
}

/// What a backup's manifest records, as the pages show it.
fn held(m: &skyward_backup::Manifest) -> Held {
    Held {
        made: m.made,
        size: m.size(),
        parts: m.parts(),
        device: m.device.clone(),
        system: m.system.clone(),
        slot: skyward_vab::Slot::from_suffix(&m.slot),
    }
}

/// Look for backups in the places asked, the restore page showing what is
/// found and saying why a place could not be looked in.
fn find_backups(app: &mut App, storage: bool, usb: bool, now: u64) {
    let mut found = Vec::new();
    for (place, look) in [(Place::Storage, storage), (Place::UsbDrive, usb)] {
        if look {
            match backups_at(place) {
                Ok(mut there) => found.append(&mut there),
                Err(e) => app.report(&format!("{e:#}"), now),
            }
        }
    }
    app.set_backups(found);
}

/// Delete the backup in `folder` of `place`; a USB drive, shown read-only,
/// is made writable for it and read-only again after.
fn delete_backup(place: Place, folder: &Path) -> Result<()> {
    let drive = Path::new(USB_DRIVE);
    if place == Place::UsbDrive {
        skyward_block::mount::remount_writable(drive, true)?;
    }
    let deleted = fs::remove_dir_all(folder).with_context(|| format!("delete {}", folder.display()));
    if place == Place::UsbDrive {
        let read_only = skyward_block::mount::remount_writable(drive, false);
        deleted?;
        return read_only;
    }
    deleted
}

/// How backing up or restoring ended, as its page tells it.
fn backup_end(end: backup::End) -> BackupEnd {
    match end {
        backup::End::Saved(folder) => {
            BackupEnd::Saved(folder.file_name().expect("a backup's folder has a name").to_string_lossy().into_owned())
        }
        backup::End::Restored { unmade } => BackupEnd::Restored { unmade: unmade.iter().map(|p| p.display().to_string()).collect() },
        backup::End::Stopped => BackupEnd::Stopped,
        backup::End::Refused(refusal) => BackupEnd::Refused(match refusal {
            backup::Refusal::Room { needed, free } => BackupRefusal::Room { needed, free },
            backup::Refusal::Update(state) => BackupRefusal::Update(state),
            backup::Refusal::OtherPhone(device) => BackupRefusal::OtherPhone(device),
            backup::Refusal::Resized(name) => BackupRefusal::Resized(name),
            backup::Refusal::DataReset => BackupRefusal::DataReset,
        }),
        backup::End::Failed(why) => BackupEnd::Failed(why),
    }
}

/// Now, in Unix seconds.
fn unix_now() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).context("the clock is before 1970")?.as_secs() as i64)
}

/// Now, in Unix milliseconds.
fn unix_ms_now() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).context("the clock is before 1970")?.as_millis() as i64)
}

/// The USB drive plugged in, and whether it is shown at [`USB_DRIVE`] or
/// why it could not be.
struct Plugged {
    drive: Drive,
    shown: Result<(), String>,
}

/// Look for the USB drive plugged in: one gone is taken away from where it
/// was shown, even with files on it still open, and one found in two looks
/// running is shown, the kernel having listed its partitions by then.
/// `seen` is a drive found by the look before that is not shown yet. What
/// the pages are told comes back when it changed.
fn look_for_drive(plugged: &mut Option<Plugged>, seen: &mut Option<String>) -> Result<Option<Option<UsbDrive>>> {
    let found = usb::drives()?.into_iter().find(Drive::ready);
    if let Some(p) = plugged {
        if found.is_some_and(|d| d.disk == p.drive.disk) {
            return Ok(None);
        }
        if p.shown.is_ok() {
            // Every division was shown under the one folder: all of them go.
            // Nothing here may end the recovery: a drive pulled out is gone
            // from the mount table already, and reading it again may fail for
            // reasons of its own (2026-11-16: this ended the recovery, and the
            // partition was locked again behind it).
            let _ = skyward_block::mount::umount_under(Path::new(USB_DRIVE));
        }
        skyward_sys::kmsg(&format!("USB drive {} is gone", p.drive.disk))?;
        *plugged = None;
        *seen = None;
        return Ok(Some(None));
    }
    let Some(drive) = found else {
        *seen = None;
        return Ok(None);
    };
    if seen.as_deref() != Some(drive.disk.as_str()) {
        *seen = Some(drive.disk.clone());
        return Ok(None);
    }
    // A mount a start that ended before taking it away left behind is taken
    // away now: the drive must be shown from the division this start picks,
    // not one an earlier start happened to reach first.
    let _ = skyward_block::mount::umount_under(Path::new(USB_DRIVE));
    let shown = match usb::mount_all(&drive, Path::new(USB_DRIVE)) {
        Ok(parts) => {
            // One line per division shown, so the log says which way each went.
            for (part, filesystem) in &parts {
                skyward_sys::kmsg(&format!("USB drive {part} shown at {USB_DRIVE}/{part} as {filesystem}"))?;
            }
            Ok(())
        }
        Err(e) => {
            skyward_sys::kmsg(&format!("USB drive {} not shown: {e:#}", drive.disk))?;
            Err(format!("{e:#}"))
        }
    };
    let told = match &shown {
        Ok(()) => UsbDrive::Shown,
        Err(why) => UsbDrive::Unreadable(why.clone()),
    };
    *plugged = Some(Plugged { drive, shown });
    *seen = None;
    Ok(Some(Some(told)))
}

/// What a choice on the clash page asks of the work on files.
fn clash_rule(choice: ClashChoice) -> skyward_files::Clash {
    match choice {
        ClashChoice::Replace => skyward_files::Clash::Replace,
        ClashChoice::Skip => skyward_files::Clash::Skip,
        ClashChoice::KeepBoth => skyward_files::Clash::KeepBoth,
    }
}

/// What a copy or move into the folder `into` gives the entries it makes.
/// On the data partition each takes the owner, group and label the folder
/// gives its own, as the system makes them there; in shared storage, the
/// folder's permissions as well. Elsewhere each keeps what it had.
fn ownership(into: &Path) -> Result<Ownership> {
    use skyward_crypto::data;
    let at = Path::new(data::MOUNT);
    if !skyward_block::mount::is_mount_point(at)? {
        return Ok(Ownership::Kept);
    }
    let here = fs::metadata(into).with_context(|| format!("look at {}", into.display()))?;
    if here.dev() != fs::metadata(at).with_context(|| format!("look at {}", at.display()))?.dev() {
        return Ok(Ownership::Kept);
    }
    let real = fs::canonicalize(into).with_context(|| format!("find {}", into.display()))?;
    let shared = [data::SHARED_STORAGE, data::SDCARD].iter().any(|s| real.starts_with(s));
    Ok(if shared { Ownership::Shared } else { Ownership::Folder })
}

/// The page's name for a part erasing goes through.
fn part(step: Step) -> Part {
    match step {
        Step::Data => Part::Data,
        Step::Cache => Part::Cache,
        Step::Metadata => Part::Metadata,
        Step::AppData => Part::AppData,
        Step::Media => Part::Media,
        Step::AppCaches => Part::AppCaches,
        Step::SystemCaches => Part::SystemCaches,
        Step::Art => Part::Art,
        Step::Logs => Part::Logs,
        Step::Updates => Part::Updates,
    }
}

/// The part of the phone a choice of the wipe page other than everything
/// erases.
fn step(choice: Wipe) -> Step {
    match choice {
        Wipe::AppData => Step::AppData,
        Wipe::Media => Step::Media,
        Wipe::AppCaches => Step::AppCaches,
        Wipe::SystemCaches => Step::SystemCaches,
        Wipe::Art => Step::Art,
        Wipe::Cache => Step::Cache,
        Wipe::Logs => Step::Logs,
        Wipe::Updates => Step::Updates,
        Wipe::All => unreachable!("everything is a factory reset, not a part of one"),
    }
}

/// How erasing stopped, as its page tells it.
fn erase_stop(stop: wipe::Stop) -> EraseStop {
    match stop {
        wipe::Stop::Refused(wipe::Refusal::Update(hold)) => EraseStop::Update(hold),
        wipe::Stop::Refused(wipe::Refusal::NotRebuilt) => EraseStop::NotRebuilt,
        wipe::Stop::Refused(wipe::Refusal::NoCache) => EraseStop::NoCache,
        wipe::Stop::Closed(why) => EraseStop::Closed(why),
        wipe::Stop::Locked(users) => EraseStop::Locked(users),
        wipe::Stop::Failed(step, e) => EraseStop::Failed(part(step), format!("{e:#}")),
    }
}

/// Start erasing all user data, its page following it; why it cannot
/// start ends it at once, on the same page. `asked` when the system asked
/// for it; `reformat` the filesystem every part is made afresh as, where
/// one was asked for.
fn factory_reset(app: &mut App, keep_memtag_mode: bool, asked: bool, reformat: Option<String>) -> Result<Eraser> {
    match wipe::factory_reset(reformat) {
        Ok(plan) => {
            app.erase_started(true, asked, plan.steps().into_iter().map(part).collect());
            Eraser::start(move |tell| plan.carry_out(keep_memtag_mode, tell))
        }
        Err(stop) => {
            app.erase_started(true, asked, Vec::new());
            Eraser::stopped(stop)
        }
    }
}

/// Start erasing all user data for rescue, the installing page saying so
/// meanwhile: as a factory reset erases it, the memory tagging asked of the
/// bootloader forgotten too.
fn rescue_wipe(app: &mut App) -> Result<Eraser> {
    app.rescue_wiping();
    match wipe::factory_reset(None) {
        Ok(plan) => Eraser::start(move |tell| plan.carry_out(false, tell)),
        Err(stop) => Eraser::stopped(stop),
    }
}

/// Start erasing parts on their own, its page following it: parts of the
/// data partition, and the cache partition, made afresh as `reformat`
/// where the system asked for that filesystem; `logs` are the phone's own
/// log folders. `asked` when the system asked for it.
fn parts_wipe(app: &mut App, steps: Vec<Step>, logs: &[String], asked: bool, reformat: Option<String>) -> Result<Eraser> {
    match wipe::parts(steps, logs, reformat) {
        Ok(parts) => {
            app.erase_started(false, asked, parts.steps().into_iter().map(part).collect());
            Eraser::start(move |tell| parts.carry_out(tell))
        }
        Err(stop) => {
            app.erase_started(false, asked, Vec::new());
            Eraser::stopped(stop)
        }
    }
}

/// The reasons the bootloader gives for a start that followed the kernel
/// stopping, as Android's own recovery lists them; it does not start an
/// install the system asked for before such a start.
const PANICS: [&str; 2] = ["kernel_panic", "Panic"];

/// Whether the phone came to the recovery after its kernel stopped.
pub fn panicked() -> bool {
    skyward_sys::prop("ro.boot.bootreason").is_some_and(|reason| PANICS.iter().any(|p| p.eq_ignore_ascii_case(&reason)))
}

/// An install the system asked for that waits for the data partition to
/// open, or for the user whose own files hold the package to unlock them.
struct Pending {
    package: asked::Asked,
    needs: Needs,
    retry: bool,
}

/// Start installing the package the system asked for.
fn install_asked(app: &App, pending: Pending) -> Result<Installer> {
    if pending.needs == Needs::Cache {
        let cache = wipe::cache()?.context("the phone has no cache partition")?;
        let at = Path::new(&cache.mount_point);
        if !mount::is_mount_point(at)? {
            fs::create_dir_all(at).with_context(|| format!("make {}", at.display()))?;
            mount::mount_ro(Path::new(&fstab::device(&cache)?), at)?;
        }
    }
    Installer::asked(pending.package, pending.retry, detach(), install_options(app))
}

/// Where a reboot rescue asked for goes: the target, and whether it is
/// rescue again, which the recovery is told in misc.
fn rescue_target(target: &str) -> (Target, bool) {
    match target {
        "bootloader" => (Target::Bootloader, false),
        "fastboot" => (Target::Fastboot, false),
        "recovery" => (Target::Recovery, false),
        "rescue" => (Target::Recovery, true),
        // As minadbd has it, anything else starts the system.
        _ => (Target::System, false),
    }
}

/// How a partition's work ended, as its page tells it.
fn item_end(item: partwork::Item) -> ItemEnd {
    match item {
        partwork::Item::Erased => ItemEnd::Erased,
        partwork::Item::Unshared => ItemEnd::Unshared,
        partwork::Item::NotShared(filesystem) => ItemEnd::NotShared(filesystem),
        partwork::Item::Fixed(Fixed::Clean) => ItemEnd::Clean,
        partwork::Item::Fixed(Fixed::Fixed) => ItemEnd::Fixed,
        partwork::Item::Made => ItemEnd::Made,
        // What the program said shows below.
        partwork::Item::Fixed(Fixed::Failed) => ItemEnd::Failed(None),
        partwork::Item::Failed(why) => ItemEnd::Failed(Some(why)),
    }
}

/// Why work on whole partitions stopped, as its page tells it.
fn part_stop(stop: partwork::Stop) -> PartStop {
    match stop {
        partwork::Stop::Refused(refusal) => PartStop::Refused(refusal),
        partwork::Stop::Unmounted(base) => PartStop::Unmounted(base),
        partwork::Stop::Failed(why) => PartStop::Failed(why),
    }
}

/// The partitions the filesystem page lists, in this order, where the
/// phone's table has them: the data partition, /metadata and the cache
/// partition. Each comes with what it holds now, its size, and the other
/// filesystems the table lists for it that the recovery makes. The data
/// partition reads only through its decryption, so what it holds is known
/// only once it is open; until then, any of the table's filesystems may be
/// it, and where the table lists one, it is that. /metadata keeps the key
/// the data partition opens with, which making it afresh would lose, so it
/// is offered no other.
fn read_filesystems() -> Result<Vec<FsRow>> {
    let mut rows = Vec::new();
    for (name, point) in [(DATA_PARTITION, "/data"), ("metadata", "/metadata"), ("cache", "/cache")] {
        let entries = fstab::read(point)?;
        let Some(entry) = entries.first() else { continue };
        let device = fstab::device(entry)?;
        let size = skyward_block::size_bytes(&device)?;
        let mut listed: Vec<String> = Vec::new();
        for e in &entries {
            if REFORMATS.contains(&e.filesystem.as_str()) && !listed.contains(&e.filesystem) {
                listed.push(e.filesystem.clone());
            }
        }
        let filesystem = match name {
            DATA_PARTITION => mount::visible(Path::new(point))?.map(|v| v.filesystem),
            _ => probe::filesystem(Path::new(&device))?,
        };
        let others = match (name, &filesystem) {
            ("metadata", _) => Vec::new(),
            (_, Some(held)) => listed.into_iter().filter(|fs| fs != held).collect(),
            (_, None) if listed.len() > 1 => listed,
            (_, None) => Vec::new(),
        };
        rows.push(FsRow { name: name.to_string(), mount_point: point.to_string(), filesystem, size, others });
    }
    Ok(rows)
}

/// The filesystem page's partition `name` for its work: the data partition,
/// or the table's first entry for another.
fn fs_part(name: &str) -> Result<partwork::Part> {
    if name == DATA_PARTITION {
        return Ok(partwork::Part::Data);
    }
    let point = format!("/{name}");
    let entry = fstab::read(&point)?.into_iter().next().with_context(|| format!("the phone's table has no {point}"))?;
    Ok(partwork::Part::Entry(entry))
}

/// What the pages act on: a touch or a key, what a program asked on the
/// recovery socket, or a USB drive plugged in or pulled out.
enum Incoming {
    /// The page shown first, which may ask for work as it shows.
    Arrived,
    Input(Event),
    Asked(Request),
    Drive(Option<UsbDrive>),
}

/// What a request on the recovery socket comes to. adbd asks for fastboot
/// while the slot's partitions are up, which fastbootd cannot work with, so
/// the phone restarts into fastboot as from the reboot page; fastbootd asks
/// for the recovery it runs in. A request that finds the recovery already
/// where it asks to be, as when the keys left fastboot first, needs nothing.
fn answer(request: Request, fastboot: bool) -> Vec<Effect> {
    match (request, fastboot) {
        (Request::Fastboot, false) => vec![Effect::Reboot(Target::Fastboot)],
        (Request::Recovery, true) => vec![Effect::EnterRecovery],
        _ => Vec::new(),
    }
}

/// The time of day for the status bar, in the zone the settings chose.
fn clock(zone: usize) -> Result<String> {
    Ok(skyward_ui::fmt::clock(unix_now()?, zone))
}

/// Tell the kernel log how the data partition was mounted, where opening
/// it mounted it: the check made first, in full where it found something,
/// and the device it is read through.
fn log_mounted(mounted: &Opened) -> Result<()> {
    if let Some(checked) = &mounted.checked {
        skyward_sys::kmsg(&format!("the check of {} ended with {}", mounted.mapped.display(), checked.status))?;
        if checked.status != 0 {
            for line in checked.said.lines() {
                skyward_sys::kmsg(line)?;
            }
        }
    }
    skyward_sys::kmsg(&format!("{} holds {} at {}", mounted.mapped.display(), mounted.filesystem, skyward_crypto::data::MOUNT))
}

/// Show what the data partition still keeps closed: the first user whose
/// own files wait on a screen lock they set themselves, with the control
/// that lock is answered with, or nothing. Users whose credential the phone
/// made up open behind the owner and are never asked; one that did not
/// open goes to the kernel log with why.
fn show_closed(app: &mut App, closed: &[Closed]) -> Result<()> {
    use skyward_crypto::user::Lock;
    skyward_sys::kmsg(&format!("the users whose own files are still closed: {:?}", closed.iter().map(|c| c.user).collect::<Vec<_>>()))?;
    for c in closed {
        if let Some(why) = &c.why {
            skyward_sys::kmsg(&format!("the files of user {} stay closed: {why}", c.user))?;
        }
    }
    let waiting = closed.iter().find(|c| !c.made_up).map(|c| {
        let keys = match c.lock {
            Lock::PATTERN => Keys::Pattern,
            Lock::PIN => Keys::Digits,
            _ => Keys::Letters,
        };
        (c.user, keys)
    });
    app.data_waiting(waiting);
    Ok(())
}

/// What the file manager lists for a folder: its entries, folders and
/// links to folders first, then by name ignoring case, each folder with how
/// many entries it holds. An entry gone by the time it is looked at, as
/// processes in /proc go, is left out.
fn read_folder(path: &Path) -> Result<Vec<FileRow>> {
    let gone = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
    let mut rows = Vec::new();
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let full = entry.with_context(|| format!("read {}", path.display()))?.path();
        let meta = match fs::symlink_metadata(&full) {
            Ok(meta) => meta,
            Err(e) if gone(&e) => continue,
            Err(e) => return Err(e).with_context(|| format!("stat {}", full.display())),
        };
        let kind = if meta.file_type().is_symlink() {
            let target = fs::read_link(&full).with_context(|| format!("read the link {}", full.display()))?;
            // A link to nothing leads to no folder.
            let folder = match fs::metadata(&full) {
                Ok(to) => to.is_dir(),
                Err(e) if gone(&e) => false,
                Err(e) => return Err(e).with_context(|| format!("follow {}", full.display())),
            };
            FileKind::Link { target: target.to_string_lossy().into_owned(), folder }
        } else if meta.is_dir() {
            // A folder that may not be read shows that in place of a count.
            let items = match fs::read_dir(&full) {
                Ok(inside) => Some(inside.count()),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => None,
                Err(e) if gone(&e) => continue,
                Err(e) => return Err(e).with_context(|| format!("read {}", full.display())),
            };
            FileKind::Folder { items }
        } else if meta.is_file() {
            FileKind::File
        } else {
            FileKind::Special
        };
        let name = full.file_name().with_context(|| format!("{} has no name", full.display()))?.to_string_lossy().into_owned();
        rows.push(FileRow { name, kind, size: meta.len(), modified: meta.mtime(), mode: meta.mode(), uid: meta.uid(), gid: meta.gid() });
    }
    rows.sort_by_key(|r| (!matches!(r.kind, FileKind::Folder { .. } | FileKind::Link { folder: true, .. }), r.name.to_lowercase()));
    Ok(rows)
}

/// The slot's logical partitions and how each shows now, for the mount page.
/// A mounted partition is in use when a program holds it, or when one of
/// `services` this phone's build of the hardware runs is run from it: those
/// are started again from there whenever they end.
fn read_mounts(services: &[Service]) -> Result<Vec<skyward_ui::app::MountRow>> {
    use skyward_ui::app::MountRow;
    let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
    let shown = skyward_vab::mounted_partitions(slot)?;
    let holders = skyward_block::mount::holders()?;
    let props = skyward_info::installed_props(slot)?;
    let services: Vec<&Service> = services.iter().filter(|s| s.runs_on(&props)).collect();
    let mut rows = Vec::new();
    for base in skyward_vab::slot_bases(slot)? {
        let point = skyward_vab::mount_point(&base);
        let name = format!("{base}{}", slot.suffix());
        let mounted = shown.iter().find(|m| m.partition == name);
        let mut using = BTreeSet::new();
        if let Some(m) = mounted {
            using.extend(holders.get(&skyward_block::devnum(&m.device)?).into_iter().flatten().cloned());
            for s in services.iter().filter(|s| !s.oneshot && s.uses(&point)) {
                let program = Path::new(&s.path).file_name().with_context(|| format!("{} names no program", s.path))?;
                using.insert(program.to_string_lossy().into_owned());
            }
        }
        rows.push(MountRow {
            base,
            point: point.display().to_string(),
            mounted: mounted.is_some(),
            holders: using.into_iter().collect(),
        });
    }
    Ok(rows)
}

/// What the phone's boot control holds of the slots, or why it cannot be
/// asked.
fn read_slots() -> Result<skyward_vab::BootSlots, String> {
    let read = || -> Result<skyward_vab::BootSlots> {
        let (binder, hardware) = (skyward_binder::driver::Binder::open()?, skyward_binder::driver::Binder::hardware()?);
        let control = skyward_vab::boot::BootControl::open(&binder, &hardware)?;
        control.slots()
    };
    read().map_err(|e| format!("{e:#}"))
}

/// Have the bootloader start `slot` next and give what the boot control
/// holds of the slots then, unless an update is in flight: it set the
/// slot it needs itself.
fn set_slot(slot: skyward_vab::Slot) -> Result<skyward_vab::BootSlots, String> {
    let set = || -> Result<skyward_vab::BootSlots> {
        let state = skyward_vab::snapshot_state()?;
        ensure!(state == skyward_vab::SnapshotState::None, "an update is in flight ({state:?}), and it set the slot it needs");
        // The second system starts only from the slot its boot set was put
        // in, and the installed system's goes back there first.
        skyward_systems::install::ensure_installed()?;
        let (binder, hardware) = (skyward_binder::driver::Binder::open()?, skyward_binder::driver::Binder::hardware()?);
        let control = skyward_vab::boot::BootControl::open(&binder, &hardware)?;
        control.set_active(slot)?;
        skyward_sys::kmsg(&format!("slot {} starts next", slot.letter()))?;
        control.slots()
    };
    set().map_err(|e| format!("{e:#}"))
}

/// What has to happen before an update writes over the installed system's
/// partitions: the files the phone's own programs run from stay in memory,
/// and the booted slot's partitions come off.
fn detach() -> Detach {
    Box::new(|| {
        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
        let mounted = skyward_vab::mounted_partitions(slot)?;
        let busy = services::pin(&mounted)?;
        for point in skyward_vab::release_partitions(slot, &busy)? {
            let _ = skyward_sys::kmsg(&format!(
                "{} still had a file of the phone's own programs open, so it left the tree rather than being unmounted",
                point.display()
            ));
        }
        Ok(())
    })
}

/// What an install does besides installing, as the install page's switches
/// are set: puts Skyward back into the recovery partitions it writes over,
/// on a phone that has them, and checks a zip's signature before its
/// install script runs.
fn install_options(app: &App) -> Options {
    Options { keep: app.state.keep_recovery && app.state.recovery_partitions, signed: app.state.check_signature }
}

/// How often the WLAN page scans while it shows.
const SCAN_EVERY: Duration = Duration::from_secs(10);

/// How often the network kept is looked for while the WLAN page is elsewhere:
/// the scan right after the driver comes up often holds nothing, and the
/// supplicant's own round is slow.
const LOOKING_FOR: Duration = Duration::from_secs(3);

/// The picture the account service keeps for the account the phone is signed
/// in to: one file, `xiaomi_user_avatar_` and the account's number.
fn account_picture(px: u32) -> Option<avatar::Avatar> {
    let dir = Path::new("/data/data/com.xiaomi.account/files");
    let mut found: Option<std::path::PathBuf> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy();
        if name.starts_with("xiaomi_user_avatar_") {
            found = Some(path);
            break;
        }
    }
    let path = found?;
    match avatar::Avatar::read(&path, px) {
        Ok(picture) => Some(picture),
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("the account's picture at {} is not read: {e:#}", path.display()));
            None
        }
    }
}

fn wlan_worker(w: &Option<wlan::Worker>) -> Result<&wlan::Worker> {
    w.as_ref().context("the phone's WLAN is not brought up")
}

/// The installed system, as the update server is told of it.
fn installed(cfg: &DeviceConfig, info: &skyward_info::DeviceInfo) -> Result<Installed> {
    let slot = info.slot.context("only an A/B phone is updated")?;
    let props = skyward_info::installed_props(slot)?;
    let prop = |name: &str| props.get(name).cloned().with_context(|| format!("the installed system says no {name}"));
    Ok(Installed {
        device: prop("ro.product.mod_device")?,
        version: prop(&cfg.device.os_version_prop)?,
        android: prop("ro.build.version.release")?,
        unlocked: skyward_block::boot_param("androidboot.flash.locked")?.as_deref() == Some("0"),
    })
}

/// Where `release`'s full package is downloaded to.
fn downloaded(release: &Release) -> PathBuf {
    Path::new(skyward_crypto::data::SDCARD).join(DOWNLOADS).join(&release.name)
}

/// What internal storage holds of `release`'s full package; nothing while it
/// cannot be read.
fn have(app: &App, release: &Release) -> Have {
    if !app.state.data.storage_open() {
        return Have::Nothing;
    }
    let path = downloaded(release);
    let size = |p: &Path| match fs::metadata(p) {
        Ok(m) => Some(m.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("read {}: {e}", p.display()));
            None
        }
    };
    match (size(&path), size(&download::partial(&path))) {
        (Some(whole), _) => Have::Whole(whole),
        (None, Some(part)) => Have::Part(part),
        (None, None) => Have::Nothing,
    }
}

/// Wait until a device, the installer or the recovery socket has something,
/// or `timeout_ms` passes.
fn wait(fds: &[i32], timeout_ms: i32) -> Result<()> {
    let mut polls: Vec<libc::pollfd> = fds.iter().map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 }).collect();
    let n = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, timeout_ms) };
    if n < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err).context("poll input devices");
        }
    }
    Ok(())
}

/// How work on files ended, as its page tells it.
fn job_end(end: filework::End) -> JobEnd {
    match end {
        filework::End::Done => JobEnd::Done,
        filework::End::Stopped => JobEnd::Stopped,
        filework::End::Failed(why) => JobEnd::Failed(why),
    }
}

/// Run the pages. What `request`, read from misc, asks for decides the
/// first of them; asking for nothing, they start at `start`. `programs`
/// are the phone's own that the recovery keeps running: the
/// configuration's, and those the phone's partitions name for its keys.
pub fn run(cfg: &DeviceConfig, programs: &[Service], font: &Path, mono: &Path, request: bcb::Request, start: Screen, dump: Option<&Path>) -> Result<()> {
    // A start that fails is tried again rather than left to init: what the
    // pages need is brought up beside them, on a thread of its own, and one
    // that is a moment late is no reason for the phone to come back to the
    // same page for ever (2026-11-19).
    let mut tries = 0u32;
    let mut pages = loop {
        match Pages::open(cfg, programs, font, mono, request.clone(), start, dump) {
            Ok(pages) => break pages,
            Err(e) => {
                tries += 1;
                let _ = skyward_sys::kmsg(&format!("the pages did not come up, try {tries}: {e:#}"));
                if tries >= 4 {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(600 * u64::from(tries)));
            }
        }
    };
    pages.begin()?;
    // The phone sleeps while its screen is off and nothing needs it awake.
    let sleeper = Sleeper::start()?;
    // A uevent may be a charger or a computer attached or taken away: the
    // battery is read at once, and the phone stays awake until it is.
    let uevents = uevent::Listener::open()?;
    // A turn that comes apart is said and gone on from: one page's failure
    // is not worth the whole recovery going dark, and init starting the
    // service again would only bring the same page back (2026-11-19).
    let mut said: Option<(Instant, String)> = None;
    loop {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pages.turn(&sleeper, &uevents))) {
            Ok(Ok(())) => said = None,
            Ok(Err(e)) => {
                say_failure(&mut said, &format!("{e:#}"));
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(panic) => {
                say_failure(&mut said, &format!("a panic, {}", panic_text(&*panic)));
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

/// Say a turn's failure once, and again only when the reason changes or a
/// second has gone by: a page that fails at every turn must not fill the
/// log.
fn say_failure(said: &mut Option<(Instant, String)>, why: &str) {
    let now = Instant::now();
    let tell = match said {
        Some((at, last)) => last != why || now.duration_since(*at) >= Duration::from_secs(1),
        None => true,
    };
    if tell {
        let _ = skyward_sys::kmsg(&format!("a turn of the pages failed: {why}"));
        *said = Some((now, why.to_owned()));
    }
}

/// What a panic was about, for the log: the text it was raised with.
pub fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = panic.downcast_ref::<&str>() {
        return (*text).to_owned();
    }
    if let Some(text) = panic.downcast_ref::<String>() {
        return text.clone();
    }
    "the panic says nothing".to_owned()
}

/// 开机后最多试多少次把马达装上。驱动把 FF 设备做出来通常在开机五秒
/// 之内；两分钟还没来，就是这台机器没有马达（或者驱动没装），再枚举也
/// 只是每秒读一次 /dev/input。
const MOTOR_ATTEMPTS: u32 = 120;

/// Take the motor up if it has not come up yet, and play a click or the
/// ticks whatever comes of it: a motor that arrives late, or one that stops
/// answering, is nothing for the page to die over. A device that failed is
/// let go of, so the next click takes a fresh one up.
pub fn gentle_click(haptics: &mut Option<Haptics>, cfg: &DeviceConfig, vibration: f32, ticks: bool) {    if haptics.is_none()
        && let Some(wanted) = cfg.haptics.as_ref()
        && let Ok(h) = Haptics::open(wanted)
    {
        let _ = skyward_sys::kmsg("the motor came up; the clicks play");
        *haptics = Some(h);
    }
    let mut broken = false;
    if let Some(h) = haptics {
        let played = if ticks { h.ticks() } else { h.click(vibration) };
        if let Err(e) = played {
            let _ = skyward_sys::kmsg(&format!("the motor would not answer: {e:#}"));
            broken = true;
        }
    }
    if broken {
        *haptics = None;
    }
}

/// The pages, and all the loop that runs them holds from one turn to the
/// next: the panel and what is drawn on it, what the phone tells of
/// itself, the request from misc, and the work under way, each on a thread
/// of its own.
struct Pages<'a> {
    cfg: &'a DeviceConfig,
    /// The phone's own programs that the recovery keeps running.
    programs: &'a [Service],
    /// What misc asked for as the recovery started.
    request: bcb::Request,
    /// Where each frame shown is written as well.
    dump: Option<&'a Path>,
    app: App,
    info: skyward_info::DeviceInfo,
    unread: Untaken,
    fonts: Fonts,
    surface: KmsSurface,
    /// The frame last drawn.
    screen: Canvas,
    inputs: Inputs,
    started: Instant,
    /// Whether the wait for a touch panel to come up is over.
    touch_known: bool,
    /// adbd and fastbootd ask over it to go over to fastboot and back.
    control: Option<Control>,
    haptics: Option<Haptics>,
    /// 上一次试着把马达装上的时刻。驱动要到开机几秒之后才把 FF 设备做出
    /// 来，而页面原先只在有人点的时候试一次 —— 第一次点按因此没有震动
    /// （v112 真机日志：`the motor came up` 出现在 40.87 s，正是第一次点
    /// 屏幕的时刻）。页面每一秒试一次，马达一到位就装上。
    motor_try: Option<Instant>,
    /// 已经试过几次，试够就不再试：马达真没有的时候不该一直枚举下去。
    motor_tries: u32,
    /// Whether the partitions were open on this start: the starts that
    /// ended before it are then done with, and a start that follows can try
    /// to open them again rather than go straight to the menu.
    reached: bool,
    backlight: Backlight,
    keeper: Keeper,
    shown_clock: String,
    /// The page shown first, until it has been told it shows.
    arriving: Option<Incoming>,
    /// The readings are taken again on a thread of their own; those taken
    /// here as work ends say how it left the phone, and readings started
    /// before them are out of date.
    readings: Readings,
    read_at: Instant,
    /// When the battery was asked for at once, after a uevent; the phone
    /// stays awake until it is read.
    asked: Option<Instant>,
    /// Whether the panel's frame memory has been blacked out for an off
    /// panel; cleared once a real frame is shown again.
    blanked: bool,
    /// When the panel gone dark began to rest.
    rested_at: Option<Instant>,
    /// Started for fastboot, the port goes to the phone's own fastbootd. The
    /// page says whether it came up and how far a computer has the port;
    /// why it did not come up goes to the kernel log and on screen.
    fastboot: bool,
    port_at: Instant,
    /// File transfer runs while its switch is on, and offers internal
    /// storage while that is open; one that does not start says why on
    /// screen, and the recovery carries on.
    transfer: Option<Transfer>,
    /// A sideload has the port, which goes back to rest once it ends.
    sideloading: bool,
    /// 正在录的一段：页面每画一帧就往它里面送一帧。
    recorder: Option<skyward_ui::record::Recorder>,
    misc: PathBuf,
    /// Work the request asks for that goes on keeps the request in misc,
    /// so a restart halfway comes back to it, until it is done or the user
    /// leaves it for somewhere else.
    kept: bool,
    /// Once the work the system asked for is done, the page shows it for a
    /// moment; then the phone starts the system, or powers off where the
    /// request says to.
    leave: Target,
    leave_at: Option<u64>,
    installer: Option<Installer>,
    /// The install the system asked for, until the data partition it is in
    /// opens, or the user whose own files hold it unlocks them.
    pending: Option<Pending>,
    /// Where rescue asked the phone to leave for, once it lets USB go.
    rescue_leaves: Option<(Target, bool)>,
    eraser: Option<Eraser>,
    /// Erasing the system asked for leaves on its own once it is done, as
    /// does erasing an update asked for, sideloaded with a restart after it;
    /// what the user chose waits for them.
    erase_leaves: bool,
    /// Erasing all user data rescue asked for, whose end goes back to rescue
    /// rather than to a page of its own.
    rescue_erasing: bool,
    /// Opening the data partition, or a user's own files in it, under way,
    /// and the users whose own files are still closed once it is open.
    decryptor: Option<Decryptor>,
    /// A job asked for before the phone's own parts were up -- the key
    /// service this needs is one of them -- started by the loop once they
    /// are, so that the page is drawn and its ring turns meanwhile.
    pending_open: Option<decrypt::Job>,
    /// When the job now running was started: a job that never comes back is
    /// not one the page can wait on for ever.
    decrypt_at: Instant,
    /// When the opening was last said to the kernel log, so that a job that
    /// takes long says where it stands without filling the log.
    opening_said: Option<Instant>,
    /// When the page was first seen saying it is at the partition with no job
    /// opening it: asked for and never started, it would wait for ever.
    idle_open_at: Option<Instant>,
    closed: Vec<Closed>,
    /// The screen lock kept from before, where one was kept, so that the data
    /// partition opens as the recovery starts rather than waiting to be typed.
    remembered: Option<Vec<u8>>,
    /// Whether the kept screen lock has been tried this start, and whether the
    /// attempt now running was made with it.
    tried_remembered: bool,
    attempt_was_remembered: bool,
    /// What was entered for the attempt now running, kept to write down after.
    last_credential: Option<Vec<u8>>,
    /// Work on whole partitions under way.
    part_worker: Option<partwork::Worker>,
    /// Copying, moving or deleting under way.
    worker: Option<filework::Worker>,
    /// Giving the data partition's files their SELinux labels under way.
    relabeler: Option<filework::Worker>,
    /// Backing up or restoring under way.
    backuper: Option<backup::Worker>,
    /// An update's merge followed: one the recovery found under way, which
    /// the kernel took up again as the snapshots were mapped, or one started
    /// on the update page.
    merger: Option<merge::Merger>,
    /// Whether the merge that was followed ended with the update merged.
    /// Kept past the look that said so, since the phone leaves on the look
    /// that finds the merge ended, which says nothing itself (2026-11-20).
    merge_done: bool,
    /// The slot root is to be put back into, kept until the merge of that
    /// slot has ended: what the merge writes lands in the same partitions
    /// the root does, and an image carried in before it would be written
    /// over (2026-11-20).
    root_slot: Option<skyward_vab::Slot>,
    /// The root being put back into that slot's boot image, on a thread of
    /// its own: the writing is a few megabytes, and the page follows it
    /// rather than standing still while it happens (2026-11-21).
    rooting: Option<Rooting>,
    /// The second system's work, on a phone that can hold one.
    systems_worker: Option<systems::Worker>,
    /// A switch the system asked for, under way; then the phone starts the
    /// system switched to.
    switch_asked: bool,
    /// A switch went through and the phone is restarting into the system.
    switched: bool,
    /// The DNS servers the network joined gave, for looking names up.
    servers: Servers,
    /// The WLAN, on a phone whose driver the recovery brings up.
    wlan: Option<wlan::Worker>,
    /// The folder served to the network, while it is served.
    serving: Option<crate::fileserve::Server>,
    /// The download under way, and the page another device types an address
    /// for it on.
    fetcher: Option<fetch::Worker>,
    /// 上一次记下的应用页码，用来在它变化时写一行日志。
    last_app_page: usize,
    /// 上一次记下的翻页点击次数。
    last_app_taps: u32,
    /// 上一次记下的“到达应用管理”次数。
    last_app_arrivals: u32,
    /// 开机后是否已经预取过应用列表。
    apps_prefetched: bool,
    /// 预取该开始的时间点（data 打开后 800ms）。
    prefetch_due: Option<u64>,
    /// 上一次记下的“点空”坐标。
    last_page_miss: Option<(f32, f32)>,
    /// 上一次记下的翻页点击坐标。
    last_page_tap: Option<(f32, f32, i32)>,
    /// 逐条补名字与图标的通道：列表先出来，名字和图标边读边填。
    apps_feed: Option<std::sync::mpsc::Receiver<Feed>>,
    /// Whether verified boot's checks have been read for the avb page shown.
    avb_read: bool,
    portal: Option<crate::portal::Portal>,
    scanned_at: Instant,
    /// The network kept with the password that worked, and how far the opening
    /// and the joining of it have gone this start.
    wlan_saved: Option<crate::wlanmem::Saved>,
    wlan_opened: bool,
    /// The account's picture and the name the phone gives itself have been
    /// looked for, once the partition they are kept on is open.
    account_read: bool,
    /// The bringing-up had not left a thread yet, which the next turn that
    /// finds one takes; the folders and libraries a thread of its own needs
    /// if none ever comes.
    wlan_pending: bool,
    wlan_modules: Vec<PathBuf>,
    wlan_libraries: Option<PathBuf>,
    wlan_joined: bool,
    wlan_retries: u32,
    wlan_failed_at: Option<Instant>,
    /// What was typed for the join under way, kept to write down once it works.
    wlan_password: Option<String>,
    wlan_kept: bool,
    /// Asking the update server, and downloading the full package.
    checker: Option<updates::Worker>,
    downloader: Option<updates::Worker>,
    /// Asking GitHub for Skyward's newest release, and updating to it.
    skyward_checker: Option<updates::Worker>,
    skyward_updater: Option<updates::Worker>,
    /// The terminal page's shell, from when the page first asks for one.
    shell: Option<pty::Shell>,
    /// The USB drive plugged in, looked for each second.
    plugged: Option<Plugged>,
    seen: Option<String>,
    drive_at: Instant,
}

/// How often the root being put back tells the page where it is: the panel
/// draws thirty frames a second, and a step of a few megabytes would tell
/// more often than that if every megabyte were a line (2026-11-21).
const ROOT_TELL_EVERY: Duration = Duration::from_millis(80);

/// Putting the phone's root back into the boot image of the slot that
/// starts next, on a thread of its own.
struct Rooting {
    /// The slot whose image is written, and whether the update erases all
    /// user data once that slot starts: both are for the page that comes
    /// after.
    slot: skyward_vab::Slot,
    wipe: bool,
    worker: skyward_sys::worker::Worker<RootSaid>,
}

/// What putting the root back tells the page.
enum RootSaid {
    /// Where the work has come: the step, and the bytes of it done and all.
    Step(skyward_install::root::Story, u64, u64),
    /// It ended, with what became of the phone's root.
    Done(Box<skyward_install::root::Rooted>),
}

impl<'a> Pages<'a> {
    /// Bring up the panel, the touchscreen and the pages, as the settings
    /// kept from the starts before leave them, with none of the work the
    /// request asks for started yet.
    fn open(cfg: &'a DeviceConfig, programs: &'a [Service], font: &Path, mono: &Path, request: bcb::Request, start: Screen, dump: Option<&'a Path>) -> Result<Pages<'a>> {
        if cfg.display.density == 0 {
            return Err(anyhow!("{}: [display] density is required to lay out the UI", cfg.device.codename));
        }
        // The pages come up without a battery that cannot be read, as while no
        // driver for it is loaded; the readings taken each second bring it in
        // once it can.
        let mut unread = Untaken::default();
        let battery = skyward_info::battery_now();
        untaken(&mut unread.battery, "the battery", battery.as_ref().err())?;
        let info = skyward_info::collect_with(cfg, battery.unwrap_or_default())?;
        let fonts = Fonts::load(font, 0, mono)?;
        let mut surface = KmsSurface::open("/dev/dri/card0")?;
        let (w, h) = surface.size();
        // A touch controller that follows the panel wakes as the panel goes
        // from rest to running, and at no other time. The recovery service
        // running again finds it as the earlier run left it, at rest where the
        // panel rested then, so the panel is put to rest and woken with a dark
        // frame, as every later wake wakes it, before the touchscreen is taken
        // in and told its settings: a controller at rest answers none of them.
        if skyward_sys::runs_again() {
            surface.rest()?;
            surface.present(&Canvas::new(w, h))?;
            surface.settle()?;
        }
        eprintln!("display {}", surface.describe());
        let inputs = Inputs::open(w, h, cfg.touch.writes.clone())?;
        let started = Instant::now();
        let control = Control::open()?;
        // The motor is not fatal: the modules it needs are loaded beside the
        // pages, and a start that comes before them must not end the whole
        // recovery -- that is what left the phone coming back to the same
        // page for ever, and the touch module the driver loads last never
        // loaded at all (2026-11-19).
        let haptics = match cfg.haptics.as_ref().map(Haptics::open) {
            Some(Ok(h)) => Some(h),
            Some(Err(e)) => {
                let _ = skyward_sys::kmsg(&format!("the motor is not up yet: {e:#}"));
                None
            }
            None => None,
        };
        let backlight = Backlight::open()?;
        let first = match &request.work {
            Work::Fastboot => Screen::Fastboot,
            Work::PromptWipeData => Screen::CantStart,
            Work::Unsupported(_) => Screen::Unhandled,
            Work::Menu
            | Work::Exit
            | Work::Update(_)
            | Work::WipeData { .. }
            | Work::WipeCache { .. }
            | Work::WipeAb { .. }
            | Work::Sideload { .. }
            | Work::Rescue
            | Work::UnshareBlocks
            | Work::SwitchSystem { .. }
            | Work::SystemStopped => start,
        };
        let mut app = App::new(first, w, h, cfg.display.density as f32 / 160.0);
        app.set_screen_corners(cfg.display.corner_radius as f32);
        // The settings kept from the starts before are back before the first
        // frame, and before file transfer starts with its switch and the time
        // zone. The pages speak the language chosen then, or else the one the
        // system named with the work it asks for, or the one the installed
        // system is built to start in, or else the one they last spoke.
        let kept = settings::read()?;
        let locale = request.locale.as_deref().or(info.locale.as_deref());
        let named = locale.map(Lang::for_locale).transpose()?;
        app.take_settings(kept.settings(), named);
        let keeper = Keeper::new(&kept);
        skyward_sys::kmsg(&format!(
            "the pages speak {:?}, {}, and the locale named is {}",
            app.state.lang,
            if app.state.language_chosen { "the language chosen" } else { "with none chosen" },
            locale.unwrap_or("none")
        ))?;
        // The home page tells of a restart into the slot the system does not
        // run from now: an update or a switch left for the next boot.
        if info.slot.is_some() {
            app.set_boot_slots(read_slots());
        }
        app.set_asked(asked(&request));
        let screen = Canvas::new(w, h);
        // A page sliding over another draws on three screens' worth of canvas
        // besides the panel's: made ready now, the first slide finds them so.
        Canvas::keep_ready(w, h, 3);
        let readings = Readings::start(cfg)?;
        let read_at = Instant::now();
        let fastboot = first == Screen::Fastboot;
        let port_at = Instant::now();
        let mut transfer: Option<Transfer> = None;
        if app.state.file_transfer {
            match Transfer::start(cfg, &info, app.state.time_zone) {
                Ok(t) => transfer = Some(t),
                Err(e) => {
                    skyward_sys::kmsg(&format!("file transfer did not start: {e:#}"))?;
                    app.file_transfer_failed(false, &skyward_ui::fmt::transfer_failed(app.state.lang, &format!("{e:#}")), monotonic_ms());
                }
            }
        }
        if fastboot {
            if let Err(e) = skyward_sys::usb::switch("fastboot", USB_TIMEOUT) {
                skyward_sys::kmsg(&format!("fastbootd has not taken the port: {e:#}"))?;
                app.report(&format!("{e:#}"), monotonic_ms());
            }
            let (running, link) = fastboot_status()?;
            app.set_fastboot(running, link);
        }
        // The wipe page offers the cache partition where the phone's table has
        // one; a table that does not read leaves it out, and the page still
        // works.
        app.state.cache_partition = match wipe::cache() {
            Ok(cache) => cache.is_some(),
            Err(e) => {
                skyward_sys::kmsg(&format!("whether the phone has a cache partition is not known: {e:#}"))?;
                false
            }
        };
        app.state.phone_logs = cfg.wipe.logs.len();
        // Skyward lives in a recovery partition of each slot where the phone
        // has them, and the install page then offers to keep it there. Such a
        // phone holds a second system where it also has boot and init_boot
        // partitions in both slots, which a switch writes.
        let by_name = Path::new(skyward_block::BY_NAME);
        let both = |base: &str| -> Result<bool> { Ok(by_name.join(format!("{base}_a")).try_exists()? && by_name.join(format!("{base}_b")).try_exists()?) };
        app.state.updates = cfg.device.update;
        app.state.recovery_partitions = both("recovery")?;
        app.state.dual_boot = app.state.recovery_partitions && both("boot")? && both("init_boot")?;
        // The second system's work; the systems are read at once, for the home
        // page to name the one that starts, once a switch cut off before is
        // finished.
        let systems_worker = match app.state.dual_boot {
            true => Some(systems::Worker::start(cfg.device.os_version_prop.clone())?),
            false => None,
        };
        // fastbootd may write the boot partitions, which the installed system's
        // boot set goes back into first, and finds /metadata as it was; why the
        // boot set could not go back is shown, and fastboot goes on, as the way
        // left to repair the phone.
        if fastboot && app.state.dual_boot {
            let metadata = Path::new("/metadata");
            let mounted = mount::is_mount_point(metadata)?;
            let put_back = skyward_systems::install::ensure_installed();
            if !mounted && mount::is_mount_point(metadata)? {
                mount::umount(metadata)?;
            }
            if let Err(e) = put_back {
                skyward_sys::kmsg(&format!("the installed system's boot set did not go back before fastboot: {e:#}"))?;
                app.report(&format!("{e:#}"), monotonic_ms());
            }
        }
        // The lock kept from before, if any: the settings switch stands on for
        // what is kept, and off for what is not.
        let mut remembered = crate::remember::read();
        // A start that ended while that lock was being tried leaves a mark:
        // the lock is not tried again this start, or a phone that cannot have
        // it answered would land on the same page at every restart.
        if crate::remember::take_trying() {
            skyward_sys::kmsg("the start before this one ended while the kept screen lock was being tried; asking for it instead")?;
            remembered = None;
        }
        // The switch stands on: a lock kept is kept again, and a phone that has
        // never kept one keeps the next it opens with. Only turning the switch
        // off leaves a mark, and that mark is what keeps it off.
        // Kept only where a lock was kept before: keeping it is chosen at the
        // decrypt page, with the risk of it said first, not taken for granted.
        app.state.remember = remembered.is_some();
        Ok(Pages {
            cfg,
            programs,
            dump,
            app,
            info,
            unread,
            fonts,
            surface,
            screen,
            inputs,
            started,
            touch_known: false,
            control,
            haptics,
            motor_try: None,
            motor_tries: 0,
            reached: false,
            backlight,
            keeper,
            shown_clock: String::new(),
            arriving: Some(Incoming::Arrived),
            readings,
            read_at,
            asked: None,
            blanked: false,
            rested_at: None,
            fastboot,
            port_at,
            transfer,
            sideloading: false,
            recorder: None,
            misc: bcb::misc(),
            kept: request.work.lasts(),
            leave: if request.shutdown_after { Target::PowerOff } else { Target::System },
            leave_at: None,
            request,
            installer: None,
            pending: None,
            rescue_leaves: None,
            eraser: None,
            erase_leaves: false,
            rescue_erasing: false,
            decryptor: None,
            pending_open: None,
            decrypt_at: Instant::now(),
            opening_said: None,
            idle_open_at: None,
            closed: Vec::new(),
            remembered: remembered.clone(),
            tried_remembered: false,
            attempt_was_remembered: false,
            last_credential: None,
            part_worker: None,
            worker: None,
            relabeler: None,
            backuper: None,
            merger: None,
            merge_done: false,
            root_slot: None,
            rooting: None,
            systems_worker,
            switch_asked: false,
            switched: false,
            // The same one the WLAN thread the bringing-up started fills.
            servers: crate::servers(),
            wlan: None,
            serving: None,
            fetcher: None,
            apps_feed: None,
            last_app_page: 0,
            last_app_taps: 0,
            last_app_arrivals: 0,
            apps_prefetched: false,
            prefetch_due: None,
            last_page_miss: None,
            last_page_tap: None,
            avb_read: false,
            portal: None,
            scanned_at: Instant::now(),
            wlan_saved: None,
            wlan_opened: false,
            account_read: false,
            wlan_pending: false,
            wlan_modules: Vec::new(),
            wlan_libraries: None,
            wlan_joined: false,
            wlan_retries: 0,
            wlan_failed_at: None,
            wlan_password: None,
            wlan_kept: false,
            checker: None,
            downloader: None,
            skyward_checker: None,
            skyward_updater: None,
            shell: None,
            plugged: None,
            seen: None,
            drive_at: Instant::now(),
        })
    }

    /// Start the work the request in misc asks for, take up what an earlier
    /// start left under way, and bring the port and the WLAN to where the
    /// pages start from.
    fn begin(&mut self) -> Result<()> {
        let cfg = self.cfg;
        self.forget_stale_request()?;
        match &self.request.work {
            Work::WipeData { reformat } => {
                self.eraser = Some(factory_reset(&mut self.app, self.request.keep_memtag_mode, true, reformat.clone())?);
                self.erase_leaves = true;
            }
            Work::WipeCache { reformat } => {
                self.eraser = Some(parts_wipe(&mut self.app, vec![Step::Cache], &cfg.wipe.logs, true, reformat.clone())?);
                self.erase_leaves = true;
            }
            Work::Sideload { .. } => {
                self.app.start_sideload();
                self.installer = Some(Installer::start(detach(), install_options(&self.app))?);
                self.sideloading = true;
            }
            Work::Rescue => {
                self.app.start_rescue();
                self.installer = Some(Installer::rescue(detach(), install_options(&self.app))?);
                self.sideloading = true;
            }
            Work::Update(update) => {
                self.app.start_asked(update.security, update.package.clone());
                let retry = update.retry > 0;
                if !retry && panicked() {
                    skyward_sys::kmsg("the phone came here after its kernel stopped, so the update asked for is not installed")?;
                    self.app.install(skyward_install::Event::Finished(skyward_install::Outcome::Refused(skyward_install::Refusal::Panicked)), monotonic_ms());
                    bcb::clear(&self.misc)?;
                    self.kept = false;
                } else {
                    let (package, needs) = asked::locate(&update.package, self.app.state.cache_partition);
                    skyward_sys::kmsg(&format!("the update asked for is {package:?}, which needs {needs:?}"))?;
                    let wanted = Pending { package, needs, retry };
                    match needs {
                        Needs::Data | Needs::User(_) => {
                            self.app.asked_opening();
                            crate::wait_prepared();
                            self.start_decrypt(decrypt::Job::Open)?;
                            self.pending = Some(wanted);
                        }
                        Needs::Nothing | Needs::Cache => self.installer = Some(install_asked(&self.app, wanted)?),
                    }
                }
            }
            Work::WipeAb { package_size } => {
                self.app.part_work_started(PartKind::Disable, true);
                self.part_worker = Some(partwork::Worker::start(partwork::Task::Disable { package_size: *package_size })?);
            }
            Work::UnshareBlocks => {
                let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                self.app.part_work_started(PartKind::Unshare, true);
                self.part_worker = Some(partwork::Worker::start(partwork::Task::Unshare { slot })?);
            }
            // The installed system is asked for by a word, and the second by
            // its id.
            Work::SwitchSystem { to } => match self.systems_worker.as_mut() {
                Some(worker) => {
                    self.app.switch_asked(if to == "installed" { Who::Installed } else { Who::Other });
                    worker.send(systems::Task::SwitchNamed(to.clone()));
                    self.switch_asked = true;
                }
                // A phone that cannot hold a second system has none to switch
                // to, and the request is done with.
                None => {
                    skyward_sys::kmsg(&format!("asked to switch to the system {to:?}, which this phone cannot hold"))?;
                    bcb::clear(&self.misc)?;
                    self.kept = false;
                }
            },
            // The second system's start stopped and brought the recovery up to
            // say why; it still starts next, unless the user switches.
            Work::SystemStopped => {
                let why = self.request.reason.clone().unwrap_or_default();
                skyward_sys::kmsg(&format!("the second system's start stopped: {why}"))?;
                if self.app.state.dual_boot {
                    self.app.system_stopped(why);
                }
            }
            Work::Menu | Work::Exit | Work::Fastboot | Work::PromptWipeData | Work::Unsupported(_) => {}
        }
        // A switch asked for finishes one cut off before itself; fastboot leaves
        // /metadata to fastbootd until the recovery's pages come up.
        if let (Some(worker), false, false) = (self.systems_worker.as_mut(), self.switch_asked, self.fastboot) {
            worker.send(systems::Task::Resume);
        }
        // The port came up with adb alone; at rest it offers file transfer
        // beside it.
        if self.transfer.is_some() && !self.fastboot && !self.sideloading {
            take_file_transfer(&mut self.app, monotonic_ms())?;
        }
        // Where an update stands is kept in /metadata, which is not mounted
        // when the pages are built: it is mounted here for the update's
        // state to be read at all, and for the merge that follows
        // (2026-11-20).
        self.open_metadata();
        if let Some(slot) = self.info.slot {
            self.app.set_update(skyward_vab::standing(slot)?);
        }
        // An update a restart cut off before it merged leaves the slot it
        // went into starting from snapshots, which no phone starts from: the
        // phone comes back here at every restart until it is merged. Taking
        // it up as the pages come up is what lets the next restart reach the
        // system (2026-11-20).
        self.resume_merge()?;
        // The WLAN starts off. Without the bundle of the vendor interface's
        // libraries the supplicant does not start, and the WLAN page says why.
        if let Some(w) = &cfg.wlan {
            self.app.state.wlan = Some(Wlan::default());
            let modules = cfg.modules.sources.iter().map(|s| skyward_modules::source_dir(s)).collect::<Result<Vec<_>>>()?;
            let libraries = secure::libraries().unwrap_or_else(|e| {
                let _ = skyward_sys::kmsg(&format!("the vendor interface's libraries are not open: {e:#}"));
                None
            });
            // The bringing-up starts the thread as soon as the driver is warm,
            // and turns the radio on there, so that the supplicant scans and
            // joins while the decrypt page is answered. This only takes it --
            // starting another here made two, and two supplicants fought over
            // the interface and the address (2026-11-15).
            // The bringing-up starts the thread and turns the radio on; this
            // waits for it to be there rather than starting a second one. Two
            // threads fought over the interface and the address when this did
            // (2026-11-15), and going on without one left the pages asking for
            // a WLAN that was never brought up, which ended the recovery and
            // started it again.
            // The bringing-up starts the thread as soon as the driver is warm
            // and turns the radio on there. This does not wait for it: waiting
            // here held the first frame back, and the screen stayed black while
            // the decrypt page's own ring was meant to be turning (2026-11-15).
            // A thread that is not there yet is taken by the first turn that
            // finds it; nothing is started here unless the bringing-up failed.
            self.wlan = crate::take_wlan();
            self.wlan_opened = self.wlan.is_some();
            self.wlan_pending = self.wlan.is_none();
            self.wlan_modules = modules;
            self.wlan_libraries = libraries;
            let _ = w;
        }
        self.scanned_at = Instant::now();
        // The WLAN kept: whether it opens as the recovery starts, and the
        // network it joins by itself with the password that worked before.
        // The order the file manager shows entries in, kept from the starts
        // before.
        if let Some((key, desc, folders)) = crate::sortmem::read() {
            self.app.state.sort = key;
            self.app.state.sort_desc = desc;
            self.app.state.folders_first = folders;
        }
        let wlan_kept = crate::wlanmem::read();
        self.wlan_saved = wlan_kept.saved.clone();
        self.app.state.wlan_auto = wlan_kept.auto;
        self.app.state.wlan_keep = wlan_kept.keep;
        self.app.state.update_server = cfg.update.is_some();
        self.drive_at = Instant::now();
        // The start before this one ended where it should not have: the log
        // keeps it. Nothing of it goes on the page -- that text is the log's
        // English written for whoever reads the log, and a page the phone has
        // just come back to is no place to read it (2026-11-19).
        if let Some(why) = skyward_sys::last_failure() {
            let _ = skyward_sys::kmsg(&format!("the start before this one ended badly: {why}"));
        }
        Ok(())
    }

    /// One turn of the loop: the page is shown as it stands, the loop waits
    /// for something to happen, and what happened is taken in. What the work
    /// under way told comes first, so a touch acts on the page as it now
    /// stands.
    fn turn(&mut self, sleeper: &Sleeper, uevents: &uevent::Listener) -> Result<()> {
        let now = monotonic_ms();
        if self.leave_at.is_some_and(|at| now >= at) {
            self.leave_at = None;
            power(self.leave)?;
        }
        if !self.touch_known && self.started.elapsed() >= TOUCH_WAIT {
            self.touch_known = true;
            // 没有标成 direct 的多点设备在等待用尽
            // 之后当作面板用，总比整块屏幕按不动强。
            if !self.inputs.has_panel() && self.inputs.take_spare_panel()? {
                self.app.set_touchscreen(true);
            }
            if !self.inputs.has_panel() {
                skyward_sys::kmsg("no touch panel came up; the pages are worked with the keys")?;
                self.app.touchscreen_missing();
            }
        }
        self.app.state.prepared = crate::prepared();
        self.tick(now)?;
        // The partition is open: whatever ended the starts before this one
        // is done with, so a start that follows tries it again (2026-11-19).
        if !self.reached && matches!(self.app.state.data, Data::Open | Data::Erased) {
            self.reached = true;
            skyward_sys::starts_ended_well();
        }
        // The opening kept back for the phone's own parts starts the turn they
        // are up, rather than the turn the page asked for it.
        if self.pending_open.is_some() && crate::prepared() {
            let job = self.pending_open.take().expect("checked above");
            skyward_sys::kmsg("the phone's own parts are up; the opening starts now")?;
            self.start_decrypt(job)?;
        }
        self.take_readings(now)?;
        // In fastboot the page follows fastbootd and the cable each second.
        if self.fastboot && self.port_at.elapsed().as_secs() >= 1 {
            let (running, link) = fastboot_status()?;
            self.app.set_fastboot(running, link);
            self.port_at = Instant::now();
        }
        let (lit, shown) = self.show(now)?;
        self.wait(sleeper, uevents, now, lit, shown)?;
        let now = monotonic_ms();
        for slept in sleeper.slept() {
            match slept {
                Slept::Woke => skyward_sys::kmsg("the phone slept and woke")?,
                Slept::Refused(why) => skyward_sys::kmsg(&format!("the phone did not sleep: {why}"))?,
            }
        }
        if uevents.heard()? {
            self.readings.ask();
            self.asked = Some(Instant::now());
        }
        let mut drive_told = None;
        if self.drive_at.elapsed().as_secs() >= 1 {
            drive_told = look_for_drive(&mut self.plugged, &mut self.seen)?;
            self.drive_at = Instant::now();
        }
        self.installer_told(now)?;
        self.eraser_told(now)?;
        self.files_told(now)?;
        self.relabel_told(now);
        self.parts_told(now)?;
        self.backup_told(now);
        self.merge_told(now)?;
        self.root_told()?;
        self.systems_told(now)?;
        self.wlan_told()?;
        self.keep_settings(now)?;
        self.updates_told(now)?;
        self.skyward_told(now)?;
        self.decrypt_told(now)?;
        self.decrypt_stalled(now)?;
        self.shell_told()?;
        self.take_in(drive_told, now)?;
        self.offer_storage(now)
    }

    /// Start opening the data partition now where the phone's own parts are
    /// up, and keep the job for the loop to start where they are not: the key
    /// service it needs is one of them, and a page drawn before they come up
    /// would fail on it.
    fn start_or_wait(&mut self, job: decrypt::Job) -> Result<()> {
        if crate::prepared() {
            self.start_decrypt(job)?;
        } else {
            skyward_sys::kmsg("the phone's own parts are not up yet; the opening waits for them")?;
            self.pending_open = Some(job);
        }
        Ok(())
    }

    /// Start a job on the data partition and note when it began: the page
    /// waits on it, so it is not to be a job that never comes back.
    fn start_decrypt(&mut self, job: decrypt::Job) -> Result<()> {
        self.decryptor = Some(Decryptor::start(job)?);
        self.decrypt_at = Instant::now();
        self.opening_said = None;
        Ok(())
    }

    /// What time passing asks for: the motor's clicks and ticks, and the
    /// status bar's clock drawn again as its minute turns.
    fn tick(&mut self, now: u64) -> Result<()> {
        // How far opening the data partition has come, read from the work
        // itself rather than guessed at: the bar on the page steps on toward
        // it every frame the page is up.
        if matches!(self.app.state.data, skyward_ui::app::Data::Closed | skyward_ui::app::Data::Opening) {
            self.app.set_data_progress(skyward_crypto::data::progress());
        }
        for fx in self.app.tick(now) {
            match fx {
                Effect::Haptic => self.click()?,
                // The power key held past its wait for volume down turns
                // the screen off.
                Effect::Ticks => self.ticks()?,
                // A page may ask for more as time passes -- the screenshot the
                // control centre's tile waits for the sheet to leave -- and
                // that goes the way a page's own effect does.
                other => {
                    let mut queue = VecDeque::from(self.effect(other, now)?);
                    while let Some(next) = queue.pop_front() {
                        queue.extend(self.effect(next, now)?);
                    }
                }
            }
        }
        let time = clock(self.app.state.time_zone)?;
        if time != self.shown_clock {
            self.shown_clock = time;
            self.app.redraw();
        }
        // The lock screen shows the date under the time, which turns once a day.
        let day = skyward_ui::fmt::date(self.app.state.lang, unix_now()?, self.app.state.time_zone);
        if day != self.app.state.date {
            self.app.state.date = day;
            self.app.redraw();
        }
        Ok(())
    }

    /// One click of the motor, as strong as the settings say.
    fn click(&mut self) -> Result<()> {
        let (cfg, vibration) = (self.cfg, self.app.state.vibration);
        gentle_click(&mut self.haptics, cfg, vibration, false);
        Ok(())
    }

    /// The motor's ticks of the screen going off.
    fn ticks(&mut self) -> Result<()> {
        let (cfg, vibration) = (self.cfg, self.app.state.vibration);
        gentle_click(&mut self.haptics, cfg, vibration, true);
        Ok(())
    }

    /// Take in the readings taken since the last turn.
    fn take_readings(&mut self, now: u64) -> Result<()> {
        for reading in self.readings.taken() {
            match reading {
                Reading::Battery(battery, at) => {
                    if self.asked.is_some_and(|asked| at >= asked) {
                        self.asked = None;
                    }
                    untaken(&mut self.unread.battery, "the battery", battery.as_ref().err())?;
                    if let Ok(battery) = battery
                        && battery != self.info.battery
                    {
                        plug(&mut self.app, &self.info.battery, &battery, now)?;
                        self.info.battery = battery;
                        self.app.refresh();
                    }
                }
                Reading::All(all, at) => {
                    if self.asked.is_some_and(|asked| at >= asked) {
                        self.asked = None;
                    }
                    untaken(&mut self.unread.details, "the phone's details", all.as_ref().as_ref().err())?;
                    if let Ok(all) = *all
                        && at >= self.read_at
                        && all != self.info
                    {
                        plug(&mut self.app, &self.info.battery, &all.battery, now)?;
                        self.info = all;
                        self.app.refresh();
                    }
                }
            }
        }
        Ok(())
    }

    /// 开一段录屏：目录建好、文件打开，三个编码线程和一个写线程拉起来。
    fn start_recording(&mut self) -> Result<(skyward_ui::record::Recorder, String, String)> {
        let (w, h) = self.surface.size();
        ensure!(w > 0 && h > 0, "the panel has no size");
        let folder = Path::new(skyward_crypto::data::SDCARD).join(RECORDINGS);
        let there = folder.ancestors().find(|p| p.exists()).context("the root is always there")?;
        let ownership = ownership(there)?;
        skyward_files::make_folders(&folder, ownership)?;
        // 录成多大、多清楚、多少帧，是录屏页上那三条定的。容器只有 MP4
        // （里面是 H.264 Baseline），一次录屏只开一个文件。
        let r = &self.app.state.record;
        let settings = skyward_ui::record::Settings::of((w, h), r.scale, r.quality, r.fps);
        let stamp = unix_ms_now()?;
        let zone = self.app.state.time_zone;
        let container = skyward_ui::record::Container::ONLY;
        let name = skyward_ui::fmt::record_name_as(stamp, zone, container.extension());
        let path = folder.join(&name);
        let file = skyward_files::make_file(&path, ownership)?;
        let take = skyward_ui::record::Take { file, path };
        let rec = skyward_ui::record::Recorder::start(take, (w, h), settings)?;
        let spec = format!(
            "{}×{} · {:.1} Mbps · {} fps · {}",
            settings.width,
            settings.height,
            settings.bitrate() as f64 / 1e6,
            settings.fps,
            container.spec()
        );
        Ok((rec, name, spec))
    }

    /// 马达还没装上就再试一次，一秒最多一次：驱动的 FF 设备比页面晚到
    /// （模块在页面起来之后才装，cs40l26 的固件又要在 probe 里下载），
    /// 而这件事原先只在有人点的时候试 —— 第一次点按就没有震动。试够
    /// [`MOTOR_ATTEMPTS`] 次还没来就不再试：这台机器真没有马达的时候，
    /// 不该每秒枚举一次 /dev/input 到永远。
    fn take_up_motor(&mut self) {
        if self.haptics.is_some() || self.motor_tries >= MOTOR_ATTEMPTS {
            return;
        }
        let Some(wanted) = self.cfg.haptics.as_ref() else { return };
        if self.motor_try.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
            return;
        }
        self.motor_try = Some(Instant::now());
        self.motor_tries += 1;
        if let Ok(h) = Haptics::open(wanted) {
            let _ = skyward_sys::kmsg("the motor came up; the clicks play");
            self.haptics = Some(h);
        }
    }

    /// Put the page on the panel where it changed, and light the panel,
    /// rest it or turn it off as the page asks. Gives whether the panel is
    /// lit, and whether a frame was shown.
    fn show(&mut self, now: u64) -> Result<(bool, bool)> {
        self.take_up_motor();
        let panel = self.app.panel(now);
        let mut shown = false;
        if panel.is_some() {
            // A display turned off comes on again with a frame drawn for it.
            if self.app.take_dirty() || !self.surface.is_on() {
                let t0 = Instant::now();
                self.app.compose(&mut self.screen, &self.fonts, &self.info, &self.shown_clock, now);
                // 录屏：这一帧先交给编码线程，再送去显示 —— 编码慢的时候页面
                // 不会等它（队列满了就丢这一帧，写线程补上一帧）。
                if let Some(rec) = &self.recorder {
                    rec.push(&self.screen);
                }
                self.surface.present(&self.screen)?;
                // 一帧画得久了就记一条：翻页卡顿这类问题靠它定位。
                let ms = t0.elapsed().as_millis();
                if ms >= 50 {
                    let _ = skyward_sys::kmsg(&format!("frame {:?} took {ms} ms", self.app.screen()));
                }
                if let Some(path) = self.dump {
                    fs::write(path, &self.screen.data).with_context(|| format!("write {}", path.display()))?;
                }
                shown = true;
            }
            self.blanked = false;
        } else if !self.blanked {
            // Black out the panel's frame memory before the backlight goes
            // to 0 and the display off, so an OLED that floors the backlight
            // above zero rather than cutting it shows nothing as it goes off.
            self.screen.data.fill(0);
            self.surface.present(&self.screen)?;
            self.surface.settle()?;
            self.blanked = true;
        }
        // After the frame is on the panel, so a panel coming back on lights
        // with what it should show now, never with what it showed when it
        // went off.
        if self.backlight.level == 0 && panel.is_some() {
            self.surface.settle()?;
        }
        self.backlight.show(panel)?;
        if panel.is_some() {
            // Lit, a panel that came on again switches to its fastest
            // refresh.
            self.surface.speed_up()?;
            self.rested_at = None;
        } else if self.app.locked() {
            // The lock waits behind the dark panel for two taps or the knob:
            // a rested panel stops reporting touches, so it is not rested. What
            // that costs in power a recovery spends anyway.
            self.rested_at = Some(Instant::now());
        } else if self.surface.is_on() {
            // With the backlight off, the panel rests, and the touch
            // controller following it stops drawing power.
            self.surface.rest()?;
            self.rested_at = Some(Instant::now());
        } else if self.rested_at.is_some_and(|at| at.elapsed() >= REST) {
            // Rested long enough, the display goes off: the panel and the
            // display processor feeding it stop drawing power.
            self.surface.turn_off()?;
            self.rested_at = None;
        }
        Ok((panel.is_some(), shown))
    }

    /// Wait until something the loop listens to has something, or the page
    /// is due to change; the phone is let sleep meanwhile where nothing
    /// needs it awake. `lit` and `shown` are as [`Pages::show`] gave them.
    fn wait(&mut self, sleeper: &Sleeper, uevents: &uevent::Listener, now: u64, lit: bool, shown: bool) -> Result<()> {
        // A page at rest draws the rows it has yet to while nothing else
        // is to be done, a band at a time, looking for input between bands.
        let ahead = lit && !shown && self.app.draw_ahead(&self.fonts, &self.info);
        // A frame shown has waited for the one before it to reach the
        // panel, which keeps the loop to the panel's pace; otherwise the CPU
        // gets at least a millisecond. While a package is being written the
        // page is drawn ten times a second at most: a bar that moves is
        // worth a frame, and the cores a full-rate redraw would take are
        // worth more to update_engine, which spends minutes on one of them
        // (2026-11-20). Every other state the installing page has -- a file
        // being looked at, an update waiting to be agreed to -- is drawn at
        // the panel's own pace: held to ten a second, the page that slides
        // in stutters (2026-11-21).
        let rooting = matches!(self.app.state.install, skyward_ui::app::Install::Rooting { .. });
        let writing = self.installer.is_some() && self.app.state.install.writes();
        // The boot image is a few megabytes: at a tenth of a second a frame
        // the bar would move in a handful of jumps, so this one is drawn at
        // the panel's own pace (2026-11-21).
        // 录屏的时候按面板的节奏画：画面就是录下来的画面，页面卡在十帧/秒，
        // 录出来也只有十帧/秒（2026-11-21）。
        let floor = if self.app.state.record.on {
            0
        } else if rooting {
            33
        } else if writing {
            100
        } else if shown {
            0
        } else {
            1
        };
        let until_due = self.app.deadline(now).map(|d| d.saturating_sub(now) as i32);
        // A resting panel turns off once its rest is over.
        let rest_left = self.rested_at.map(|at| REST.saturating_sub(at.elapsed()).as_millis() as i32);
        let timeout = if ahead { 0 } else { until_due.into_iter().chain(rest_left).min().map_or(1000, |t| t.clamp(floor, 1000)) };
        sleeper.allow(self.may_sleep()?);
        let mut fds = vec![self.inputs.fd(), uevents.fd(), sleeper.fd()];
        if let Some(i) = &self.installer {
            fds.push(i.fd());
        }
        if let Some(c) = &self.control {
            fds.push(c.fd());
        }
        if let Some(e) = &self.eraser {
            fds.push(e.fd());
        }
        if let Some(sh) = &self.shell {
            fds.extend(sh.fds());
        }
        if let Some(w) = &self.worker {
            fds.push(w.fd());
        }
        if let Some(r) = &self.relabeler {
            fds.push(r.fd());
        }
        if let Some(b) = &self.backuper {
            fds.push(b.fd());
        }
        if let Some(m) = &self.merger {
            fds.push(m.fd());
        }
        if let Some(r) = &self.rooting {
            fds.push(r.worker.fd());
        }
        if let Some(d) = &self.decryptor {
            fds.push(d.fd());
        }
        if let Some(p) = &self.part_worker {
            fds.push(p.fd());
        }
        if let Some(w) = &self.wlan {
            fds.push(w.fd());
        }
        if let Some(c) = &self.checker {
            fds.push(c.fd());
        }
        if let Some(d) = &self.downloader {
            fds.push(d.fd());
        }
        if let Some(c) = &self.skyward_checker {
            fds.push(c.fd());
        }
        if let Some(u) = &self.skyward_updater {
            fds.push(u.fd());
        }
        if let Some(s) = &self.systems_worker {
            fds.push(s.fd());
        }
        fds.push(self.readings.fd());
        wait(&fds, timeout)
    }

    /// Whether the phone may sleep. With the display off, it sleeps unless
    /// something needs it awake: work under way, fastboot or sideload, a
    /// restart on its way, a command running in the terminal, a network
    /// being joined, the battery read after a uevent, or a computer on the
    /// USB port, whose adb and file transfer stay up.
    fn may_sleep(&self) -> Result<bool> {
        let systems_busy = self.systems_worker.as_ref().is_some_and(systems::Worker::busy);
        let busy = self.installer.is_some()
            || self.eraser.is_some()
            || self.worker.is_some()
            || self.relabeler.is_some()
            || self.backuper.is_some()
            || self.merger.is_some()
            || self.decryptor.is_some()
            || self.part_worker.is_some()
            || self.checker.is_some()
            || self.downloader.is_some()
            || self.skyward_checker.is_some()
            || self.skyward_updater.is_some()
            || systems_busy;
        let joining = self.app.state.wlan.as_ref().and_then(|w| w.joined.as_ref()).is_some_and(|j| matches!(j.stage, Joining::Joining | Joining::Addressing));
        let running = match &self.shell {
            Some(sh) => !sh.waiting()?,
            None => false,
        };
        Ok(self.surface.is_off()
            && !busy
            && !self.fastboot
            && !self.sideloading
            && self.leave_at.is_none()
            && !joining
            && !running
            && self.asked.is_none()
            && usb_link()? != UsbLink::Connected)
    }

    /// The request in misc is done with: a restart from here on starts the
    /// system rather than coming back to what it asked.
    fn done_with_request(&mut self) -> Result<()> {
        if self.kept {
            bcb::clear(&self.misc)?;
            self.kept = false;
        }
        Ok(())
    }

    /// Clear a `boot-recovery` left in misc with nothing for the recovery to
    /// do. The bootloader sends the phone here for as long as it stands
    /// there, so a phone whose recovery partition no longer holds Skyward,
    /// or whose recovery was started once by the keys, comes back to it at
    /// every restart and never starts its system: clearing it here is what
    /// ends that (2026-11-20). A request that does ask for work is left to
    /// [`done_with_request`] once the work ends, as Android's own recovery
    /// leaves it, so a restart halfway comes back to it.
    fn forget_stale_request(&mut self) -> Result<()> {
        if !matches!(self.request.work, Work::Menu | Work::Exit) {
            return Ok(());
        }
        let asked = bcb::read(&self.misc)?;
        if asked.command.is_empty() {
            return Ok(());
        }
        skyward_sys::kmsg(&format!(
            "misc holds {:?} with nothing for the recovery to do; cleared, so a restart starts the system",
            asked.command
        ))?;
        bcb::clear(&self.misc)?;
        self.kept = false;
        Ok(())
    }

    /// An update that went into `target` leaves snapshots that the slot
    /// starts from until they merge, and a slot started from snapshots
    /// without a system that merges them comes back to the recovery instead
    /// of starting: a phone restarted between the install and the merge
    /// never gets in (2026-11-20). The merge is started here rather than
    /// left to whoever restarts next.
    fn merge_after_install(&mut self, target: skyward_vab::Slot) {
        if self.merger.is_some() {
            return;
        }
        let seen = self.info.slot.unwrap_or(target);
        match skyward_vab::standing(seen) {
            // An update that wrote over the running system leaves nothing
            // to merge, and one that has merged needs nothing either.
            Ok(Standing::None | Standing::Merged) => {
                let _ = skyward_sys::kmsg(&format!("the update in the {target:?} slot has nothing left to merge"));
                return;
            }
            // Its snapshots are compressed, which the system merges with
            // snapuserd; a recovery does not run it.
            Ok(Standing::Compressed) => {
                let _ = skyward_sys::kmsg("the update's snapshots are compressed; the system merges those itself");
                return;
            }
            Ok(standing) => {
                let _ = skyward_sys::kmsg(&format!("the update in the {target:?} slot stands as {standing:?}; merging it now"));
            }
            Err(e) => {
                let _ = skyward_sys::kmsg(&format!("where the update in the {target:?} slot stands is not read: {e:#}"));
            }
        }
        self.start_merge(target);
    }

    /// Start the merge of `target` and follow it on the page. The phone does
    /// not leave for the system when it ends: a restart is the user's to ask
    /// for, on the page that says the update is done (2026-11-20).
    fn start_merge(&mut self, target: skyward_vab::Slot) {
        self.app.set_update(Standing::Merging(0.0));
        self.merge_done = false;
        match merge::Merger::start(target, true) {
            Ok(merger) => {
                let _ = skyward_sys::kmsg(&format!("the update in the {target:?} slot merges; the page shows how far it is"));
                self.merger = Some(merger);
            }
            // A merge that cannot start is not a reason to keep the phone
            // here: the page says what became of the update, and a restart
            // is still the user's to ask for.
            Err(e) => {
                let _ = skyward_sys::kmsg(&format!("the update in the {target:?} slot does not merge: {e:#}"));
                self.app.set_update(skyward_vab::standing(target).unwrap_or(Standing::Unknown));
            }
        }
    }

    /// /metadata keeps where an update stands, and the recovery's own
    /// settings. It is not mounted when the pages are built, so it is
    /// mounted read-only here and left mounted while they are up: the
    /// merge needs it, and a merge takes it writable for itself.
    fn open_metadata(&mut self) {
        let at = Path::new("/metadata");
        if skyward_block::mount::is_mount_point(at).unwrap_or(false) {
            return;
        }
        let device = Path::new(skyward_block::BY_NAME).join("metadata");
        if let Err(e) = skyward_block::mount::mount_ro(&device, at) {
            let _ = skyward_sys::kmsg(&format!("{} is not mounted: {e:#}", at.display()));
        }
    }

    /// The slot an update waits in to be merged, for the phone running
    /// from `current`: written into this slot and waiting, or half merged
    /// before the phone restarted. An update written into the other slot
    /// merges after the phone has started from it, which a recovery does
    /// not do here.
    fn merge_target(&self, current: skyward_vab::Slot) -> Option<skyward_vab::Slot> {
        match skyward_vab::standing(current) {
            Ok(Standing::Waiting) => Some(current),
            Ok(Standing::Merging(_) | Standing::Failed(_)) => match skyward_vab::target_slot() {
                Ok(Some(target)) => Some(target),
                _ => Some(current),
            },
            _ => None,
        }
    }

    /// An update the phone could not merge before it restarted is taken up
    /// as the recovery comes up. A phone cannot start from snapshots, so a
    /// phone left with them comes back here at every restart; merging them
    /// as the pages come up is what lets it start at all, without anyone
    /// having to choose to merge first (2026-11-20).
    fn resume_merge(&mut self) -> Result<()> {
        // Work that writes the partitions or /metadata keeps the merge
        // away until it has ended; a page's own start, a restart into the
        // system and an unread request do not (2026-11-20).
        if self.merger.is_some() || self.request.work.lasts() {
            return Ok(());
        }
        let Some(current) = self.info.slot else { return Ok(()) };
        let Some(target) = self.merge_target(current) else {
            // What the phone reads there goes into the kernel log, so that
            // a phone that will not start says why without a page to ask
            // (2026-11-20).
            let _ = skyward_sys::kmsg(&format!(
                "no update waits to be merged in the {current:?} slot: {:?}",
                skyward_vab::standing(current)
            ));
            return Ok(());
        };
        skyward_sys::kmsg(&format!("an update stands in the {target:?} slot to be merged; taking it up now"))?;
        self.start_merge(target);
        Ok(())
    }

    /// Copies of the boot images that older versions left on internal
    /// storage take room and are of no use: root is patched back into the
    /// partition it lives in, before anything restarts, so nothing has to be
    /// picked up by hand (2026-11-20).
    fn forget_boot_set(&mut self) {
        let at = Path::new(skyward_crypto::data::SDCARD).join("Skyward").join("boot");
        if !at.is_dir() {
            return;
        }
        match fs::remove_dir_all(&at) {
            Ok(()) => {
                let _ = skyward_sys::kmsg(&format!("the boot images older versions saved in {} are cleared", at.display()));
            }
            Err(e) => {
                let _ = skyward_sys::kmsg(&format!("the boot images older versions saved in {} are not cleared: {e:#}", at.display()));
            }
        }
    }

    /// Put the phone's root back into the boot image of `slot`, the slot
    /// that starts next, on a thread of its own: the image the update wrote
    /// there is patched again with the phone's own tool, and where that
    /// cannot be done, the image the phone runs from is carried over whole
    /// (2026-11-21: the update's own image first, since it is made for the
    /// system that starts from it).
    fn start_root(&mut self, slot: skyward_vab::Slot, wipe: bool) {
        // The page goes on from the merge's bar to this at once: what it
        // shows is a few megabytes being read, patched, written and read
        // back, and a page that sits still for it looks like one that has
        // stopped (2026-11-21).
        self.app.rooting(skyward_install::root::Story::Reading, 0, 0, slot, wipe);
        let data = PathBuf::from(skyward_crypto::data::MOUNT);
        let dir = PathBuf::from(skyward_block::BY_NAME);
        let work = std::env::temp_dir().join("skyward-root");
        let started = skyward_sys::worker::Worker::start("root", move |tell| {
            let mut last = Instant::now();
            let mut story = skyward_install::root::Story::Reading;
            let mut said = |now: skyward_install::root::Story, done: u64, total: u64| {
                // An image is a handful of megabytes, and the panel draws
                // thirty frames a second: a new step and the end of one are
                // told at once, the rest at the panel's pace (2026-11-21).
                if now != story || done >= total || last.elapsed() >= ROOT_TELL_EVERY {
                    story = now;
                    last = Instant::now();
                    tell.tell(RootSaid::Step(now, done, total));
                }
            };
            let rooted = skyward_install::root::put_back(&data, &dir, slot, &work, &mut said);
            tell.tell(RootSaid::Done(Box::new(rooted)));
        });
        match started {
            Ok(worker) => {
                let _ = skyward_sys::kmsg(&format!(
                    "root: the image the {slot:?} slot starts from is being given the phone's root; the page follows it"
                ));
                self.rooting = Some(Rooting { slot, wipe, worker });
            }
            // A thread that does not start leaves the slot that starts next
            // without the root, which the page says as it says any patch
            // that did not land (2026-11-21).
            Err(e) => {
                let _ = skyward_sys::kmsg(&format!("root: not put back, the thread for it does not start: {e:#}"));
                let rooted = skyward_install::root::Rooted::Missed { what: "root" };
                self.app.rooted_after_merge(rooted, slot, wipe);
                self.app.set_update(Standing::Merged);
                self.app.refresh();
            }
        }
    }

    /// What putting the root back told since the last turn, and its end: the
    /// page follows each step, and says the install is done once the image
    /// the slot starts from holds the root the phone has (2026-11-21).
    fn root_told(&mut self) -> Result<()> {
        let (events, ended, slot, wipe) = match &self.rooting {
            Some(rooting) => (rooting.worker.events(), rooting.worker.ended(), rooting.slot, rooting.wipe),
            None => return Ok(()),
        };
        let mut ended_with = None;
        for ev in events {
            match ev {
                RootSaid::Step(story, done, total) => self.app.rooting(story, done, total, slot, wipe),
                RootSaid::Done(rooted) => ended_with = Some(*rooted),
            }
        }
        let rooted = match ended_with {
            Some(rooted) => rooted,
            // A thread that came apart says nothing: the page says the patch
            // did not land rather than showing a bar for ever.
            None if ended => {
                let _ = skyward_sys::kmsg("root: the thread that puts it back ended without saying how it went");
                skyward_install::root::Rooted::Missed { what: "root" }
            }
            None => return Ok(()),
        };
        self.rooting = None;
        self.app.rooted_after_merge(rooted, slot, wipe);
        // The restart the page offers is the user's to ask for, and the
        // update is where the readings say it stands (2026-11-20).
        if self.merge_done {
            self.app.set_update(Standing::Merged);
            skyward_sys::kmsg("the update has merged and the phone's root is back in the slot that starts next; the phone waits for a restart to be asked for")?;
        }
        self.app.refresh();
        Ok(())
    }

    /// The mount page shows the partitions as they are when it opens and
    /// after each change; why they cannot be read is the page's to say.
    fn mounts(&self) -> Result<Vec<skyward_ui::app::MountRow>, String> {
        read_mounts(self.programs).map_err(|e| format!("{e:#}"))
    }
}

/// What the work under way told since the last turn, each kind of work
/// taken in by a method of its own.
impl Pages<'_> {
    /// What the installer told: its page follows it, and its end starts
    /// what the package, the request or rescue asked for after it.
    fn installer_told(&mut self, now: u64) -> Result<()> {
        use skyward_install::{Event as Told, Outcome};
        let told = self.installer.as_ref().map(Installer::events).unwrap_or_default();
        for ev in told {
            let finished = matches!(ev, Told::Finished(_));
            let installed = matches!(ev, Told::Finished(Outcome::Installed { .. } | Outcome::Ran { .. }));
            let wipe_after = matches!(ev, Told::Finished(Outcome::Installed { wipe: true, .. }));
            let script_cache = matches!(ev, Told::Finished(Outcome::Ran { wipe_cache: true, .. }));
            // The slot an update went into, taken before the page is told and
            // the installer let go of.
            let went_in = match &ev {
                Told::Finished(Outcome::Installed { slot, .. }) => Some(*slot),
                _ => None,
            };
            // A second system installed is read for the pages to show.
            if matches!(ev, Told::Done(Outcome::SystemInstalled { .. }) | Told::Finished(Outcome::SystemInstalled { .. })) {
                self.systems_worker.as_mut().context("a second system went in on a phone that cannot hold one")?.send(systems::Task::Read);
            }
            eprintln!("install {ev:?}");
            match &ev {
                // Rescue's own erasing tells rescue how it went once it
                // ends; file transfer that will not let internal
                // storage go keeps it from starting.
                Told::Wipe => {
                    ensure!(self.eraser.is_none(), "erasing is under way already");
                    if withhold(&mut self.transfer, &mut self.app, now)? {
                        self.eraser = Some(rescue_wipe(&mut self.app)?);
                        self.rescue_erasing = true;
                    } else {
                        self.installer.as_ref().context("rescue asked for erasing")?.wiped(false);
                    }
                }
                Told::Reboot(target) => self.rescue_leaves = Some(rescue_target(target)),
                _ => {}
            }
            // The update is written: the merge starts before the page hears
            // the end, so that the bar that followed the writing goes
            // straight on with the merge instead of showing a page that
            // says the install is done while the phone cannot start yet
            // (2026-11-20). Root goes back once that merge has ended, since
            // what it writes lands in the partitions root lives in and
            // would write the carried image over; where there is no merge
            // to wait for, it goes back now.
            if let Some(slot) = went_in {
                // A boot-recovery request left in misc has done its work:
                // kept, it would send the phone back here at every restart,
                // to a recovery the package may have written over.
                bcb::clear(&self.misc)?;
                self.kept = false;
                self.merge_after_install(slot);
                self.forget_boot_set();
                self.root_slot = Some(slot);
                if self.merger.is_none() {
                    self.root_slot = None;
                    self.start_root(slot, wipe_after);
                }
            }
            self.app.install(ev, now);
            if finished {
                self.installer = None;
                self.sideloading = false;
                // A sideload or an update the system asked for is done
                // with however it went, and so is rescue, once its page
                // is left or the computer asks the phone to leave. A
                // sideload asked to leaves on its own after it, and an
                // update that went in does, once what the package or the
                // request asked to be erased is.
                if matches!(self.request.work, Work::Sideload { .. } | Work::Update(_) | Work::Rescue) {
                    self.done_with_request()?;
                }
                let (leaves, cache_after) = match &self.request.work {
                    Work::Sideload { auto_reboot } => (*auto_reboot, script_cache),
                    Work::Update(update) => (installed, script_cache || (installed && update.wipe_cache)),
                    _ => (false, script_cache),
                };
                if cache_after && !self.app.state.cache_partition {
                    skyward_sys::kmsg("the install asked for the cache partition to be erased, and the phone has none")?;
                }
                // The update wrote the boot images over, and the slot it
                // went into starts from snapshots until they merge: both
                // were taken up before the page heard the end
                // (2026-11-20).
                if wipe_after {
                    ensure!(self.eraser.is_none(), "erasing is under way already");
                    self.eraser = Some(factory_reset(&mut self.app, false, false, None)?);
                    self.erase_leaves = leaves;
                } else if cache_after && self.app.state.cache_partition {
                    ensure!(self.eraser.is_none(), "erasing is under way already");
                    self.eraser = Some(parts_wipe(&mut self.app, vec![Step::Cache], &self.cfg.wipe.logs, false, None)?);
                    self.erase_leaves = leaves;
                } else if leaves {
                    // A merge that is still going needs the phone to stay:
                    // restarting part way leaves the other slot with
                    // snapshots it cannot start from. The page says when it
                    // has merged, and the restart is the user's to ask for
                    // (2026-11-20).
                    if self.merger.is_some() {
                        skyward_sys::kmsg("the update merges first; the page says when it may be restarted")?;
                    } else {
                        self.leave_at = Some(now + LEAVE_AFTER_MS);
                    }
                }
                // Rescue asked to come back is told so in misc, as
                // Android's own recovery tells it.
                if let Some((target, again)) = self.rescue_leaves.take() {
                    if again {
                        bcb::write(&self.misc, &bcb::Bcb { command: "boot-rescue".into(), args: Vec::new() })?;
                    }
                    power(target)?;
                }
            }
        }
        Ok(())
    }

    /// What the eraser told: its page follows it, or rescue's does, and
    /// its end is the end of what asked for it.
    fn eraser_told(&mut self, now: u64) -> Result<()> {
        let told = self.eraser.as_ref().map(Eraser::events).unwrap_or_default();
        let mut erased = false;
        for ev in told {
            match ev {
                // Rescue's page says only that erasing is under way.
                wipe::Event::At(_) | wipe::Event::Progress(..) if self.rescue_erasing => {}
                wipe::Event::At(step) => self.app.erase_at(part(step)),
                wipe::Event::Progress(done, total) => self.app.erase_progress(done, total),
                // Rescue stays on: its request stays in misc, and the
                // computer hears how erasing went.
                wipe::Event::Ended(outcome) if self.rescue_erasing => {
                    let said = outcome.as_ref().map_or_else(wipe::said, |_| "done".to_string());
                    skyward_sys::kmsg(&format!("erasing for rescue ended: {said}"))?;
                    self.installer.as_ref().context("rescue waits for erasing to end")?.wiped(outcome.is_ok());
                    self.app.rescue_wiped(outcome.map_err(erase_stop), now);
                    if self.app.state.data == Data::Erased {
                        self.closed.clear();
                    }
                    self.rescue_erasing = false;
                    erased = true;
                }
                wipe::Event::Opened(opening) => {
                    if let Some(mounted) = &opening.mounted {
                        log_mounted(mounted)?;
                    }
                    self.closed = opening.closed;
                    show_closed(&mut self.app, &self.closed)?;
                }
                wipe::Event::Ended(outcome) => {
                    let said = outcome.as_ref().map_or_else(wipe::said, |_| "done".to_string());
                    skyward_sys::kmsg(&format!("erasing ended: {said}"))?;
                    // As with the phone's own recovery, the request is
                    // done with either way, so a restart starts the
                    // system rather than erasing again. A factory
                    // reset done is what an update that asked for one
                    // asked, too.
                    self.done_with_request()?;
                    if outcome.is_ok() && self.app.state.erase.reset {
                        bcb::clear(&self.misc)?;
                    }
                    if outcome.is_ok() && self.erase_leaves {
                        // An update that went in before this may still be
                        // merging: it finishes first, and the page says when
                        // the phone may be restarted (2026-11-20).
                        if self.merger.is_none() {
                            self.leave_at = Some(now + LEAVE_AFTER_MS);
                        }
                    }
                    self.app.erase_ended(outcome.map_err(erase_stop), now);
                    // Users of a data partition erased whole have no
                    // files left to open.
                    if self.app.state.data == Data::Erased {
                        self.closed.clear();
                    }
                    erased = true;
                }
            }
        }
        // Erasing all data makes /metadata afresh, and the settings with it
        // are written there again.
        if erased {
            self.eraser = None;
            self.keeper.remade();
        }
        Ok(())
    }

    /// What copying, moving or deleting told; once it has ended, the
    /// folder shown is read again for what the work changed.
    fn files_told(&mut self, now: u64) -> Result<()> {
        let told = self.worker.as_ref().map(filework::Worker::events).unwrap_or_default();
        let mut worked = false;
        for ev in told {
            match ev {
                filework::Event::Progress(p) => self.app.file_progress((p.bytes, p.total_bytes), (p.entries, p.total_entries)),
                filework::Event::Relabeling(_) => unreachable!("work on files gives no labels"),
                filework::Event::Ended(end) => {
                    self.app.file_job_ended(job_end(end), now);
                    worked = true;
                }
            }
        }
        if worked {
            self.worker = None;
            // The page the work was asked from is the one read again: a file
            // put on the OTG drive leaves that page stale otherwise, and the
            // file manager's own folder untouched.
            match self.app.screen() {
                Screen::FilesOtg => {
                    let folder = self.app.state.otg.path.clone();
                    list_otg(&mut self.app, folder)?;
                }
                Screen::Choose(place) => {
                    let folder = self.app.state.folder(Screen::Choose(place)).path.clone();
                    list_place(&mut self.app, place, folder)?;
                }
                _ => {
                    let folder = self.app.state.files.path.clone();
                    list(&mut self.app, folder)?;
                }
            }
        }
        Ok(())
    }

    /// What giving the data partition's files their labels told.
    fn relabel_told(&mut self, now: u64) {
        let told = self.relabeler.as_ref().map(filework::Worker::events).unwrap_or_default();
        let mut relabeled = false;
        for ev in told {
            match ev {
                filework::Event::Relabeling(p) => self.app.relabel_progress(p.checked, p.total, p.changed),
                filework::Event::Progress(_) => unreachable!("relabeling moves no bytes"),
                filework::Event::Ended(end) => {
                    self.app.relabel_ended(job_end(end), now);
                    relabeled = true;
                }
            }
        }
        if relabeled {
            self.relabeler = None;
        }
    }

    /// What the work on whole partitions told.
    fn parts_told(&mut self, now: u64) -> Result<()> {
        let told = self.part_worker.as_ref().map(partwork::Worker::events).unwrap_or_default();
        let mut parted = false;
        for ev in told {
            match ev {
                partwork::Event::Listed(names) => self.app.part_listed(names),
                partwork::Event::At(index) => self.app.part_at(index),
                partwork::Event::Ended(index, item, said) => {
                    skyward_sys::kmsg(&format!("partition work: item {index} ended as {item:?}"))?;
                    self.app.part_ended(index, item_end(item), &said);
                }
                partwork::Event::Done(done) => {
                    skyward_sys::kmsg(&format!("partition work ended: {done:?}"))?;
                    let work = self.app.state.part_work.as_ref().context("partition work shows on its page")?;
                    let whole = done.is_ok() && !work.items.iter().any(|(_, s)| matches!(s, ItemState::Ended(ItemEnd::Failed(_))));
                    let data = work.items.iter().any(|(name, _)| name == DATA_PARTITION);
                    let asked = work.asked;
                    // What the system asked for is done with either way,
                    // and the phone leaves on its own once it went
                    // through every partition.
                    if asked {
                        self.done_with_request()?;
                        if whole {
                            self.leave_at = Some(now + LEAVE_AFTER_MS);
                        }
                    }
                    // Work on the data partition leaves it closed.
                    if data {
                        self.closed.clear();
                    }
                    self.app.part_work_ended(done.map_err(part_stop), now);
                    parted = true;
                }
            }
        }
        // Work on whole partitions may have erased or made afresh the one
        // the settings are kept on.
        if parted {
            self.part_worker = None;
            self.keeper.remade();
        }
        Ok(())
    }

    /// What backing up or restoring told.
    fn backup_told(&mut self, now: u64) {
        let told = self.backuper.as_ref().map(backup::Worker::events).unwrap_or_default();
        let mut backed = false;
        for ev in told {
            match ev {
                backup::Event::Plan(plan) => self.app.backup_plan(plan),
                backup::Event::At { item, done, of, checking } => self.app.backup_at(BackupAt { item, done, of, checking }),
                backup::Event::Ended(end) => {
                    self.app.backup_ended(backup_end(end), now);
                    backed = true;
                }
            }
        }
        if backed {
            self.backuper = None;
        }
    }

    /// What the merge followed told.
    fn merge_told(&mut self, now: u64) -> Result<()> {
        let Some(m) = &self.merger else { return Ok(()) };
        let slot = self.info.slot.context("only an A/B phone merges an update")?;
        // Whether the update merged. A merge that stopped is no reason to
        // restart: the phone would come straight back here with the same
        // update outstanding, and keep coming back (2026-11-20).
        let mut merged = self.merge_done;
        // The page is told the merge has ended only after root has gone back
        // in: the page that says so offers the restart, and a restart tapped
        // while the image is being written would leave the phone without
        // either (2026-11-20).
        let mut tell_merged = false;
        for ev in m.events() {
            match ev {
                merge::Event::Looked(look) => {
                    if matches!(look, Merge::Done | Merge::NeedsReboot) {
                        merged = true;
                    }
                    eprintln!("merge {look:?}");
                    match look {
                        Merge::Done | Merge::NeedsReboot => tell_merged = true,
                        Merge::Going(done) => self.app.set_update(Standing::Merging(done)),
                        Merge::Failed(code) => self.app.set_update(Standing::Failed(code)),
                        Merge::Cancelled | Merge::Idle(_) => self.app.set_update(skyward_vab::standing(slot)?),
                    }
                }
                merge::Event::Failed(why) => {
                    skyward_sys::kmsg(&format!("the update did not merge: {why}"))?;
                    self.app.report(&skyward_ui::fmt::merge_failed(self.app.state.lang), now);
                    self.app.set_update(skyward_vab::standing(slot)?);
                }
            }
        }
        // Merged or not, the phone's readings say where the update
        // stands now.
        self.merge_done = merged;
        if m.ended() {
            self.merger = None;
            self.read_at = Instant::now();
            read_details(self.cfg, &mut self.info, &mut self.unread.details)?;
            // Root goes back now that nothing else writes those partitions:
            // what the merge writes lands in the same ones, and an image
            // carried in before it would be written over (2026-11-20). The
            // page follows that writing from here, and only when it has
            // ended does it offer the restart (2026-11-21).
            match self.root_slot.take() {
                Some(slot) => {
                    let wipe = match self.app.state.install {
                        skyward_ui::app::Install::Merging { wipe, .. } => wipe,
                        _ => false,
                    };
                    self.start_root(slot, wipe);
                }
                None => {
                    self.app.refresh();
                    // The page is told the update has merged whether the
                    // look that said so came with this turn or an earlier
                    // one: the event and the end of the thread are read
                    // apart, and a page left waiting on a merge that has
                    // finished sits at 95% for ever (2026-11-21).
                    if tell_merged || merged {
                        self.app.set_update(Standing::Merged);
                    }
                    // The phone stays where it is once the update has
                    // merged: a restart is the user's to ask for, on the
                    // page that says the update is done (2026-11-20).
                    if self.merge_done {
                        skyward_sys::kmsg("the update has merged; the phone waits for a restart to be asked for")?;
                    }
                }
            }
        }
        Ok(())
    }

    /// What the work on the two systems told.
    fn systems_told(&mut self, now: u64) -> Result<()> {
        let Some(worker) = &mut self.systems_worker else { return Ok(()) };
        for ev in worker.events() {
            let lang = self.app.state.lang;
            match ev {
                systems::Event::Resumed(to) => {
                    skyward_sys::kmsg(&format!("a switch cut off before is finished, and the {to:?} system starts next"))?;
                    self.app.report(skyward_ui::fmt::switch_resumed(lang), now);
                }
                systems::Event::NotResumed(why) => {
                    skyward_sys::kmsg(&format!("a switch cut off before is not finished: {why}"))?;
                    self.app.report(&skyward_ui::fmt::switch_unfinished(lang, &why), now);
                }
                systems::Event::Switched(to) => {
                    skyward_sys::kmsg(&format!("the {to:?} system starts next"))?;
                    // A switch the system asked for is done with.
                    if self.switch_asked {
                        self.done_with_request()?;
                    }
                    self.switch_asked = false;
                    self.switched = true;
                    power(Target::System)?;
                }
                systems::Event::Failed(why) => {
                    skyward_sys::kmsg(&format!("work on the systems failed: {why}"))?;
                    if self.switch_asked {
                        self.done_with_request()?;
                    }
                    self.switch_asked = false;
                    self.app.systems_work_failed(&why, now);
                }
                // The page stays as it is while the phone restarts.
                systems::Event::Read(read) => {
                    if !self.switched {
                        self.app.set_systems(read);
                    }
                }
            }
        }
        Ok(())
    }

    /// What the WLAN told; its page scans again now and then while it
    /// shows, as the stock settings do.
    fn wlan_told(&mut self) -> Result<()> {
        if let Some((told, traffic)) = self.wlan.as_ref().map(wlan::Worker::told) {
            if let Some(w) = told {
                self.app.set_wlan(w);
            }
            if let Some(t) = traffic {
                self.app.set_traffic(t);
            }
        }
        // The account's picture and the name the phone gives itself, read once
        // the partition they are kept on is open: both are there and nowhere
        // else, so the home page shows a row of them only once it is.
        if !self.account_read && matches!(self.app.state.data, Data::Open) {
            // `avatar` is already pixels, the theme having taken the density in.
            let px = self.app.theme().m.avatar.round() as u32;
            self.account_read = true;
            // The account's picture is kept by the account service, one file
            // named for the account it belongs to -- `xiaomi_user_avatar_` and
            // the account's number. The user's photo under `users/0` is the
            // system's own stand-in, a grey figure, which is what showed.
            match account_picture(px) {
                Some(picture) => self.app.set_avatar(picture),
                None => {
                    let _ = skyward_sys::kmsg("the account's picture is not read");
                }
            }
            // The name the owner gave the phone, and where the settings keep
            // none, the model the phone calls itself by.
            match avatar::device_name(Path::new("/data/system/users/0/settings_secure.xml")) {
                Some(name) => self.app.set_device_name(name),
                None => match skyward_sys::prop("ro.product.marketname") {
                    Some(model) if !model.trim().is_empty() => {
                        let _ = skyward_sys::kmsg("the name the phone gives itself is not read; its model is shown instead");
                        self.app.set_device_name(model.trim().to_string());
                    }
                    _ => {
                        let _ = skyward_sys::kmsg("the name the phone gives itself is not read");
                    }
                },
            }
        }
        // The thread the bringing-up started, taken as soon as it is there: the
        // pages turn while it is still being made, and the first turn that finds
        // it goes on with it.
        if self.wlan_pending && let Some(worker) = crate::take_wlan() {
            self.wlan_pending = false;
            self.wlan = Some(worker);
            self.wlan_opened = true;
        }
        // No thread ever came, and the phone's parts are up: one is started
        // here, once, so that two never share the interface.
        if self.wlan.is_none() && self.wlan_pending && crate::prepared() {
            self.wlan_pending = false;
            if let Some(w) = &self.cfg.wlan {
                let modules = std::mem::take(&mut self.wlan_modules);
                let libraries = self.wlan_libraries.take();
                self.wlan = Some(wlan::Worker::start(w.clone(), modules, libraries, self.servers.clone())?);
                self.app.state.wlan = Some(Wlan::default());
            }
        }
        // The WLAN kept is opened once the phone's own parts are up -- whether
        // or not a network is kept with it: the switch is about the radio, and
        // a phone whose network was forgotten still opens it. The network kept
        // is joined as soon as it shows in a scan.
        if self.app.state.wlan_auto && !self.wlan_opened {
            let power = self.app.state.wlan.as_ref().map(|w| w.power.clone());
            // Everything up first: asking earlier only moved the wait, it did
            // not shorten it -- the driver takes eleven seconds to make its
            // interface whether it is asked at 3 s or at 5 s (2026-11-15).
            if crate::prepared() && matches!(power, Some(WlanPower::Off | WlanPower::Failed(_))) {
                self.wlan_opened = true;
                wlan_worker(&self.wlan)?.send(wlan::Command::On);
                skyward_sys::kmsg(if self.wlan_saved.is_some() {
                    "the WLAN is opened for the network kept"
                } else {
                    "the WLAN is opened as the recovery starts, with no network kept"
                })?;
            }
        }
        if self.app.state.wlan_auto && !self.wlan_joined {
            if let Some(saved) = self.wlan_saved.clone() {
                let power = self.app.state.wlan.as_ref().map(|w| w.power.clone());
                // The radio is up: the network kept is joined at once, from
                // what was kept of it -- its name, its password and the kind
                // of protection it has. Waiting for a scan to list it first
                // costs seconds, and on a network of many channels a scan
                // can take ten (2026-11-20).
                if matches!(power, Some(WlanPower::On)) {
                    self.wlan_joined = true;
                    self.wlan_password = Some(saved.password.clone());
                    let name = String::from_utf8_lossy(&saved.ssid).into_owned();
                    skyward_sys::kmsg(&format!("joining the network kept: {name}"))?;
                    wlan_worker(&self.wlan)?.send(wlan::Command::Join {
                        ssid: saved.ssid,
                        name,
                        security: saved.security,
                        password: saved.password,
                    });
                }
            }
        }
        // A WLAN that did not come up is tried again a few times: the driver
        // sometimes needs longer than the first go gives it.
        if self.app.state.wlan_auto && !self.wlan_joined && self.wlan_retries < 3 {
            let failed = self.app.state.wlan.as_ref().is_some_and(|w| matches!(w.power, WlanPower::Failed(_)));
            if failed && self.wlan_failed_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(1)) {
                self.wlan_retries += 1;
                self.wlan_failed_at = Some(Instant::now());
                wlan_worker(&self.wlan)?.send(wlan::Command::On);
                skyward_sys::kmsg("the WLAN is opened again after it did not come up")?;
            }
        }
        // A network joined with a password the recovery has is kept for the
        // starts that follow, or forgotten where the access point refused it.
        if self.app.state.wlan_keep && !self.wlan_kept {
            if let Some(joined) = self.app.state.wlan.as_ref().and_then(|w| w.joined.as_ref()) {
                match &joined.stage {
                    Joining::Connected(_) => {
                        if let Some(password) = &self.wlan_password {
                            match crate::wlanmem::save(&joined.ssid, password, joined.security) {
                                Ok(()) => {
                                    self.wlan_kept = true;
                                    self.wlan_saved = Some(crate::wlanmem::Saved {
                                        ssid: joined.ssid.clone(),
                                        password: password.clone(),
                                        security: joined.security,
                                    });
                                    skyward_sys::kmsg(&format!("the network {} is kept for the starts that follow", joined.name))?;
                                }
                                Err(e) => skyward_sys::kmsg(&format!("the network was not kept: {e:#}"))?,
                            }
                        }
                    }
                    Joining::WrongPassword => {
                        self.wlan_kept = true;
                        self.wlan_saved = None;
                        self.wlan_password = None;
                        let _ = crate::wlanmem::forget_network();
                        skyward_sys::kmsg("the password kept was refused; the WLAN stays on and the network is forgotten")?;
                    }
                    _ => {}
                }
            }
        }
        // The avb page reads the checks when it shows, and lets them go when
        // it is left, so that the next visit reads them again.
        if self.app.screen() == Screen::Avb {
            if !self.avb_read {
                self.avb_read = true;
                let _ = self.effect(Effect::AvbRead, monotonic_ms())?;
                let _ = self.effect(Effect::VbmetaList, monotonic_ms())?;
            }
        } else if self.avb_read {
            self.avb_read = false;
            self.app.state.avb = None;
            self.app.state.vbmeta_kept = None;
            self.app.state.vbmeta_choice = None;
            self.app.state.vbmeta_note = None;
        }
        // 数据分区一打开就把应用列表预取到内存里：等用户点进「应用管理」时，
        // 列表已经在了，进页面不用读目录（2026-11-18）。
        if self.prefetch_due.is_none()
            && !matches!(self.app.state.data, skyward_ui::app::Data::Closed | skyward_ui::app::Data::Opening)
        {
            // data 一打开就先记下时间，等 800ms 再预取：刚解完密的头几帧不跟它抢。
            self.prefetch_due = Some(monotonic_ms() + 800);
        }
        if let Some(due) = self.prefetch_due
            && !self.apps_prefetched
            && monotonic_ms() >= due
        {
            self.apps_prefetched = true;
            let _ = self.effect(skyward_ui::app::Effect::ReadDataApps, monotonic_ms());
        }
        // 点在翻页带上却没命中：这一下是被吞掉的。
        if self.app.state.page_miss != self.last_page_miss {
            self.last_page_miss = self.app.state.page_miss;
            if let Some((x, y)) = self.last_page_miss {
                let _ = skyward_sys::kmsg(&format!("page miss at ({x:.0}, {y:.0})"));
            }
        }
        // 翻页键上的点击落在哪儿。
        if self.app.state.page_tap != self.last_page_tap {
            self.last_page_tap = self.app.state.page_tap;
            if let Some((x, y, by)) = self.last_page_tap {
                let _ = skyward_sys::kmsg(&format!("page tap at ({x:.0}, {y:.0}) by {by}"));
            }
        }
        // 有没有意外的“重新到达”：它会把页码恢复原位，看起来就像点了没反应。
        if self.app.state.app_arrivals != self.last_app_arrivals {
            self.last_app_arrivals = self.app.state.app_arrivals;
            let _ = skyward_sys::kmsg(&format!("arrived at data apps #{}", self.last_app_arrivals));
        }
        // 翻页点击有没有进来：计数变了就记一行。
        if self.app.state.app_page_taps != self.last_app_taps {
            self.last_app_taps = self.app.state.app_page_taps;
            let _ = skyward_sys::kmsg(&format!("app page tap #{} -> page {}", self.last_app_taps, self.app.state.app_page + 1));
        }
        // 应用页码变了就记一行：点「下一页」到底有没有落到处理里。
        if self.app.state.app_page != self.last_app_page {
            self.last_app_page = self.app.state.app_page;
            let _ = skyward_sys::kmsg(&format!("app page -> {}", self.last_app_page + 1));
        }
        // 名字与图标一条条填进列表：每轮收几条，界面跟着动。
        if let Some(rx) = &self.apps_feed {
            let mut done = 0;
            let mut ended = false;
            loop {
                match rx.try_recv() {
                    Ok(Feed::System(list)) => {
                        self.app.set_sys_apps(list);
                        done += 1;
                    }
                    Ok(Feed::Apps(apps)) => {
                        self.app.set_apps(Ok(apps), Vec::new());
                        done += 1;
                    }
                    Ok(Feed::Failed(why)) => {
                        self.app.set_apps(Err(why), Vec::new());
                        done += 1;
                    }
                    Ok(Feed::Name(i, label, icon)) => {
                        self.app.data_app_enriched(i, label, icon);
                        done += 1;
                        if done >= 32 {
                            break;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        ended = true;
                        break;
                    }
                }
            }
            if done > 0 {
                // 一批收完画一次就够。
                self.app.refresh();
            }
            if ended {
                self.apps_feed = None;
            }
        }
        // How the download goes: how many bytes have come, and how it ended.
        if let Some(worker) = &self.fetcher {
            let (done, total) = worker.progress();
            if matches!(self.app.state.fetch, skyward_ui::app::Fetch::Running { .. }) {
                self.app.state.fetch = skyward_ui::app::Fetch::Running { done, total };
                self.app.refresh();
            }
            if let Some(end) = worker.ended() {
                let ended = match end {
                    fetch::Ended::Done { path, bytes } => skyward_ui::app::Fetch::Done { path: path.display().to_string(), bytes },
                    fetch::Ended::Failed(why) => skyward_ui::app::Fetch::Failed(why),
                    fetch::Ended::Stopped => skyward_ui::app::Fetch::Stopped,
                };
                skyward_sys::kmsg(&format!("the download ended: {ended:?}"))?;
                self.app.state.fetch = ended;
                self.app.refresh();
                self.fetcher = None;
            }
        }
        // The page another device types an address on is served while the
        // download page shows and the phone is on a network.
        let joined = self
            .app
            .state
            .wlan
            .as_ref()
            .and_then(|w| w.joined.as_ref())
            .and_then(|j| match &j.stage {
                Joining::Connected(link) => Some(link.address.to_string()),
                _ => None,
            });
        match (self.portal.is_some(), self.app.screen(), joined) {
            (false, Screen::Fetch, Some(ip)) => match crate::portal::Portal::start(&ip) {
                Ok(portal) => {
                    skyward_sys::kmsg(&format!("the page for a link is at {}", portal.url()))?;
                    self.app.state.fetch_portal = Some(portal.url().to_string());
                    self.portal = Some(portal);
                }
                Err(e) => skyward_sys::kmsg(&format!("the page for a link is not served: {e:#}"))?,
            },
            (true, screen, joined) if screen != Screen::Fetch || joined.is_none() => {
                self.portal = None;
                self.app.state.fetch_portal = None;
            }
            _ => {}
        }
        if let Some(portal) = &self.portal
            && let Some(url) = portal.take()
        {
            skyward_sys::kmsg("the link was taken from the page another device opened")?;
            self.app.state.fetch_url = url;
            self.app.refresh();
        }
        // How many requests the served folder has answered, which the page
        // shows to say it is alive.
        let served_now = self.serving.as_ref().map(|s| s.served());
        if let (Some(count), skyward_ui::app::Serving::Running { served, .. }) = (served_now, &mut self.app.state.serving)
            && *served != count
        {
            *served = count;
            self.app.refresh();
        }
        let wlan_on = self.app.state.wlan.as_ref().is_some_and(|w| w.power == WlanPower::On);
        if self.app.screen() == Screen::Wlan && wlan_on && self.scanned_at.elapsed() >= SCAN_EVERY {
            self.scanned_at = Instant::now();
            wlan_worker(&self.wlan)?.send(wlan::Command::Scan);
        }
        // The scan the driver answers with right after it comes up often holds
        // nothing; the network kept is looked for more often than the page
        // looks for its own, so that joining it does not wait for the
        // supplicant's own slow round.
        if self.app.state.wlan_auto && !self.wlan_joined && wlan_on && self.scanned_at.elapsed() >= LOOKING_FOR {
            self.scanned_at = Instant::now();
            wlan_worker(&self.wlan)?.send(wlan::Command::Scan);
        }
        Ok(())
    }

    /// Settings changed go into their file a moment after the last change,
    /// while nothing else has /metadata: not in fastboot, whose fastbootd
    /// may, nor while an install, a merge, erasing, a backup or work on
    /// whole partitions is under way.
    fn keep_settings(&mut self, now: u64) -> Result<()> {
        let metadata_busy = self.fastboot
            || self.installer.is_some()
            || self.merger.is_some()
            || self.eraser.is_some()
            || self.backuper.is_some()
            || self.part_worker.is_some()
            || self.systems_worker.as_ref().is_some_and(systems::Worker::busy);
        if !metadata_busy
            && let Some(due) = self.keeper.due(self.app.settings(), now)
        {
            match settings::write(&due) {
                Ok(()) => {
                    skyward_sys::kmsg("the settings are kept")?;
                    self.keeper.written(due);
                }
                Err(e) => {
                    skyward_sys::kmsg(&format!("the settings were not kept: {e:#}"))?;
                    self.keeper.failed();
                }
            }
        }
        Ok(())
    }

    /// What asking the update server, and downloading the system's full
    /// package, told.
    fn updates_told(&mut self, now: u64) -> Result<()> {
        let told = self.checker.as_ref().map(updates::Worker::events).unwrap_or_default();
        let mut checked = false;
        for ev in told {
            let updates::Event::Checked(answer) = ev else { unreachable!("asking the server downloads nothing") };
            let have = match &answer {
                Ok(Some(release)) => have(&self.app, release),
                _ => Have::Nothing,
            };
            self.app.update_checked(answer, have);
            checked = true;
        }
        if checked {
            self.checker = None;
        }
        let told = self.downloader.as_ref().map(updates::Worker::events).unwrap_or_default();
        let mut fetched = false;
        for ev in told {
            match ev {
                updates::Event::Progress(progress, rate) => self.app.download_progress(progress, rate),
                updates::Event::Ended(end) => {
                    let release = self.app.state.newest().context("a download is of the newest system")?.clone();
                    let end = match end {
                        updates::End::Downloaded => DownloadEnd::Downloaded,
                        updates::End::Stopped => DownloadEnd::Stopped,
                        updates::End::Failed(why) => DownloadEnd::Failed(why),
                        updates::End::Updated(_) | updates::End::NotWritten(_) | updates::End::WriteFailed(..) => {
                            unreachable!("a system's full package is only downloaded")
                        }
                    };
                    skyward_sys::kmsg(&format!("downloading {} ended: {end:?}", release.name))?;
                    let have = have(&self.app, &release);
                    self.app.download_ended(end, have, now);
                    fetched = true;
                }
                updates::Event::Checked(_) | updates::Event::SkywardChecked(_) | updates::Event::Writing { .. } => {
                    unreachable!("a download asks nothing of its own, and writes nothing")
                }
            }
        }
        if fetched {
            self.downloader = None;
        }
        Ok(())
    }

    /// What asking GitHub for Skyward's newest release, and updating to it,
    /// told.
    fn skyward_told(&mut self, now: u64) -> Result<()> {
        let told = self.skyward_checker.as_ref().map(updates::Worker::events).unwrap_or_default();
        let mut asked = false;
        for ev in told {
            let updates::Event::SkywardChecked(answer) = ev else { unreachable!("asking GitHub downloads nothing") };
            skyward_sys::kmsg(&format!("GitHub told of Skyward's newest release: {answer:?}"))?;
            self.app.skyward_checked(answer);
            asked = true;
        }
        if asked {
            self.skyward_checker = None;
        }
        let told = self.skyward_updater.as_ref().map(updates::Worker::events).unwrap_or_default();
        let mut updated = false;
        for ev in told {
            match ev {
                updates::Event::Progress(progress, rate) => self.app.skyward_progress(SkywardStage::Download(progress), rate),
                updates::Event::Writing { done, total } => self.app.skyward_progress(SkywardStage::Write { done, total }, 0),
                updates::Event::Ended(end) => {
                    let end = match end {
                        updates::End::Updated(slots) => SkywardEnd::Updated(slots),
                        updates::End::Stopped => SkywardEnd::Stopped,
                        updates::End::Failed(why) => SkywardEnd::Download(why),
                        updates::End::NotWritten(why) => SkywardEnd::NotWritten(why),
                        updates::End::WriteFailed(slots, why) => SkywardEnd::WriteFailed(slots, why),
                        updates::End::Downloaded => unreachable!("Skyward's build is written once it is downloaded"),
                    };
                    skyward_sys::kmsg(&format!("updating Skyward ended: {end:?}"))?;
                    self.app.skyward_ended(end, now);
                    updated = true;
                }
                updates::Event::Checked(_) | updates::Event::SkywardChecked(_) => unreachable!("an update asks nothing of its own"),
            }
        }
        if updated {
            self.skyward_updater = None;
        }
        Ok(())
    }

    /// A job the page is waiting on that has gone quiet, or a page waiting on
    /// no job at all. One made with the screen lock kept from before is let go
    /// of after [`REMEMBER_WAIT`]: the page asks for the lock instead of
    /// standing at a job whose answer the secure world may hold for a minute,
    /// and a lock that does come back later opens the partition just the same.
    /// A job that only takes long says where it stands in the kernel log, so a
    /// page that does not move can be told from a phone that is not working.
    fn decrypt_stalled(&mut self, now: u64) -> Result<()> {
        if self.decryptor.is_none() && self.pending_open.is_none() {
            self.opening_said = None;
            if !matches!(self.app.state.data, Data::Closed | Data::Opening)
                || self.app.data_finishing()
                || self.app.screen() != Screen::Decrypt
            {
                self.idle_open_at = None;
                return Ok(());
            }
            // Asked for and never started -- the request failed where it was
            // made, and the page was left saying it is at the partition.
            let since = *self.idle_open_at.get_or_insert(Instant::now());
            if since.elapsed() >= Duration::from_secs(3) {
                self.idle_open_at = None;
                skyward_sys::kmsg("the page is at the partition with nothing opening it; asking again")?;
                self.app.data_gave_up(self.app.state.lang.pick(OPEN_LOST).to_string(), now);
            }
            return Ok(());
        }
        self.idle_open_at = None;
        if self.attempt_was_remembered && self.decrypt_at.elapsed() >= REMEMBER_WAIT {
            skyward_sys::kmsg("the screen lock kept from before has not come back; the page asks for it instead")?;
            if let Some(job) = self.decryptor.take() {
                job.detach();
            }
            self.attempt_was_remembered = false;
            self.tried_remembered = true;
            self.last_credential = None;
            self.opening_said = None;
            crate::remember::done_trying();
            if self.app.screen() == Screen::Decrypt {
                self.app.data_gave_up(self.app.state.lang.pick(REMEMBER_SLOW).to_string(), now);
            }
            return Ok(());
        }
        if self.opening_said.is_none_or(|said| said.elapsed() >= OPENING_SAID_EVERY) {
            self.opening_said = Some(Instant::now());
            skyward_sys::kmsg(&format!(
                "opening the data partition for {}s, at {} thousandths",
                self.decrypt_at.elapsed().as_secs(),
                skyward_crypto::data::progress()
            ))?;
        }
        Ok(())
    }

    /// How opening the data partition, or a user's own files in it, ended.
    fn decrypt_told(&mut self, now: u64) -> Result<()> {
        let Some(ended) = self.decryptor.as_ref().and_then(Decryptor::ended) else { return Ok(()) };
        self.decryptor = None;
        match ended {
            decrypt::Ended::Opened(Ok(opening)) => {
                if let Some(mounted) = &opening.mounted {
                    log_mounted(mounted)?;
                }
                self.closed = opening.closed;
                show_closed(&mut self.app, &self.closed)?;
                // What was typed opened the partition itself: keep it too, so
                // that the next start needs nothing typed. A password that
                // opens them here is as good as one kept from before.
                if self.app.state.remember
                    && !self.attempt_was_remembered
                    && let Some(credential) = self.last_credential.clone()
                    && self.closed.is_empty()
                {
                    match crate::remember::write(&credential) {
                        Ok(()) => {
                            self.remembered = Some(credential);
                            skyward_sys::kmsg("the screen lock is kept for the starts that follow")?;
                        }
                        Err(e) => skyward_sys::kmsg(&format!("the screen lock was not kept: {e:#}"))?,
                    }
                }
                // The screen lock kept from before opens the user's own files
                // without anyone typing it. A wrong one is not tried again: the
                // secure world counts it against the user.
                if !self.tried_remembered && !self.closed.is_empty() {
                    self.tried_remembered = true;
                    if let (Some(credential), Some(first)) = (self.remembered.clone(), self.closed.first()) {
                        let user = first.user;
                        skyward_sys::kmsg(&format!("opening user {user}'s files with the screen lock kept from before"))?;
                        self.last_credential = Some(credential.clone());
                        self.attempt_was_remembered = true;
                        let job = decrypt::Job::Unlock { closed: self.closed.clone(), user, credential };
                        // The mark says a start that ends here is not to try
                        // this lock again: see remember::take_trying.
                        if let Err(e) = crate::remember::mark_trying() {
                            skyward_sys::kmsg(&format!("the mark that the kept lock is being tried was not left: {e:#}"))?;
                        }
                        self.start_decrypt(job)?;
                        // The owner's files are being opened behind the lock
                        // kept from before: the page stays at the partition,
                        // with the bar running, rather than asking for a lock
                        // it is already using.
                        self.app.data_still_opening();
                    }
                }
            }
            decrypt::Ended::Opened(Err(why)) => {
                skyward_sys::kmsg(&format!("the data partition stays closed: {why}"))?;
                // The update the system asked for cannot be read, and
                // is done with.
                if self.pending.take().is_some() {
                    let failed = skyward_install::Outcome::Failed(skyward_install::Failure::Unreadable(why.clone()));
                    self.app.install(skyward_install::Event::Finished(failed), now);
                    self.done_with_request()?;
                }
                self.app.data_failed(why);
            }
            decrypt::Ended::Unlocked(Ok(still)) => {
                self.closed = still;
                show_closed(&mut self.app, &self.closed)?;
                // The screen lock that opened them is kept for the starts that
                // follow, so that the next start needs nothing typed.
                if self.app.state.remember
                    && !self.attempt_was_remembered
                    && let Some(credential) = self.last_credential.clone()
                {
                    match crate::remember::write(&credential) {
                        Ok(()) => {
                            self.remembered = Some(credential);
                            skyward_sys::kmsg("the screen lock is kept for the starts that follow")?;
                        }
                        Err(e) => skyward_sys::kmsg(&format!("the screen lock was not kept: {e:#}"))?,
                    }
                }
                self.attempt_was_remembered = false;
                self.last_credential = None;
                if self.tried_remembered {
                    crate::remember::done_trying();
                }
                // Everything the recovery can open is open: the decrypt page
                // has done its work and the menu is what to show.
                if self.closed.is_empty() {
                    skyward_sys::kmsg("the decrypt page is done; the menu shows")?;
                    self.app.decrypted();
                }
            }
            decrypt::Ended::Unlocked(Err(why)) => {
                skyward_sys::kmsg(&format!("what was entered did not open the user's files: {why}"))?;
                if self.attempt_was_remembered {
                    self.remembered = None;
                    let _ = crate::remember::forget();
                    skyward_sys::kmsg("the screen lock kept from before did not open the user's files, so it is forgotten")?;
                }
                self.attempt_was_remembered = false;
                self.last_credential = None;
                crate::remember::done_trying();
                self.app.unlock_failed(why, now);
            }
        }
        // Screenshots taken while internal storage was closed go into
        // the gallery's once it opens.
        if self.app.state.place_open(Place::Storage)
            && let Err(e) = keep_held_screenshots()
        {
            let why = format!("{e:#}");
            skyward_sys::kmsg(&format!("the screenshots held did not go into the gallery's: {why}"))?;
            self.app.screenshots_not_kept(&why, now);
        }
        // The update the system asked for starts once what holds it is
        // open; until the user whose own files hold it unlocks them,
        // its page says to.
        if let Some(wanted) = self.pending.take() {
            match wanted.needs {
                Needs::User(user) if self.closed.iter().any(|c| c.user == user) => {
                    self.app.asked_locked(user, now);
                    self.pending = Some(wanted);
                }
                _ => self.installer = Some(install_asked(&self.app, wanted)?),
            }
        }
        Ok(())
    }

    /// What the terminal's shell wrote, and its end.
    fn shell_told(&mut self) -> Result<()> {
        let Some(sh) = &mut self.shell else { return Ok(()) };
        let wrote = sh.read()?;
        if !wrote.is_empty() {
            self.app.term_output(&wrote);
        }
        if let Some(status) = sh.ended()? {
            // What it wrote last shows before its end does.
            let last = sh.read()?;
            if !last.is_empty() {
                self.app.term_output(&last);
            }
            self.app.term_ended(pty::how(status));
            self.shell = None;
        }
        Ok(())
    }

    /// Internal storage goes to the computer while it is open, and no
    /// erasing or installing could take it away. File transfer that does
    /// not follow, or whose service ended, stops, and the page says why.
    fn offer_storage(&mut self, now: u64) -> Result<()> {
        let kept = match &mut self.transfer {
            Some(t) if !t.service.running() => Err(anyhow!("the file transfer service ended")),
            Some(t) => {
                let offer = self.app.state.data.storage_open() && self.eraser.is_none() && self.installer.is_none();
                t.offer(offer, self.app.state.lang).and_then(|()| {
                    if t.zone != self.app.state.time_zone {
                        t.service.set_zone(skyward_ui::fmt::rules(self.app.state.time_zone).clone())?;
                        t.zone = self.app.state.time_zone;
                    }
                    Ok(())
                })
            }
            None => Ok(()),
        };
        if let Err(e) = kept {
            // An ended service's own error says more than its ending does.
            let why = match self.transfer.take().expect("only file transfer that runs fails").stop(!self.fastboot && !self.sideloading) {
                Err(cause) => cause,
                Ok(()) => e,
            };
            skyward_sys::kmsg(&format!("file transfer stops: {why:#}"))?;
            self.app.file_transfer_failed(false, &skyward_ui::fmt::transfer_stopped(self.app.state.lang, &format!("{why:#}")), now);
        }
        Ok(())
    }
}

/// What comes in to the pages, and what they ask for in answer.
impl Pages<'_> {
    /// Hand the pages what came in since the last turn: the first page
    /// showing, a USB drive plugged in or pulled out, what programs asked on
    /// the recovery socket, each answered as the recovery stands when its
    /// turn comes, and touches and keys. What the pages ask for is done in
    /// the order they ask.
    fn take_in(&mut self, drive_told: Option<Option<UsbDrive>>, now: u64) -> Result<()> {
        let requests = match &self.control {
            Some(c) => c.requests()?,
            None => Vec::new(),
        };
        let incoming = self
            .arriving
            .take()
            .into_iter()
            .chain(drive_told.map(Incoming::Drive))
            .chain(requests.into_iter().map(Incoming::Asked))
            .chain(self.inputs.read()?.into_iter().map(Incoming::Input));
        for came in incoming {
            let effects = match came {
                Incoming::Arrived => self.app.arrive(),
                Incoming::Drive(drive) => self.app.set_usb_drive(drive),
                Incoming::Asked(asked) => {
                    skyward_sys::kmsg(&format!("asked on the recovery socket: {asked:?}"))?;
                    answer(asked, self.fastboot)
                }
                Incoming::Input(Event::Touch(t, at)) => self.app.touch(t, &self.fonts, at),
                Incoming::Input(Event::Key(k, true)) => self.app.key(k, now),
                Incoming::Input(Event::Key(k, false)) => self.app.key_up(k, now),
            };
            // What the pages ask for in answer to the work done goes on
            // after it.
            let mut queue = VecDeque::from(effects);
            while let Some(fx) = queue.pop_front() {
                eprintln!("effect {fx:?}");
                // What a page asks for can fail for reasons that are the
                // user's to see -- a file gone, an installer already running,
                // storage held by file transfer -- and none of them is a
                // reason to end the recovery and start it again, which is what
                // an error let out here did (2026-11-15, picking a flashable
                // zip with the port busy restarted the recovery).
                match self.effect(fx.clone(), now) {
                    Ok(more) => queue.extend(more),
                    Err(e) => {
                        let said = format!("{e:#}");
                        skyward_sys::kmsg(&format!("the page asked for {fx:?}, which failed: {said}"))?;
                        self.app.report(&said, now);
                        self.app.refresh();
                    }
                }
            }
        }
        Ok(())
    }

    /// Do what a page asks for, and give what the pages ask for in answer
    /// to it.
    fn effect(&mut self, fx: Effect, now: u64) -> Result<Vec<Effect>> {
        let cfg = self.cfg;
        match fx {
            // Leaving is the user's answer to whatever the request
            // still kept in misc asked.
            Effect::Reboot(target) => {
                // A restart part way through the merge leaves the other slot
                // with snapshots it cannot start from, and the phone comes
                // back to a recovery rather than starting (2026-11-20).
                if matches!(target, Target::System) && self.merger.is_some() {
                    let lang = self.app.state.lang;
                    self.app.report(&skyward_ui::fmt::merge_wait(lang), now);
                    return Ok(Vec::new());
                }
                self.done_with_request()?;
                power(target)?
            }
            Effect::Dismiss => self.done_with_request()?,
            Effect::FactoryReset => self.reset_asked(now)?,
            Effect::Wipe(chosen) => self.wipe(chosen, now)?,
            Effect::Install => self.installer.as_ref().context("no installer holds an update")?.install(),
            Effect::StartShell(folder) => self.start_shell(&folder, now)?,
            // A shell that will not take keys is the page's to report; the
            // recovery carries on.
            Effect::TermInput(bytes) => {
                let sh = self.shell.as_mut().context("keys went to a shell that has ended")?;
                if let Err(e) = sh.write(&bytes) {
                    self.app.report(&format!("{e:#}"), now);
                }
            }
            Effect::ShellTo(folder) => self.shell_to(&folder, now)?,
            Effect::Haptic => self.click()?,
            Effect::Ticks => self.ticks()?,
            // The panel shows the last frame drawn, which is what is
            // saved; one that cannot be is the page's to report.
            Effect::RecordStart => match self.start_recording() {
                Ok((rec, name, spec)) => {
                    self.recorder = Some(rec);
                    self.app.record_started(name, spec.clone(), now);
                    skyward_sys::kmsg(&format!("record: started, {spec}"))?;
                }
                Err(e) => self.app.record_failed(&format!("{e:#}"), now),
            },
            Effect::RecordStop => {
                if let Some(rec) = self.recorder.take() {
                    match rec.finish() {
                        Ok(done) => {
                            let name = done.name();
                            let secs = (done.millis + 500) / 1000;
                            skyward_sys::kmsg(&format!(
                                "record: stopped after {} frames, {} bytes in {}",
                                done.frames(),
                                done.bytes(),
                                done.path().display()
                            ))?;
                            self.app.record_stopped(&name, secs, now);
                        }
                        Err(e) => self.app.record_failed(&format!("{e:#}"), now),
                    }
                }
            }
            // 胶囊上的红点：停一下，或者接着录。写线程自己记这两下，时间轴
            // 因此从停下的地方接上去（2026-11-21）。
            Effect::RecordPause(paused) => {
                if let Some(rec) = &self.recorder {
                    rec.set_paused(paused);
                }
                skyward_sys::kmsg(match paused {
                    true => "record: paused",
                    false => "record: carried on",
                })?;
            }
            Effect::Screenshot { storage } => {
                let t0 = Instant::now();
                let saved = save_screenshot(&self.screen, storage, self.app.state.time_zone).map_err(|e| format!("{e:#}"));
                eprintln!("screenshot {saved:?} in {} ms", t0.elapsed().as_millis());
                self.app.screenshot_saved(saved, now);
            }
            Effect::Sideload => {
                if self.may_install(now)? {
                    self.installer = Some(Installer::start(detach(), install_options(&self.app))?);
                    self.sideloading = true;
                }
            }
            Effect::InstallFile(place, path) => {
                if self.may_install(now)? {
                    self.installer = Some(Installer::open(in_place(place, &path), detach(), install_options(&self.app))?);
                }
            }
            Effect::InstallPath(path) => {
                if self.may_install(now)? {
                    self.installer = Some(Installer::open(PathBuf::from(path), detach(), install_options(&self.app))?);
                }
            }
            Effect::Flash { partition, pick } => {
                let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                self.installer.as_ref().context("no installer holds an image")?.flash(partition, pick.slots(slot));
            }
            Effect::Cancel => self.cancel_install(now)?,
            Effect::ReadMounts => self.app.set_mounts(self.mounts()),
            Effect::ReadDataApps => {
                // 扫描目录、读 packages.xml、算体积都在后台做：上次的列表继续
                // 显示着，新的到了再换（2026-11-18）。
                let (tx, rx) = std::sync::mpsc::channel();
                self.apps_feed = Some(rx);
                std::thread::Builder::new().name("data apps".into()).spawn(move || {
                    let t0 = std::time::Instant::now();
                    let rough = dirs_only();
                    let _ = skyward_sys::kmsg(&format!("data apps: {} dirs in {} ms", rough.len(), t0.elapsed().as_millis()));
                    if !rough.is_empty() && tx.send(Feed::Apps(rough)).is_err() {
                        return;
                    }
                    let t1 = std::time::Instant::now();
                    match quick_data_apps() {
                        Ok(apps) => {
                            let _ = skyward_sys::kmsg(&format!("data apps: full list {} in {} ms", apps.len(), t1.elapsed().as_millis()));
                            let total = apps.len();
                            if tx.send(Feed::Apps(apps.clone())).is_err() {
                                return;
                            }
                            // 让主循环先把列表和最初几下点击收下，再开始解 apk。
                            std::thread::sleep(std::time::Duration::from_millis(250));
                            let mut done = 0;
                            for (i, app) in apps.iter().enumerate() {
                                let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| enrich_one(&app.dir)))
                                    .ok()
                                    .flatten();
                                let (label, icon) = match got {
                                    Some((label, icon)) => (label, icon),
                                    None => {
                                        if i < 5 {
                                            let _ = skyward_sys::kmsg(&format!("data apps: {} has no name to read", app.package));
                                        }
                                        (app.package.clone(), None)
                                    }
                                };
                                if tx.send(Feed::Name(i, label, icon)).is_err() {
                                    break;
                                }
                                done += 1;
                            }
                            let _ = skyward_sys::kmsg(&format!("data apps: {done} of {total} names read"));
                        }
                        Err(why) => {
                            let _ = tx.send(Feed::Failed(why));
                        }
                    }
                })?;
            }
            Effect::ReadSystemApps => {
                let (tx, rx) = std::sync::mpsc::channel();
                self.apps_feed = Some(rx);
                std::thread::Builder::new().name("system apps".into()).spawn(move || {
                    let t0 = std::time::Instant::now();
                    let list = system_apps();
                    let _ = skyward_sys::kmsg(&format!("system apps: {} in {} ms", list.len(), t0.elapsed().as_millis()));
                    let _ = tx.send(Feed::System(list));
                })?;
            }
            Effect::DataAppRemove(package, dir) => {
                match remove_data_app(&package, &dir) {
                    Ok(()) => {
                        skyward_sys::kmsg(&format!("the app {package} is off, from {dir}"))?;
                        self.app.report(&format!("{package} 已卸载"), monotonic_ms());
                    }
                    Err(e) => {
                        skyward_sys::kmsg(&format!("{package} is not removed: {e}"))?;
                        self.app.report(&format!("卸载失败：{e}"), monotonic_ms());
                    }
                }
                if let Ok(apps) = quick_data_apps() {
                    self.app.set_apps(Ok(apps), Vec::new());
                }
            }
            // 系统应用的安装包导到内部存储：分区是只读的，复制一份出去，
            // 比什么都做不了强。成与不成都在页面上说一句。
            Effect::ExportApk { from, name } => {
                let l = self.app.state.lang;
                match export_apk(&from, &name) {
                    Ok(path) => {
                        skyward_sys::kmsg(&format!("{name} exported to {}", path.display()))?;
                        self.app.report(&skyward_ui::fmt::exported(l, &path.display().to_string()), now);
                    }
                    Err(e) => {
                        skyward_sys::kmsg(&format!("{name} is not exported: {e:#}"))?;
                        self.app.report(&skyward_ui::fmt::export_failed(l, &format!("{e:#}")), now);
                    }
                }
            }
            Effect::ReadModules => {
                let rows = read_modules();
                self.app.set_modules(rows);
            }
            Effect::ModuleRemove(id, on) => {
                if let Err(e) = mark_module_remove(&id, on) {
                    skyward_sys::kmsg(&format!("module {id} is not marked: {e}"))?;
                }
                let rows = read_modules();
                self.app.set_modules(rows);
            }
            Effect::ModuleInstall(path) => {
                match install_module(&path) {
                    Ok(id) => {
                        skyward_sys::kmsg(&format!("the module {id} is in from {path}"))?;
                        self.app.report(&format!("{id}: installed; it loads at the next start"), monotonic_ms());
                    }
                    Err(e) => {
                        skyward_sys::kmsg(&format!("{path} is not installed as a module: {e}"))?;
                        self.app.report(&format!("not installed: {e}"), monotonic_ms());
                    }
                }
                let rows = read_modules();
                self.app.set_modules(rows);
            }
            Effect::ModuleSwitch(id, on) => {
                if let Err(e) = switch_module(&id, on) {
                    skyward_sys::kmsg(&format!("module {id} is not switched: {e}"))?;
                }
                let rows = read_modules();
                self.app.set_modules(rows);
            }
            Effect::ReadSlots => self.app.set_boot_slots(read_slots()),
            Effect::SetSlot(slot) => self.app.slot_switched(set_slot(slot), now),
            Effect::ReadSystems => self.systems()?.send(systems::Task::Read),
            // An install still under way may yet write the boot
            // partitions; a switch waits for it.
            Effect::SwitchSystem(to) => {
                if self.installer.is_some() {
                    self.app.systems_work_failed(self.app.state.lang.pick(INSTALL_UNFINISHED), now);
                } else {
                    skyward_sys::kmsg(&format!("switching to the {to:?} system, as the systems page asked"))?;
                    self.systems()?.send(systems::Task::Switch(to));
                }
            }
            Effect::RemoveSystem => {
                if self.installer.is_some() {
                    self.app.systems_work_failed(self.app.state.lang.pick(INSTALL_UNFINISHED), now);
                } else {
                    skyward_sys::kmsg("removing the second system, as the systems page asked")?;
                    self.systems()?.send(systems::Task::Remove);
                }
            }
            Effect::InstallSystem { size } => self.installer.as_ref().context("no installer holds a system")?.install_system(size),
            // The decrypt page shows meanwhile that the work is
            // under way, and takes nothing more until it is done.
            Effect::OpenData => {
                ensure!(self.decryptor.is_none() && self.pending_open.is_none(), "the data partition is being opened already");
                self.start_or_wait(decrypt::Job::Open)?;
            }
            Effect::Unlock { user, code } => {
                ensure!(self.decryptor.is_none(), "the data partition is being opened already");
                // Kept to write down once it opens what it was entered for.
                self.last_credential = Some(code.clone().into_bytes());
                self.attempt_was_remembered = false;
                let job = decrypt::Job::Unlock { closed: self.closed.clone(), user, credential: code.into_bytes() };
                self.start_or_wait(job)?;
            }
            Effect::ReadDir(folder) => list(&mut self.app, folder)?,
            Effect::ReadPlace(place, folder) => list_place(&mut self.app, place, folder)?,
            Effect::ReadOtg(folder) => list_otg(&mut self.app, folder)?,
            Effect::Paste { op, path, into, clash } => self.paste(op, path, into, clash, now)?,
            Effect::Delete(path) => {
                ensure!(self.worker.is_none(), "work on files is under way already");
                self.worker = Some(filework::Worker::start(filework::Task::Delete { paths: vec![PathBuf::from(&path)] })?);
                self.app.file_job_started(FileTask::Delete { path });
            }
            Effect::DeleteMany(paths) => {
                ensure!(self.worker.is_none(), "work on files is under way already");
                let first = paths.first().cloned().unwrap_or_default();
                let all: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
                self.worker = Some(filework::Worker::start(filework::Task::Delete { paths: all })?);
                self.app.file_job_started(FileTask::Delete { path: first });
            }
            Effect::PasteMany { op, paths, into, clash } => self.paste_many(op, paths, into, clash, now)?,
            Effect::StopJob => self.worker.as_ref().context("there is no work on files to stop")?.stop(),
            Effect::Relabel => self.relabel(now)?,
            Effect::StopRelabel => self.relabeler.as_ref().context("there is no relabeling to stop")?.stop(),
            // Partitions that cannot be read are the page's to say.
            Effect::ReadFilesystems => self.app.set_filesystems(read_filesystems().map_err(|e| format!("{e:#}"))),
            Effect::Filesystem { name, action } => self.filesystem(&name, action, now)?,
            Effect::Backup { parts, place, compress, verify } => self.back_up(parts, place, compress, verify, now)?,
            Effect::Restore { place, name, parts } => self.restore(place, &name, parts, now)?,
            Effect::FindBackups { storage, usb } => find_backups(&mut self.app, storage, usb, now),
            Effect::DeleteBackup { place, name } => {
                let deleted = delete_backup(place, &backups_in(place).join(&name)).map_err(|e| format!("{e:#}"));
                return Ok(self.app.backup_deleted(deleted, now));
            }
            Effect::StopBackup => self.backuper.as_ref().context("there is no backup or restore to stop")?.stop(),
            Effect::ReadUpdate => {
                let slot = self.info.slot.context("only an A/B phone has an update to merge")?;
                // A merge followed tells where it stands itself.
                if self.merger.is_none() {
                    self.app.set_update(skyward_vab::standing(slot)?);
                }
                // An update in the other slot starts with that slot,
                // which a system's updater may not have set yet.
                self.app.set_boot_slots(read_slots());
            }
            Effect::Merge => {
                ensure!(self.merger.is_none(), "the update is being merged already");
                let current = self.info.slot.context("only an A/B phone merges an update")?;
                let slot = self.merge_target(current).unwrap_or(current);
                skyward_sys::kmsg("merging the update, as the update page asked")?;
                self.merger = Some(merge::Merger::start(slot, true)?);
                self.merge_done = false;
            }
            // An update that would not merge is given up: the slot starts
            // from its own partitions again, without what the update
            // wrote, rather than the phone coming back here at every
            // restart (2026-11-20).
            Effect::GiveUp => {
                ensure!(self.merger.is_none(), "the update is being merged; it cannot be given up now");
                let current = self.info.slot.context("only an A/B phone merges an update")?;
                let slot = self.merge_target(current).unwrap_or(current);
                skyward_sys::kmsg(&format!("giving the update in the {slot:?} slot up, as the update page asked"))?;
                let lang = self.app.state.lang;
                match merge::cancel(slot) {
                    Ok(()) => {
                        self.app.set_update(Standing::None);
                        self.app.report(&skyward_ui::fmt::update_given_up(lang), now);
                    }
                    Err(e) => {
                        skyward_sys::kmsg(&format!("the update is not given up: {e:#}"))?;
                        self.app.report(&skyward_ui::fmt::give_up_failed(lang), now);
                        self.app.set_update(skyward_vab::standing(slot).unwrap_or(Standing::Unknown));
                    }
                }
            }
            Effect::Wlan(on) => wlan_worker(&self.wlan)?.send(if on { wlan::Command::On } else { wlan::Command::Off }),
            Effect::ScanWlan => {
                self.scanned_at = Instant::now();
                wlan_worker(&self.wlan)?.send(wlan::Command::Scan);
            }
            Effect::JoinWlan { ssid, name, security, password } => {
                // Kept to write down once the network takes it.
                self.wlan_password = Some(password.0.clone());
                wlan_worker(&self.wlan)?.send(wlan::Command::Join { ssid, name, security, password: password.0 })
            }
            Effect::LeaveWlan => wlan_worker(&self.wlan)?.send(wlan::Command::Leave),
            // The folder the file manager shows is served to a browser on the
            // network the phone joined, so a computer moves files in and out
            // without a cable.
            Effect::Serve(on) => {
                if on {
                    ensure!(self.serving.is_none(), "the folder is served already");
                    let folder = PathBuf::from(&self.app.state.files.path);
                    let address = self
                        .app
                        .state
                        .wlan
                        .as_ref()
                        .and_then(|w| w.joined.as_ref())
                        .and_then(|j| match &j.stage {
                            Joining::Connected(link) => Some(link.address.to_string()),
                            _ => None,
                        });
                    let at = match address {
                        Some(ip) => format!("http://{ip}:{}/", crate::fileserve::PORT),
                        None => format!("http://<phone>:{}/", crate::fileserve::PORT),
                    };
                    match crate::fileserve::Server::start(folder.clone()) {
                        Ok(server) => {
                            skyward_sys::kmsg(&format!("serving {} at {at}", folder.display()))?;
                            self.serving = Some(server);
                            self.app.state.serving = skyward_ui::app::Serving::Running { at, served: 0 };
                        }
                        Err(e) => {
                            let why = format!("{e:#}");
                            skyward_sys::kmsg(&format!("the folder is not served: {why}"))?;
                            self.app.state.serving = skyward_ui::app::Serving::Failed(why);
                        }
                    }
                    self.app.refresh();
                } else {
                    if self.serving.take().is_some() {
                        skyward_sys::kmsg("the folder is no longer served")?;
                    }
                    self.app.state.serving = skyward_ui::app::Serving::Off;
                    self.app.refresh();
                }
            }
            // A file from an address, into Downloads, under the name the
            // address ends in.
            Effect::Fetch(url) => {
                ensure!(self.fetcher.is_none(), "a download is under way already");
                let name = fetch::name_of(&url);
                let path = Path::new(skyward_crypto::data::SDCARD).join("Download").join(&name);
                skyward_sys::kmsg(&format!("fetching {url} into {}", path.display()))?;
                self.fetcher = Some(fetch::Worker::start(url, path, self.servers.clone())?);
            }
            Effect::StopFetch => {
                if let Some(worker) = &self.fetcher {
                    skyward_sys::kmsg("the download is stopped, as the page asked")?;
                    worker.stop();
                }
            }
            // Verified boot's two checks in the slot the phone runs from, as
            // the avb page reads them and as it writes what was turned.
            Effect::AvbRead => {
                use skyward_install::commands::avbctl::{self, Switch};
                let read = (|| -> anyhow::Result<(bool, bool)> {
                    Ok((avbctl::read(Switch::Verity)?, avbctl::read(Switch::Verification)?))
                })();
                self.app.state.avb = Some(read.map_err(|e| format!("{e:#}")));
                self.app.refresh();
            }
            Effect::AvbSet(verity, verification) => {
                use skyward_install::commands::avbctl::{self, Switch};
                let wrote = (|| -> anyhow::Result<(bool, bool)> {
                    avbctl::turn(Switch::Verity, verity)?;
                    avbctl::turn(Switch::Verification, verification)?;
                    Ok((avbctl::read(Switch::Verity)?, avbctl::read(Switch::Verification)?))
                })();
                match wrote {
                    Ok(now) => {
                        skyward_sys::kmsg(&format!("verified boot's checks are now verity {now:?}"))?;
                        self.app.state.avb = Some(Ok(now));
                        self.app.state.avb_pending = None;
                        self.app.state.avb_notice = Some(format!(
                            "已写入：逐块校验 {}，启动校验 {}",
                            if now.0 { "开" } else { "关" },
                            if now.1 { "开" } else { "关" }
                        ));
                    }
                    Err(e) => {
                        let why = format!("{e:#}");
                        skyward_sys::kmsg(&format!("the checks are not written: {why}"))?;
                        self.app.state.avb = Some(Err(why.clone()));
                        self.app.state.avb_notice = Some(why);
                    }
                }
                self.app.refresh();
            }
            // vbmeta 的备份与写回：存一份、列出来、把选中的写回去。
            Effect::VbmetaDump => {
                use skyward_install::commands::avbctl;
                let kept = (|| -> anyhow::Result<avbctl::Kept> { avbctl::keep() })();
                match kept {
                    Ok(one) => {
                        skyward_sys::kmsg(&format!(
                            "vbmeta of the {} slot kept at {} (rollback {})",
                            one.slot, one.path, one.rollback_index
                        ))?;
                        self.app.state.vbmeta_note = Some(format!(
                            "已存 {}：回滚 {}，{}",
                            one.slot.to_uppercase(),
                            one.rollback_index,
                            one.name
                        ));
                        let _ = self.effect(Effect::VbmetaList, monotonic_ms())?;
                    }
                    Err(e) => {
                        let why = format!("{e:#}");
                        skyward_sys::kmsg(&format!("vbmeta is not kept: {why}"))?;
                        self.app.state.vbmeta_note = Some(why);
                    }
                }
                self.app.refresh();
            }
            Effect::VbmetaList => {
                use skyward_install::commands::avbctl;
                let list = (|| -> anyhow::Result<Vec<avbctl::Kept>> { avbctl::kept() })();
                let rows = list
                    .map(|rows| {
                        rows.into_iter()
                            .map(|one| skyward_ui::app::VbmetaKept {
                                path: one.path,
                                name: one.name,
                                bytes: one.bytes,
                                rollback_index: one.rollback_index,
                                slot: one.slot,
                            })
                            .collect::<Vec<_>>()
                    })
                    .map_err(|e| format!("{e:#}"));
                self.app.state.vbmeta_kept = Some(rows);
                let count = self.app.state.vbmeta_kept.as_ref().and_then(|r| r.as_ref().ok()).map_or(0, |l| l.len());
                if self.app.state.vbmeta_choice.is_some_and(|k| k >= count) {
                    self.app.state.vbmeta_choice = None;
                }
                self.app.refresh();
            }
            Effect::VbmetaPutBack(path) => {
                use skyward_install::commands::avbctl;
                let wrote = (|| -> anyhow::Result<avbctl::Kept> { avbctl::put_back(&path) })();
                match wrote {
                    Ok(one) => {
                        skyward_sys::kmsg(&format!(
                            "vbmeta put back from {} (rollback {})",
                            one.name, one.rollback_index
                        ))?;
                        self.app.state.vbmeta_note = Some(format!(
                            "已写回 {}：回滚 {}，下次启动按它来",
                            one.name, one.rollback_index
                        ));
                    }
                    Err(e) => {
                        let why = format!("{e:#}");
                        skyward_sys::kmsg(&format!("vbmeta is not put back: {why}"))?;
                        self.app.state.vbmeta_note = Some(why);
                    }
                }
                self.app.refresh();
            }
            Effect::KeepSort(key, desc, folders) => {
                self.app.state.sort = key;
                self.app.state.sort_desc = desc;
                self.app.state.folders_first = folders;
                match crate::sortmem::write(key, desc, folders) {
                    Ok(()) => skyward_sys::kmsg("the file order is kept for the starts that follow")?,
                    Err(e) => skyward_sys::kmsg(&format!("the file order is not kept: {e:#}"))?,
                }
            }
            Effect::Remember(on) => {
                self.app.state.remember = on;
                if !on {
                    self.remembered = None;
                    match crate::remember::forget() {
                        Ok(()) => skyward_sys::kmsg("the screen lock kept is forgotten, and none is kept from now on")?,
                        Err(e) => skyward_sys::kmsg(&format!("the screen lock kept was not forgotten: {e:#}"))?,
                    }
                    // A mark, so the switch stays off at the next start even
                    // with no lock kept to say so.
                    let _ = crate::remember::opt_out();
                } else {
                    let _ = crate::remember::opt_in();
                    skyward_sys::kmsg("the screen lock is kept once it opens the user's files")?;
                }
            }
            Effect::WlanAuto(on) => {
                crate::wlanmem::set_auto(on)?;
                skyward_sys::kmsg(if on {
                    "the WLAN opens as the recovery starts"
                } else {
                    "the WLAN stays off as the recovery starts"
                })?;
            }
            Effect::WlanKeep(on) => {
                crate::wlanmem::set_keep(on)?;
                if !on {
                    self.wlan_saved = None;
                }
                skyward_sys::kmsg(if on {
                    "the WLAN password is kept once a network is joined"
                } else {
                    "the WLAN password kept is forgotten"
                })?;
            }
            Effect::CheckUpdate => self.check_update()?,
            Effect::Download(release) => self.download(release, now)?,
            Effect::StopDownload => self.downloader.as_ref().context("there is no download to stop")?.stop(),
            Effect::CheckSkyward => {
                ensure!(self.skyward_checker.is_none(), "GitHub is being asked already");
                let probe = cfg.wlan.as_ref().context("the phone's WLAN is not brought up")?.probe.clone();
                let task = updates::Task::CheckSkyward { probe, codename: cfg.device.codename.clone() };
                self.skyward_checker = Some(updates::Worker::start(task, self.servers.clone())?);
            }
            // The zip goes into the recovery's own folder, which is
            // in memory, before its image goes into the recovery
            // partitions.
            Effect::UpdateSkyward(build) => {
                ensure!(self.skyward_updater.is_none(), "Skyward is being updated already");
                ensure!(self.installer.is_none(), "an installer is running already");
                skyward_sys::kmsg(&format!("updating Skyward to {}", build.name))?;
                let path = Path::new(skyward_sys::RUN).join(&build.name);
                let task = updates::Task::UpdateSkyward { build, codename: cfg.device.codename.clone(), path };
                self.skyward_updater = Some(updates::Worker::start(task, self.servers.clone())?);
            }
            Effect::StopSkywardUpdate => self.skyward_updater.as_ref().context("Skyward is not being updated")?.stop(),
            // Logs that cannot be saved are the page's to report;
            // the recovery carries on.
            Effect::ExportLogs { recovery, kernel, storage } => {
                let into = match storage {
                    true => Path::new(skyward_crypto::data::SDCARD).join(STORAGE_LOGS),
                    false => PathBuf::from(TMP_LOGS),
                };
                match export_logs(recovery, kernel, &into, self.app.state.time_zone) {
                    Ok(folder) => self.app.report(&skyward_ui::fmt::exported(self.app.state.lang, &folder.display().to_string()), now),
                    Err(e) => self.app.report(&skyward_ui::fmt::export_failed(self.app.state.lang, &format!("{e:#}")), now),
                }
            }
            Effect::SetMode { path, mode } => {
                let outcome = skyward_files::set_mode(Path::new(&path), mode).map_err(|e| format!("{e:#}"));
                self.app.mode_set(mode, outcome, now);
            }
            Effect::OpenText(path) => {
                let read = skyward_files::read_up_to(Path::new(&path), skyward_ui::editor::MAX_BYTES).map_err(|e| format!("{e:#}"));
                self.app.text_opened(&self.fonts, path, read, now);
            }
            // The folder is listed again, for the file's new size
            // and time.
            Effect::SaveText { path, text } => match skyward_files::replace(Path::new(&path), text.as_bytes()) {
                Ok(()) => {
                    self.app.text_saved(Ok(()), now);
                    let folder = self.app.state.files.path.clone();
                    list(&mut self.app, folder)?;
                }
                Err(e) => self.app.text_saved(Err(format!("{e:#}")), now),
            },
            Effect::Rename { path, name } => match skyward_files::rename(Path::new(&path), &name) {
                Ok(_) => {
                    self.app.renamed(Ok(()), now);
                    let folder = self.app.state.files.path.clone();
                    list(&mut self.app, folder)?;
                }
                Err(e) if e.is::<skyward_files::Taken>() => self.app.renamed(Err(RenameRefused::Taken), now),
                Err(e) => self.app.renamed(Err(RenameRefused::Failed(format!("{e:#}"))), now),
            },
            // A partition that will not mount, or is in use, is the
            // page's to report; the recovery carries on.
            Effect::Mount(base) => {
                let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                if let Err(e) = skyward_vab::mount_one(slot, &base) {
                    self.app.mount_failed(MountFailure { mounting: true, base, detail: format!("{e:#}") });
                }
                self.app.set_mounts(self.mounts());
            }
            Effect::Unmount(base) => {
                let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                if let Err(e) = skyward_vab::unmount_one(slot, &base) {
                    self.app.mount_failed(MountFailure { mounting: false, base, detail: format!("{e:#}") });
                }
                self.app.set_mounts(self.mounts());
            }
            Effect::EnterRecovery => self.enter_recovery()?,
            Effect::FileTransfer(on) => self.file_transfer(on, now)?,
        }
        Ok(Vec::new())
    }

    /// The second system's work, which only a phone that can hold a second
    /// system has.
    fn systems(&mut self) -> Result<&mut systems::Worker> {
        self.systems_worker.as_mut().context("this phone holds no second system")
    }

    /// Whether an install may start: none runs, and file transfer has let
    /// internal storage go, which the page is told otherwise.
    fn may_install(&mut self, now: u64) -> Result<bool> {
        ensure!(self.installer.is_none(), "an installer is running already");
        withhold(&mut self.transfer, &mut self.app, now)
    }

    /// Erase all user data, as the page the system's request showed asks.
    fn reset_asked(&mut self, now: u64) -> Result<()> {
        ensure!(self.eraser.is_none(), "erasing is under way already");
        if !withhold(&mut self.transfer, &mut self.app, now)? {
            return Ok(());
        }
        self.eraser = Some(factory_reset(&mut self.app, self.request.keep_memtag_mode, true, None)?);
        self.erase_leaves = true;
        Ok(())
    }

    /// Erase what the wipe page chose. Opening the data partition and
    /// erasing it are not to cross; the page says to wait for the one under
    /// way.
    fn wipe(&mut self, chosen: Vec<Wipe>, now: u64) -> Result<()> {
        ensure!(self.eraser.is_none(), "erasing is under way already");
        if self.decryptor.is_some() {
            self.app.report(self.app.state.lang.pick(DATA_OPENING), now);
            return Ok(());
        }
        if !withhold(&mut self.transfer, &mut self.app, now)? {
            return Ok(());
        }
        self.erase_leaves = false;
        if chosen.contains(&Wipe::All) {
            self.eraser = Some(factory_reset(&mut self.app, false, false, None)?);
        } else if chosen.iter().any(|&w| w != Wipe::Cache) && self.app.state.data == Data::Erased {
            self.app.erase_started(false, false, Vec::new());
            self.app.erase_ended(Err(EraseStop::Erased), now);
        } else {
            let steps = chosen.into_iter().map(step).collect();
            self.eraser = Some(parts_wipe(&mut self.app, steps, &self.cfg.wipe.logs, false, None)?);
        }
        Ok(())
    }

    /// Start the terminal's shell in `folder`. A shell that will not start
    /// is the page's to report; the recovery carries on.
    fn start_shell(&mut self, folder: &str, now: u64) -> Result<()> {
        ensure!(self.shell.is_none(), "a shell runs already");
        let (rows, cols) = self.app.term_size(&self.fonts);
        match pty::Shell::start(rows, cols, &self.cfg.device.codename, Path::new(folder)) {
            Ok(sh) => {
                self.shell = Some(sh);
                self.app.term_started(rows, cols);
            }
            Err(e) => {
                skyward_sys::kmsg(&format!("no shell for the terminal: {e:#}"))?;
                self.app.report(&format!("{e:#}"), now);
            }
        }
        Ok(())
    }

    /// Have the terminal's shell go to `folder`, while it waits for a
    /// command; one that will not is the page's to report.
    fn shell_to(&mut self, folder: &str, now: u64) -> Result<()> {
        let sh = self.shell.as_mut().context("a folder went to a shell that has ended")?;
        let went = match sh.waiting() {
            Ok(true) => sh.change_to(folder),
            Ok(false) => {
                self.app.report(self.app.state.lang.pick(skyward_ui::fmt::TERMINAL_BUSY), now);
                Ok(())
            }
            Err(e) => Err(e),
        };
        if let Err(e) = went {
            self.app.report(&format!("{e:#}"), now);
        }
        Ok(())
    }

    /// Stop waiting for the computer, for where an image goes, or for an
    /// update to be agreed to.
    fn cancel_install(&mut self, now: u64) -> Result<()> {
        match &self.installer {
            Some(i) => i.cancel()?,
            // The update the system asked for, waiting on the
            // lock of the user whose files hold it, is given up
            // as its page is left, and is done with.
            None => {
                ensure!(self.pending.take().is_some(), "no installer to cancel");
                skyward_sys::kmsg("the update asked for is given up, its page left")?;
                self.done_with_request()?;
                let cancelled = skyward_install::Event::Finished(skyward_install::Outcome::Cancelled);
                self.app.install(cancelled, now);
            }
        }
        Ok(())
    }

    /// Copy or move `path` into the folder `into`. A name taken where an
    /// entry goes is the user's to decide on before anything is written.
    /// Copy or move several entries into a folder, asking what to do where a
    /// name is taken as one copy would.
    fn paste_many(&mut self, op: FileOp, paths: Vec<String>, into: String, clash: Option<ClashChoice>, now: u64) -> Result<()> {
        ensure!(self.worker.is_none(), "work on files is under way already");
        let sources: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let clash = match clash {
            Some(choice) => clash_rule(choice),
            None => match skyward_files::clashes(&sources, Path::new(&into)) {
                Ok(taken) if taken.is_empty() => skyward_files::Clash::Refuse,
                Ok(taken) => {
                    let names = taken.iter().map(|t| t.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())).collect();
                    let first = paths.first().cloned().unwrap_or_default();
                    self.app.file_clash(op, first, into, names);
                    return Ok(());
                }
                Err(e) => {
                    self.app.report(&format!("{e:#}"), now);
                    return Ok(());
                }
            },
        };
        let ownership = match ownership(Path::new(&into)) {
            Ok(ownership) => ownership,
            Err(e) => {
                self.app.report(&format!("{e:#}"), now);
                return Ok(());
            }
        };
        let to = PathBuf::from(&into);
        let first = paths.first().cloned().unwrap_or_default();
        let (task, shown) = match op {
            FileOp::Copy => (filework::Task::Copy { from: sources, into: to, clash, ownership }, FileTask::Copy { path: first, into }),
            FileOp::Move => (filework::Task::Move { from: sources, into: to, clash, ownership }, FileTask::Move { path: first, into }),
        };
        self.worker = Some(filework::Worker::start(task)?);
        self.app.file_job_started(shown);
        Ok(())
    }

    fn paste(&mut self, op: FileOp, path: String, into: String, clash: Option<ClashChoice>, now: u64) -> Result<()> {
        ensure!(self.worker.is_none(), "work on files is under way already");
        // What is being put where, so a failure says which way it went.
        let _ = skyward_sys::kmsg(&format!("files: {op:?} {path} into {into}"));
        let clash = match clash {
            Some(choice) => clash_rule(choice),
            None => match skyward_files::clashes(&[PathBuf::from(&path)], Path::new(&into)) {
                Ok(taken) if taken.is_empty() => skyward_files::Clash::Refuse,
                Ok(taken) => {
                    let names = taken.iter().map(|t| t.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())).collect();
                    self.app.file_clash(op, path, into, names);
                    return Ok(());
                }
                Err(e) => {
                    self.app.report(&format!("{e:#}"), now);
                    return Ok(());
                }
            },
        };
        let ownership = match ownership(Path::new(&into)) {
            Ok(ownership) => ownership,
            Err(e) => {
                self.app.report(&format!("{e:#}"), now);
                return Ok(());
            }
        };
        let (from, to) = (vec![PathBuf::from(&path)], PathBuf::from(&into));
        let (task, shown) = match op {
            FileOp::Copy => (filework::Task::Copy { from, into: to, clash, ownership }, FileTask::Copy { path, into }),
            FileOp::Move => (filework::Task::Move { from, into: to, clash, ownership }, FileTask::Move { path, into }),
        };
        self.worker = Some(filework::Worker::start(task)?);
        self.app.file_job_started(shown);
        Ok(())
    }

    /// Give the data partition's files the labels the installed system's
    /// rules give them. Rules kept on a partition that is not mounted are
    /// the page's to report, with what to do first.
    fn relabel(&mut self, now: u64) -> Result<()> {
        ensure!(self.relabeler.is_none(), "relabeling is under way already");
        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
        match relabel::rule_files(slot) {
            Ok(rules) => {
                self.relabeler = Some(filework::Worker::start(filework::Task::Relabel { rules })?);
                self.app.relabel_started();
            }
            Err(e) => match e.downcast_ref::<relabel::Unmounted>() {
                Some(relabel::Unmounted(base)) => self.app.report(&skyward_ui::fmt::mount_first(self.app.state.lang, base), now),
                None => self.app.report(&skyward_ui::fmt::rules_unread(self.app.state.lang, &format!("{e:#}")), now),
            },
        }
        Ok(())
    }

    /// Repair the filesystem of the partition `name`, or make it afresh.
    /// The data partition made afresh as another filesystem is a factory
    /// reset, as the system asks for one; the rest is work on one
    /// partition. Opening the data partition and working on it are not to
    /// cross, and file transfer lets internal storage go first.
    fn filesystem(&mut self, name: &str, action: FsAction, now: u64) -> Result<()> {
        ensure!(self.eraser.is_none() && self.part_worker.is_none(), "work on partitions is under way already");
        if name == DATA_PARTITION && self.decryptor.is_some() {
            self.app.report(self.app.state.lang.pick(DATA_OPENING), now);
            return Ok(());
        }
        if name == DATA_PARTITION && !withhold(&mut self.transfer, &mut self.app, now)? {
            return Ok(());
        }
        let part = fs_part(name)?;
        let (kind, task) = match action {
            FsAction::Remake(filesystem) if name == DATA_PARTITION => {
                self.erase_leaves = false;
                self.eraser = Some(factory_reset(&mut self.app, false, false, Some(filesystem))?);
                return Ok(());
            }
            FsAction::Repair => (PartKind::Repair, partwork::Task::Repair(part)),
            FsAction::Remake(filesystem) => (PartKind::Remake(filesystem.clone()), partwork::Task::Remake(part, filesystem)),
        };
        skyward_sys::kmsg(&format!("{kind:?} the filesystem of {name}, as the filesystem page asked"))?;
        self.app.part_work_started(kind, false);
        self.part_worker = Some(partwork::Worker::start(task)?);
        Ok(())
    }

    /// Back the parts chosen up into `place`. An install still stopping may
    /// yet write; a backup waits for it.
    fn back_up(&mut self, parts: [bool; 4], place: Place, compress: bool, verify: bool, now: u64) -> Result<()> {
        ensure!(self.backuper.is_none(), "a backup or restore is under way already");
        if self.installer.is_some() {
            self.app.report(self.app.state.lang.pick(INSTALL_UNFINISHED), now);
            return Ok(());
        }
        let into = backups_in(place);
        let there = into.ancestors().find(|p| p.exists()).context("the root is always there")?;
        let ownership = match ownership(there) {
            Ok(ownership) => ownership,
            Err(e) => {
                self.app.report(&format!("{e:#}"), now);
                return Ok(());
            }
        };
        let made = unix_now()?;
        let task = backup::BackupTask {
            parts,
            stamp: skyward_ui::fmt::stamp(made, self.app.state.time_zone),
            into,
            ownership,
            compress,
            verify,
            remount: (place == Place::UsbDrive).then(|| PathBuf::from(USB_DRIVE)),
            made,
            device: self.cfg.device.codename.clone(),
            system: self.info.os_version.clone(),
            efs: self.cfg.backup.efs.clone(),
            skipped: backup::data_skipped(&self.cfg.backup.data_skipped),
        };
        self.backuper = Some(backup::Worker::start(backup::Task::Backup(task))?);
        self.app.backup_started(false, place, verify);
        Ok(())
    }

    /// Restore the parts chosen of the backup `name` in `place`. An install
    /// still stopping may yet write; a restore waits for it.
    fn restore(&mut self, place: Place, name: &str, parts: [bool; 4], now: u64) -> Result<()> {
        ensure!(self.backuper.is_none(), "a backup or restore is under way already");
        if self.installer.is_some() {
            self.app.report(self.app.state.lang.pick(INSTALL_UNFINISHED), now);
            return Ok(());
        }
        let task = backup::RestoreTask {
            folder: backups_in(place).join(name),
            parts,
            device: self.cfg.device.codename.clone(),
            skipped: backup::data_skipped(&self.cfg.backup.data_skipped),
            detach: detach(),
        };
        self.backuper = Some(backup::Worker::start(backup::Task::Restore(task))?);
        self.app.backup_started(true, place, true);
        Ok(())
    }

    /// Ask the update server for the newest system. What the installed
    /// system is called is the page's to report when it cannot be read.
    fn check_update(&mut self) -> Result<()> {
        ensure!(self.checker.is_none(), "the update server is being asked already");
        let server = self.cfg.update.as_ref().context("the phone's update server is not known")?.server.clone();
        let probe = self.cfg.wlan.as_ref().context("the phone's WLAN is not brought up")?.probe.clone();
        match installed(self.cfg, &self.info) {
            Ok(installed) => {
                self.checker = Some(updates::Worker::start(updates::Task::Check { server, probe, installed }, self.servers.clone())?)
            }
            Err(e) => self.app.update_checked(Err(Unanswered::Failed(format!("{e:#}"))), Have::Nothing),
        }
        Ok(())
    }

    /// Download `release`'s full package into internal storage.
    fn download(&mut self, release: Release, now: u64) -> Result<()> {
        ensure!(self.downloader.is_none(), "the full package is being downloaded already");
        let server = self.cfg.update.as_ref().context("the phone's update server is not known")?.server.clone();
        let path = downloaded(&release);
        match installed(self.cfg, &self.info) {
            Ok(installed) => {
                let task = updates::Task::Download { server, installed, release, path };
                self.downloader = Some(updates::Worker::start(task, self.servers.clone())?);
            }
            Err(e) => {
                let have = have(&self.app, &release);
                self.app.download_ended(DownloadEnd::Failed(Failure::Other(format!("{e:#}"))), have, now);
            }
        }
        Ok(())
    }

    /// Leave fastboot for the recovery's pages. init stops fastbootd as the
    /// port goes through "none"; then the slot's partitions come up as at
    /// boot.
    fn enter_recovery(&mut self) -> Result<()> {
        skyward_sys::usb::switch(skyward_sys::usb::resting(), USB_TIMEOUT)?;
        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
        crate::prepare(self.cfg, slot)?;
        self.read_at = Instant::now();
        read_details(self.cfg, &mut self.info, &mut self.unread.details)?;
        self.fastboot = false;
        self.app.enter_recovery();
        if let Some(w) = self.systems_worker.as_mut() {
            w.send(systems::Task::Resume);
        }
        Ok(())
    }

    /// Turn file transfer on or off. A port that something else has takes
    /// file transfer on, or leaves it, when it comes back to rest.
    fn file_transfer(&mut self, on: bool, now: u64) -> Result<()> {
        let rests = !self.fastboot && !self.sideloading;
        match (on, self.transfer.take()) {
            (true, None) => match Transfer::start(self.cfg, &self.info, self.app.state.time_zone) {
                Ok(t) => {
                    self.transfer = Some(t);
                    if rests {
                        take_file_transfer(&mut self.app, now)?;
                    }
                }
                Err(e) => {
                    skyward_sys::kmsg(&format!("file transfer did not start: {e:#}"))?;
                    self.app.file_transfer_failed(false, &skyward_ui::fmt::transfer_failed(self.app.state.lang, &format!("{e:#}")), now);
                }
            },
            (false, Some(t)) => {
                if let Err(e) = t.stop(rests) {
                    skyward_sys::kmsg(&format!("file transfer did not stop cleanly: {e:#}"))?;
                    self.app.report(&skyward_ui::fmt::transfer_left_on(self.app.state.lang, &format!("{e:#}")), now);
                }
            }
            (on, t) => unreachable!("the switch turned file transfer {on} while it was {}", t.is_some()),
        }
        Ok(())
    }
}

/// The modules the phone's own system loads: KernelSU and Magisk keep them the
/// same way — a folder each under `/data/adb/modules` with a `module.prop`
/// inside, a `disable` file when it is switched off and a `remove` file when it
/// is to go at the next start.
fn read_modules() -> Result<Vec<skyward_ui::app::ModuleRow>, String> {
    use std::path::Path;
    const DIR: &str = "/data/adb/modules";
    let list = match std::fs::read_dir(DIR) {
        Ok(list) => list,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{DIR} is not read: {e}")),
    };
    let mut out = Vec::new();
    for entry in list.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let prop = std::fs::read_to_string(Path::new(&path).join("module.prop")).unwrap_or_default();
        if prop.trim().is_empty() {
            continue;
        }
        let field = |name: &str| {
            let key = format!("{name}=");
            prop.lines()
                .find_map(|line| line.trim().strip_prefix(&key).map(|v| v.trim().to_string()))
                .unwrap_or_default()
        };
        let (name, version, author) = (field("name"), field("version"), field("author"));
        let note = match (version.is_empty(), author.is_empty()) {
            (true, true) => String::new(),
            (false, true) => version,
            (true, false) => author,
            (false, false) => format!("{version} · {author}"),
        };
        out.push(skyward_ui::app::ModuleRow {
            id: id.clone(),
            name: if name.is_empty() { id } else { name },
            note,
            about: field("description"),
            enabled: !path.join("disable").exists(),
            removing: path.join("remove").exists(),
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Switch the module in `id` on or off: `disable` inside its folder is what the
/// phone's own system reads.
fn switch_module(id: &str, on: bool) -> std::io::Result<()> {
    let flag = std::path::Path::new("/data/adb/modules").join(id).join("disable");
    match on {
        true => match std::fs::remove_file(&flag) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        },
        false => std::fs::write(&flag, b""),
    }
}

/// Mark the module in `id` to be removed at the next start, or take the mark
/// off: KernelSU and Magisk both read a `remove` file inside the module's
/// folder.
fn mark_module_remove(id: &str, on: bool) -> std::io::Result<()> {
    let flag = std::path::Path::new("/data/adb/modules").join(id).join("remove");
    match on {
        true => std::fs::write(&flag, b""),
        false => match std::fs::remove_file(&flag) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        },
    }
}

/// Install a module from the zip at `path`: its `module.prop` names it, and
/// everything beside that file goes into `/data/adb/modules/<id>`, replacing a
/// copy already there.
fn install_module(path: &str) -> Result<String, String> {
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{path} is not a zip: {e}"))?;
    // The folder the module sits in inside the zip: the shortest path to a
    // module.prop. Modules are packed both at the root and inside one folder.
    let mut prefix: Option<String> = None;
    for i in 0..zip.len() {
        let name = zip.by_index(i).map_err(|e| format!("{path}: {e}"))?.name().to_string();
        if let Some(here) = name.strip_suffix("module.prop") {
            let here = here.to_string();
            if prefix.as_ref().is_none_or(|p| here.len() < p.len()) {
                prefix = Some(here);
            }
        }
    }
    let prefix = prefix.ok_or_else(|| format!("{path} carries no module.prop"))?;
    let prop = {
        let mut entry = zip.by_name(&format!("{prefix}module.prop")).map_err(|e| format!("{path}: {e}"))?;
        let mut text = String::new();
        entry.read_to_string(&mut text).map_err(|e| format!("{path}: {e}"))?;
        text
    };
    let field = |key: &str| {
        let want = format!("{key}=");
        prop.lines().find_map(|line| line.trim().strip_prefix(&want).map(|v| v.trim().to_string())).unwrap_or_default()
    };
    let mut id = field("id");
    if id.is_empty() {
        id = field("name").to_lowercase().replace(' ', "_");
    }
    if id.is_empty() || id.contains('/') || id.contains("..") {
        return Err("the module names no folder to sit in".into());
    }
    let dest = std::path::Path::new("/data/adb/modules").join(&id);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| format!("clear {}: {e}", dest.display()))?;
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("{path}: {e}"))?;
        let name = entry.name().to_string();
        let Some(rest) = name.strip_prefix(prefix.as_str()) else { continue };
        if rest.is_empty() || name.ends_with('/') {
            continue;
        }
        let at = dest.join(rest);
        if let Some(parent) = at.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| format!("{path}: {e}"))?;
        std::fs::write(&at, &data).map_err(|e| format!("write {}: {e}", at.display()))?;
        // 脚本要能执行，其余按普通文件。
        let mode = if rest.ends_with(".sh") { 0o755 } else { 0o644 };
        let _ = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(mode));
    }
    Ok(id)
}


/// 只把 `/data/app` 的目录名字列出来：不读 packages.xml、不算体积，毫秒级。
/// 先拿它把列表和翻页键画出来，时间和体积随后再补（2026-11-18）。
fn dirs_only() -> Vec<skyward_ui::app::dataapp::DataApp> {
    use skyward_ui::app::dataapp as app;
    let Ok(top) = std::fs::read_dir("/data/app") else { return Vec::new() };
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for entry in top.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("~~") {
            if let Ok(inner) = std::fs::read_dir(&path) {
                dirs.extend(inner.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
            }
        } else {
            dirs.push(path);
        }
    }
    let mut out = Vec::new();
    for path in dirs {
        if !path.join("base.apk").exists() {
            continue;
        }
        let dir_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let package = app::package_of(&dir_name);
        out.push(app::DataApp {
            label: package.clone(),
            package,
            dir: path.to_string_lossy().into_owned(),
            version: String::new(),
            installed: 0,
            updated: 0,
            size: 0,
            icon: Vec::new(),
            system: false,
        });
    }
    out.sort_by(|a, b| a.package.cmp(&b.package));
    out
}

/// `/data/app` 里有哪些应用、装在什么时候：只读目录和 `packages.xml`，不碰 apk
/// 里面，所以是毫秒级的。名字先拿包名顶着，随后由 [`enrich_one`] 补上。
/// 数据分区应用那一路线程发回来的东西：整张列表，或者一个应用的名字与图标。
enum Feed {
    System(Vec<skyward_ui::app::dataapp::DataApp>),
    Apps(Vec<skyward_ui::app::dataapp::DataApp>),
    Name(usize, String, Option<(u32, u32, Vec<u8>)>),
    Failed(String),
}

fn quick_data_apps() -> Result<Vec<skyward_ui::app::dataapp::DataApp>, String> {
    use skyward_ui::app::dataapp as app;
    let times = std::fs::read_to_string("/data/system/packages.xml")
        .map(|text| app::install_times(&text))
        .unwrap_or_default();
    let mut out = Vec::new();
    // 只看用户自己装的应用：预装的那些在 /system、/product 这些分区里，
    // 不是这台机器的主人装的，也就不是这里要管的（2026-11-18）。
    let mut user_dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(top) = std::fs::read_dir("/data/app") {
        for entry in top.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("~~") {
                if let Ok(inner) = std::fs::read_dir(&path) {
                    user_dirs.extend(inner.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
                }
            } else {
                user_dirs.push(path);
            }
        }
    }
    for path in user_dirs {
        let Some(apk) = apk_in(&path) else { continue };
        let dir_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let package = app::package_of(&dir_name);
        // 存档里没记这个包（自装、或安装记录被清过）时，用 apk 文件的修改时间
        // 兜底：它通常就是装进来或更新那一刻。
        let (installed, updated) = match times.get(&package).copied() {
            Some((first, last)) if first > 0 => (first, last),
            _ => {
                let when = std::fs::metadata(&apk)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                (when * 1000, when * 1000)
            }
        };
        out.push(app::DataApp {
            label: package.clone(),
            package,
            dir: path.to_string_lossy().into_owned(),
            version: String::new(),
            installed: installed / 1000,
            updated: updated / 1000,
            size: app::size_of(&path),
            icon: Vec::new(),
            system: false,
        });
    }
    // 系统分区里的预装应用：与用户应用分开标记，页面上用开关切换。
    out.sort_by(|a, b| b.installed.cmp(&a.installed).then_with(|| a.package.cmp(&b.package)));
    Ok(out)
}

/// 一个目录里的 apk：`base.apk` 优先，否则第一个 `.apk`。
fn apk_in(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let base = dir.join("base.apk");
    if base.is_file() {
        return Some(base);
    }
    let list = std::fs::read_dir(dir).ok()?;
    list.flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "apk"))
}

/// 系统分区里的预装应用：只读页用。recovery 里手机分区挂在 /mnt 下。
fn system_apps() -> Vec<skyward_ui::app::dataapp::DataApp> {
    use skyward_ui::app::dataapp as app;
    let mut out = Vec::new();
    for root in [
        "/mnt/system/system/app",
        "/mnt/system/system/priv-app",
        "/mnt/product/app",
        "/mnt/product/priv-app",
        "/mnt/system_ext/app",
        "/mnt/system_ext/priv-app",
        "/vendor/app",
        "/odm/app",
    ] {
        let Ok(list) = std::fs::read_dir(root) else { continue };
        for entry in list.flatten() {
            let path = entry.path();
            let (dir, apk) = if path.is_dir() {
                (path.clone(), apk_in(&path))
            } else if path.extension().is_some_and(|e| e == "apk") {
                // 单个 apk 摆在目录里：记它自己的路径，导出时照它复制。
                (path.clone(), Some(path.clone()))
            } else {
                continue;
            };
            let Some(apk) = apk else { continue };
            let stem = apk.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let package = if stem.eq_ignore_ascii_case("base") {
                dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            } else {
                stem
            };
            if package.is_empty() {
                continue;
            }
            out.push(app::DataApp {
                label: package.clone(),
                package,
                dir: dir.to_string_lossy().into_owned(),
                version: String::new(),
                installed: 0,
                updated: 0,
                // 目录里那几个文件的字节数，详情页要给人看它占多大。
                size: match std::fs::metadata(&dir) {
                    Ok(m) if m.is_file() => m.len(),
                    _ => app::size_of(&dir),
                },
                icon: Vec::new(),
                system: true,
            });
        }
    }
    out.sort_by(|a, b| a.package.cmp(&b.package));
    out
}

/// 一个应用的名字与图标：解它的清单、资源表，必要时还要看自适应图标那张
/// XML。慢，所以只在后台线程里一条一条做。
fn enrich_one(dir: &str) -> Option<(String, Option<(u32, u32, Vec<u8>)>)> {
    use skyward_ui::app::dataapp as app;
    use std::io::Read;
    let path = std::path::Path::new(dir);
    let package = app::package_of(&path.file_name()?.to_string_lossy());
    let file = std::fs::File::open(apk_in(path)?).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let mut manifest = Vec::new();
    zip.by_name("AndroidManifest.xml").ok()?.read_to_end(&mut manifest).ok()?;
    let mut label = app::literal_label(&manifest);
    if label.is_none() {
        if let Some(id) = app::label_id(&manifest) {
            let mut arsc = Vec::new();
            if let Ok(mut entry) = zip.by_name("resources.arsc") {
                let _ = entry.read_to_end(&mut arsc);
            }
            label = app::resolve_string(&arsc, id);
        }
    }
    let label = label.unwrap_or(package);
    // 图标不再取：各应用的图标有矢量、自适应、WebP 几种形态，认不全时反而
    // 一半有一半没有，不如统一显示同一个（2026-11-18）。
    Some((label, None))
}

/// Copy an app's package into internal storage, under the recovery's own
/// folder there: a system app sits on a read-only partition, so taking a copy
/// out is what can be done with it here. `from` is the folder the package
/// sits in, or the package file itself; `name` is what the copy is called.
fn export_apk(from: &str, name: &str) -> Result<PathBuf> {
    let path = Path::new(from);
    let apk = match path.is_file() {
        true => path.to_path_buf(),
        false => apk_in(path).with_context(|| format!("no package in {from}"))?,
    };
    let into = Path::new(skyward_crypto::data::SDCARD).join(STORAGE_APKS);
    let there = into.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    skyward_files::make_folders(&into, ownership)?;
    let dest = into.join(format!("{name}.apk"));
    match fs::remove_file(&dest) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("replace {}", dest.display())),
    }
    let mut source = fs::File::open(&apk).with_context(|| format!("read {}", apk.display()))?;
    let mut file = skyward_files::make_file(&dest, ownership)?;
    std::io::copy(&mut source, &mut file).with_context(|| format!("write {}", dest.display()))?;
    file.sync_all().with_context(|| format!("write {}", dest.display()))?;
    Ok(dest)
}

/// Take an app off: its folder in `/data/app` and what it kept in `/data/data`.
fn remove_data_app(package: &str, dir: &str) -> Result<(), String> {
    // 系统分区的应用删了会让系统起不来，这里只删用户装在 /data 的那部分。
    if !dir.starts_with("/data/app") {
        return Err(format!("{package} 装在系统分区，不能这样卸载"));
    }
    for at in [dir.to_string(), format!("/data/data/{package}")] {
        match std::fs::remove_dir_all(&at) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{at}: {e}")),
        }
    }
    Ok(())
}
