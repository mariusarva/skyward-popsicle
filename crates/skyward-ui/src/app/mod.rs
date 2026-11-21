//! The recovery's pages as one machine: what it holds, what touches, keys
//! and the clock do to it, and what it asks of the runtime in return. Each
//! page's own types, methods and tests are in the module named for it.

use crate::canvas::{Canvas, Run};
use crate::charge;
use crate::color::Rgba;
use crate::editor::{self, Editor};
use crate::files::{self, FileType};
use crate::fling::{Fling, Tracker};
use crate::fmt;
use crate::focus::{self, Group, Spot, Stop};
use crate::font::Fonts;
use crate::geom::Rect;
use crate::glow::{self, GlowClock};
use crate::lang::{Lang, Text};
use crate::motion::{self, Spring};
use crate::pages::{self, Band, Ctx};
use crate::sprite::{Blurs, Sprite};
use crate::search;
use crate::settings::Settings;
use crate::term::{self, Term, TermKey};
use crate::theme::Theme;
use crate::tr;
use crate::widgets::{self, Bar, Bolt, Cap, Layer, WlanMark};
use skyward_info::DeviceInfo;
use skyward_install::{Event, ImageKind, Outcome, Reflash, Refusal, Stage, Step};
use skyward_net::download::{Failure, Progress};
use skyward_net::github::{Build, Newest};
use skyward_net::release::{Release, Unanswered};
use skyward_net::supplicant::Security;
use skyward_net::wlan::{Joined, Network, Power, Stage as Joining, Traffic, Wlan};
use skyward_systems::{Overview, Who};
use skyward_vab::{BootSlots, Hold, Slot, SnapshotState, Standing};

mod act;
mod backup;
mod decrypt;
mod draw;
mod entry;
mod filesystem;
mod frame;
mod input;
mod modules;
pub mod dataapp;
mod dataapp_list;
mod install;
mod keys;
mod manager;
mod mount;
mod nav;
mod power;
mod prefs;
mod reboot;
mod relabel;
mod scroll;
mod skyward;
mod state;
mod systems;
mod terminal;
#[cfg(test)]
mod testing;
mod update;
mod wipe;
mod wlan;

pub use backup::*;
pub use decrypt::*;
pub use entry::*;
pub use filesystem::*;
pub use frame::*;
pub use install::*;
pub use manager::*;
pub use mount::*;
pub use prefs::*;
pub use reboot::*;
pub use relabel::*;
pub use skyward::*;
pub use state::*;


pub use systems::*;
pub use terminal::*;
pub use update::*;
pub use wipe::*;
pub use wlan::*;

/// The pages of the recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Decrypt,
    Home,
    Settings,
    Reboot,
    Wipe,
    /// Mounting and unmounting the slot's partitions.
    Mount,
    Install,
    /// A place's folders and the files in them that can be installed, to
    /// choose one.
    Choose(Place),
    /// Installing a package, which the computer sends with `adb sideload`
    /// or which is a file chosen in a place, and how it went.
    Installing,
    About,
    Specs,
    /// How long the screen stays on with nothing touched.
    ScreenOff,
    /// Finding a feature or partition by name, typed on the screen's own
    /// keyboard.
    Search,
    Backup,
    /// The backups found, to restore one.
    Restore,
    /// One backup, and what to restore from it.
    RestoreBackup,
    /// Backing up or restoring under way, and how it ended.
    BackupJob,
    /// Tools for the experienced: files, a terminal, the logs.
    Advanced,
    /// The modules installed for the phone's own system (KernelSU's and
    /// Magisk's), and whether each is switched on.
    Modules,
    /// One module: what it is, and what can be done with it.
    ModuleInfo,
    /// The apps installed in the phone's data partition, newest first.
    DataApps,
    /// 系统分区里的预装应用：只看，不能动。
    SystemApps,
    /// One of them: what it is, and a way to take it off.
    DataAppInfo,
    Files,
    /// The files on the OTG drive, all of them.
    FilesOtg,
    /// One file, and what can be done with it.
    FileInfo,
    /// The permission bits of the entry the file page shows, a switch each.
    Permissions,
    /// A text file, changed on the screen's keyboard and then saved.
    Editor,
    Terminal,
    /// Giving the data partition's files the SELinux labels the installed
    /// system's rules give them: what it does and a swipe to start, then
    /// how far it has come and how it ended.
    Relabel,
    /// The partitions whose filesystems can be repaired or made afresh as
    /// another.
    Filesystems,
    /// One of them: what it holds, and what to do to it, with a swipe to
    /// do it.
    Filesystem,
    /// Work on whole partitions under way, and how it ended: the
    /// filesystem page's, or what the system asked for before the recovery
    /// started.
    PartWork,
    Language,
    TimeZone,
    /// Saving the logs, as a bug report asks for them.
    Logs,
    /// Where an update the system installed stands, and merging it into
    /// the slot it went to.
    Update,
    /// The files of a folder served to a browser on the network the phone is
    /// on, so a computer moves them in and out without a cable.
    Serve,
    /// A file taken from an address typed on the page's keyboard, or handed
    /// over by a page another device opens.
    Fetch,
    /// Verified boot's two checks in the slot the phone runs from, turned on
    /// or off with a swipe.
    Avb,
    /// Skyward's newest release, and updating to it: its build for the
    /// phone downloaded and written into the recovery partitions.
    SkywardUpdate,
    /// Skyward's own licence.
    License,
    /// The interface typeface and the terms it is used under.
    FontLicense,
    /// Which slot the phone starts from next.
    Slot,
    /// The installed system and the one Skyward installed beside it: which
    /// of them starts, switching between them, and taking the other away.
    Systems,
    /// The phone's own fastbootd serving the computer's fastboot, and where
    /// to go from it. Its only page, worked with the keys: the partitions
    /// the touchscreen runs from are left out while fastbootd rewrites
    /// super.
    Fastboot,
    /// Erasing what the system asked to erase before the recovery started,
    /// all user data or the cache alone: how far it has come, and where to
    /// go if it stops.
    Erase,
    /// The system could not start Android and asks whether to erase all
    /// user data, which may be what keeps it from starting.
    CantStart,
    /// Something the system asked of the recovery that it does not do.
    Unhandled,
    /// A new name for the entry the file page shows, typed on the screen's
    /// keyboard.
    Rename,
    /// The folder an entry is copied or moved to already holds that name;
    /// what to do is the user's to choose.
    Clash,
    /// Copying, moving or deleting under way, and how it ended.
    FileJob,
    /// The WLAN switch, the network chosen and how joining it goes, and
    /// the networks in range.
    Wlan,
    /// The password of the network chosen, typed on the screen's keyboard.
    WlanPassword,
    /// The network joined, and the address and the rest it gave.
    WlanNetwork,
    /// A network added by hand: its name, how it is protected and its
    /// password, typed on the screen's keyboard.
    WlanAdd,
    /// 任务：安装进行到哪一步、安装脚本最后说了什么、系统更新在下什么。
    Tasks,
    /// 分区管理：每个分区的名字、大小和挂载情况，同一条路上的导出与写回。
    Partitions,
    /// 关机：先说清楚手上没做完的活会被切断，再滑到底关机。
    Shutdown,
    /// 录屏：开始、停止，以及这一段写在哪。
    Record,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Reboot,
    Wipe,
    Flash,
    Backup,
    Restore,
    /// Start next from the slot chosen on the slot page.
    Slot,
    /// Erase all user data, as the system offered when it could not start.
    FactoryReset,
    /// Delete the entry the file page shows.
    DeleteEntry,
    /// Delete the backup the restore page shows.
    DeleteBackup,
    /// 卸载应用管理里选中的那个应用。
    DeleteApp,
    /// 卸载模块管理里选中的那个模块。
    DeleteModule,
    /// Keep the screen lock for the starts that follow, with the risk of it
    /// said first.
    KeepLock,
    /// Write the verified boot checks the avb page has turned.
    Avb,
    /// Install the update that waits to be agreed to.
    Install,
    /// Install an update that erases all user data after it.
    InstallWipe,
    /// Run the install script of the zip that waits to be agreed to.
    RunScript,
    /// Merge the update that waits in the slot the phone runs from.
    Merge,
    /// Give up an update that would not merge, so the slot starts from its
    /// own partitions again.
    GiveUp,
    /// Give the data partition's files the SELinux labels the installed
    /// system's rules give them.
    Relabel,
    /// Do to the partition the filesystem page shows what it chose.
    Filesystem,
    /// Switch to the system picked on the systems page, and restart into
    /// it.
    SwitchSystem,
    /// Take the other system away.
    RemoveSystem,
    /// Install the system the installer holds with the room chosen.
    InstallSystem,
    /// 写回一份 vbmeta 备份的滑动确认。
    Vbmeta,
    /// 关机页的滑动确认：切断电源，没做完的活就此停下。
    Shutdown,
}

/// What a control does when touched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Open(Screen),
    /// Change what the file manager sorts by, how, and whether folders stay on
    /// top.
    CycleSort,
    ToggleSortOrder,
    ToggleFoldersFirst,
    /// Turn one of verified boot's checks: `false` is dm-verity, `true` is
    /// the bootloader's own verification.
    Avb(bool),
    /// 备份当前槽的 vbmeta。
    VbmetaDump,
    /// 选中要写回的那一份。
    VbmetaPick(usize),
    /// Start the download the address typed is of, and stop the one under way.
    FetchStart,
    FetchStop,
    /// Take the address from a small file a computer put in Downloads.
    FetchFromFile,
    /// Start serving the folder the file manager shows, or stop.
    ServeToggle,
    /// Choose the folder served: the file manager's, which is opened for it,
    /// and this is the bar it is taken from.
    ServePick,
    ServeUseHere,
    /// Leave the decrypt page for the page it was opened from; the page
    /// the recovery started on, for the home page without keeping it
    /// behind.
    Skip,
    /// Try opening the data partition again, after it did not open.
    OpenData,
    Back,
    Theme { dark: bool },
    Toggle(Switch),
    RebootTarget(usize),
    /// Choose or leave out what the wipe page erases.
    WipeItem(Wipe),
    /// A slider's track: touching or dragging it sets the slider's level.
    Slider(Slider),
    /// Mount or unmount the mount page's partition at this index.
    MountToggle(usize),
    /// A swipe-to-confirm track; dragging the knob to the far end confirms.
    Confirm(Confirm),
    /// The pattern grid on the unlock page.
    Pattern,
    /// Start receiving a package over USB.
    Sideload,
    /// Send the image to one of the partitions it fits.
    FlashPartition(usize),
    /// Send the image to these slots.
    FlashSlots(Pick),
    /// Leave the recovery for this target at once.
    RebootTo(Target),
    /// Stop fastboot and bring the recovery's own pages up, without
    /// restarting.
    EnterRecovery,
    /// Leave what the system asked of the recovery for the recovery's own
    /// pages.
    Dismiss,
    /// Keep the screen on for this choice in [`SCREEN_OFF`].
    ScreenOff(usize),
    /// Keep the time zone at this place in [`TIME_ZONES`].
    TimeZone(usize),
    /// Speak this language.
    Language(Lang),
    /// 开始或停止录屏；正在录就停。
    Record,
    /// 换录音的那一档：分辨率、画质、帧率，点一下换下一档。
    RecordSetting(RecordSetting),
    /// Tick or untick a part of the backup, by its place in
    /// [`BACKUP_PARTS`].
    BackupPart(usize),
    /// Send the backup to this place.
    BackupTo(Place),
    /// Open a backup found, by its place in the list, to restore it.
    OpenBackup(usize),
    /// Tick or untick a part of the backup being restored.
    RestorePart(usize),
    /// Stop backing up, or checking a backup before it is restored.
    StopBackup,
    /// Tick or untick a log to export: the recovery's, the kernel's.
    LogPart(usize),
    /// Save the logs to the temporary folder, or to internal storage.
    LogsTo(usize),
    /// Save the logs ticked where they are to go.
    ExportLogs,
    /// Pick this slot to start next; a swipe then switches to it.
    PickSlot(Slot),
    /// Pick this system to start next; a swipe then switches to it.
    PickSystem(Who),
    /// Give the root of the system being installed the size at this place
    /// in those offered.
    SystemSize(usize),
    /// A key of the on-screen keyboard.
    Type(Cap),
    /// A key of the row above the keyboard, the terminal's or the editor's.
    TermKey(TermKey),
    /// Empty the search field.
    ClearSearch,
    /// Leave the search, by the round button beside its field.
    CloseSearch,
    /// Open the file manager's entry at this place in the folder: a folder
    /// to look in, a text file to edit, or what can be done with any other.
    OpenEntry(usize),
    /// Copy or move the entry the file page shows onto the OTG drive, or off
    /// it onto the owner's storage.
    ToDrive(FileOp),
    ToPhone(FileOp),
    /// Ask where a copy or a move goes: the phone's own storage or the drive.
    AskPlace(FileOp),
    /// Switch a module on or off, by its place in the list the page shows.
    ModuleToggle(usize),
    /// Mark the module at this place in the list to go at the next start, or
    /// take that mark off.
    ModuleRemove(usize),
    /// Install the zip the file page shows as a module.
    InstallAsModule,
    /// Open the module at this place in the list.
    ModuleOpen(usize),
    /// 模块管理翻一屏：负数往回。
    ModulePage(i32),
    /// 只读的系统应用页翻一屏。
    SysPage(i32),
    /// 打开系统应用页上的这一个：与用户应用一样看详情、导出安装包，
    /// 只是不能卸载（2026-11-19）。
    SysAppOpen(usize),
    /// 把系统应用的安装包导出到内部存储。
    SysAppExport(usize),
    /// 应用管理换个范围看：用户应用或系统应用。
    AppScope(AppsScope),
    /// 应用管理翻一屏：负数往回。
    AppPage(i32),
    /// 打开搜索页，这一次找的是应用。
    SearchApps,
    /// 打开搜索页，这一次找的是模块。
    SearchModules,
    /// Open the data app at this place in the list.
    DataAppOpen(usize),
    /// Take the data app at this place in the list off the phone.
    DataAppRemove(usize),
    /// Put the question away without doing anything.
    DismissPlace,
    /// Look at the files on the OTG drive.
    FilesOtg,
    /// Open the OTG entry at this place.
    OpenOtgEntry(usize),
    /// Open the terminal in the folder the file manager shows.
    TerminalHere,
    /// Open the entry at this place in a place's folder: a folder to look
    /// in, or the file to install.
    ChooseEntry(usize),
    /// Go up to the folder this many levels below the top of the list
    /// shown, by its place in the path shown over the list.
    OpenAncestor(usize),
    /// Pick the entry the file page shows to copy or move, and look for
    /// where it goes.
    PickEntry(FileOp),
    /// Copy or move the entry picked into the folder shown.
    Paste,
    /// Put the entry picked down again.
    Unpick,
    /// Give the entry the file page shows a new name.
    RenameEntry,
    /// Open the file the file page shows to edit it.
    EditEntry,
    /// Install the file the file page shows.
    InstallEntry,
    /// Write the file the editor page shows as it now stands.
    SaveText,
    /// Open the page of the permissions of the entry the file page shows.
    OpenPermissions,
    /// Turn this permission bit of the entry the file page shows on or off.
    ModeBit(u32),
    /// Ask to delete the entry the file page shows, or the backup the
    /// restore page shows; a swipe then confirms.
    AskDelete,
    /// Put the asking away, leaving the entry as it was.
    CancelDelete,
    /// Start choosing several entries of the folder shown, or stop.
    ChooseMany(bool),
    /// Tick or untick the entry at this place in the folder shown.
    ChooseEntryAt(usize),
    /// Tick or untick every entry of the folder shown.
    ChooseAll,
    /// Copy or move the entries chosen into the folder shown.
    PasteChosen(FileOp),
    /// Ask to delete the entries chosen; a swipe then confirms.
    AskDeleteChosen,
    /// How to go on when a name is taken where an entry goes.
    Resolve(ClashChoice),
    /// Stop giving the data partition's files their labels.
    StopRelabel,
    /// Open the page of the filesystem page's partition at this place.
    OpenFilesystem(usize),
    /// Choose what to do to the partition that page shows.
    ChooseFs(FsChoice),
    /// Stop copying, moving or deleting.
    StopJob,
    /// Join the network at this place in the list of those in range, or
    /// open the page of the one joined.
    JoinNetwork(usize),
    /// Open the details of the network at this place in the list of those
    /// in range.
    NetworkDetails(usize),
    /// The network chosen: its password again once refused, its details
    /// otherwise.
    OpenJoined,
    /// Leave the network joined, and forget it.
    LeaveNetwork,
    /// Look for the networks in range again.
    Rescan,
    /// Open the page that adds a network by hand.
    AddNetwork,
    /// Type into this field of the page that adds a network.
    TypeInto(Field),
    /// Open the protections a network added by hand can have.
    ChooseSecurity,
    /// Take this protection, and close the list.
    PickSecurity(Security),
    /// Close the list open over the page, choosing nothing.
    CloseChoices,
    /// Ask the update server for the newest system.
    CheckUpdate,
    /// Download the newest system's full package, or carry on downloading
    /// it.
    Download,
    StopDownload,
    /// Install the full package downloaded.
    InstallDownloaded,
    /// Ask GitHub for Skyward's newest release.
    CheckSkyward,
    /// Download the newest release's build for the phone and write it into
    /// the recovery partitions.
    UpdateSkyward,
    /// Stop downloading the build.
    StopSkywardUpdate,
    /// A control whose work is not wired up yet; touching it says so.
    Unavailable(&'static str),
}

/// One finger, in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    Down(f32, f32),
    Move(f32, f32),
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Power,
    VolumeUp,
    VolumeDown,
    Back,
}

/// Where a screenshot went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shot {
    /// Into the gallery's screenshots, in the owner's shared storage.
    Album,
    /// Into the recovery's own memory, to go into the gallery's once the
    /// owner's storage opens.
    Held,
}

/// Work the runtime carries out on the UI's behalf.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Reboot(Target),
    /// A short tap of the vibration motor.
    Haptic,
    /// The motor's two quick ticks as the power key turns the screen off,
    /// as strong as the phone's own system plays them whatever the
    /// vibration settings say.
    Ticks,
    /// Save what the panel shows as a picture: into the gallery's
    /// screenshots in the owner's shared storage where `storage` says it is
    /// open, and otherwise held until it is, telling how it went with
    /// [`App::screenshot_saved`].
    Screenshot { storage: bool },
    /// Offer internal storage to a computer over USB, or stop.
    FileTransfer(bool),
    /// Start receiving a package over USB.
    Sideload,
    /// Start installing the file at this path from the top of a place,
    /// telling how it goes as [`Effect::Sideload`] does.
    InstallFile(Place, String),
    /// Start installing the file at this full path, opened in the file
    /// manager, telling how it goes as [`Effect::Sideload`] does.
    InstallPath(String),
    /// Write the image the installer holds to `partition` in the slots
    /// `pick` means.
    Flash { partition: &'static str, pick: Pick },
    /// Stop waiting for the computer, for where an image goes, or for an
    /// update to be agreed to.
    Cancel,
    /// Read the slot's partitions for the mount page, which has just opened.
    ReadMounts,
    /// Read the modules installed for the phone's own system.
    ReadModules,
    /// Have the module in this folder switched on or off: the folder's name,
    /// and whether it is to be on.
    ModuleSwitch(String, bool),
    /// Have the module in this folder removed at the next start, or keep it:
    /// the folder's name, and whether it is to go.
    ModuleRemove(String, bool),
    /// Install, or install over, the module the zip at this path carries.
    ModuleInstall(String),
    /// Read the apps installed in the data partition.
    ReadDataApps,
    /// 读系统分区里的预装应用（只读页用）。
    ReadSystemApps,
    /// Take the app in this `/data/app` folder off, package and folder.
    DataAppRemove(String, String),
    /// Copy the system app's package at `from` into internal storage, under
    /// this name, telling how it went with [`App::report`].
    ExportApk { from: String, name: String },
    /// Read what the boot control holds of the slots, telling it with
    /// [`App::set_boot_slots`].
    ReadSlots,
    /// Have the bootloader start this slot next, telling how it went with
    /// [`App::slot_switched`].
    SetSlot(Slot),
    /// Read the systems for the systems page, telling them with
    /// [`App::set_systems`].
    ReadSystems,
    /// Write this system's boot set into the boot partitions and restart
    /// into it, telling of a switch that did not go with
    /// [`App::systems_work_failed`].
    SwitchSystem(Who),
    /// Take the other system away, telling how it went with
    /// [`App::set_systems`] or [`App::systems_work_failed`].
    RemoveSystem,
    /// Install the system the installer holds with a root of `size` bytes.
    InstallSystem { size: u64 },
    /// Mount the slot's partition with this base name, such as `system`.
    Mount(String),
    /// Unmount the slot's partition with this base name.
    Unmount(String),
    /// List the folder at this path for the file manager.
    ReadDir(String),
    /// List the folder at this path from the top of a place, to choose a
    /// file to install in, telling what it holds with [`App::set_place`].
    ReadPlace(Place, String),
    /// Open the data partition as far as it opens with nothing a user
    /// knows, telling what waits with [`App::data_waiting`] or why it did
    /// not open with [`App::data_failed`].
    OpenData,
    /// Open the own files of `user` with what they entered at their screen
    /// lock, telling what still waits with [`App::data_waiting`] or why it
    /// did not open with [`App::unlock_failed`].
    Unlock { user: u32, code: String },
    /// Hand the USB port back from fastboot to adb, bring up what the
    /// recovery runs from the slot's partitions, then show its pages with
    /// [`App::enter_recovery`].
    EnterRecovery,
    /// Erase all user data, as a factory reset does, telling how it goes
    /// with [`App::erase_at`] and [`App::erase_ended`].
    FactoryReset,
    /// Erase what the wipe page has chosen, [`Wipe::All`] as a factory
    /// reset does, telling how it goes with [`App::erase_at`],
    /// [`App::erase_progress`] and [`App::erase_ended`].
    Wipe(Vec<Wipe>),
    /// Install the update held for the user to agree to.
    Install,
    /// Forget what the system asked of the recovery, so that the next start
    /// goes where its reboot reason says.
    Dismiss,
    /// Start a shell for the terminal page in this folder, on a screen of
    /// the size [`App::term_size`] gives, telling of it with
    /// [`App::term_started`].
    StartShell(String),
    /// Have the terminal page's shell go to this folder if it waits for a
    /// command; one running a command is left to it, and [`App::report`]
    /// says so.
    ShellTo(String),
    /// Send these bytes to the shell, as typed.
    TermInput(Vec<u8>),
    /// Copy or move an entry into a folder; with no choice made yet, a name
    /// taken there is told with [`App::file_clash`] instead of going on.
    Paste { op: FileOp, path: String, into: String, clash: Option<ClashChoice> },
    /// Copy or move these, or delete them.
    PasteMany { op: FileOp, paths: Vec<String>, into: String, clash: Option<ClashChoice> },
    DeleteMany(Vec<String>),
    /// Read the folder at this path on the OTG drive.
    ReadOtg(String),
    /// Give an entry a new name; how it went comes back with
    /// [`App::renamed`].
    Rename { path: String, name: String },
    /// Give an entry these permission bits; how it went comes back with
    /// [`App::mode_set`].
    SetMode { path: String, mode: u32 },
    /// Read the file at this path to edit it, as much as
    /// [`editor::MAX_BYTES`] and one byte more, telling what it holds with
    /// [`App::text_opened`].
    OpenText(String),
    /// Write `text` as the contents of the file at `path`, which keeps what
    /// it has beside them; how it went comes back with
    /// [`App::text_saved`].
    SaveText { path: String, text: String },
    /// Delete an entry, folders whole.
    Delete(String),
    /// Stop the work on files under way.
    StopJob,
    /// Give the data partition's files the SELinux labels the installed
    /// system's rules give them, telling how it goes with
    /// [`App::relabel_started`], [`App::relabel_progress`] and
    /// [`App::relabel_ended`].
    Relabel,
    /// Stop giving the data partition's files their labels.
    StopRelabel,
    /// Read the partitions for the filesystem page, which has just opened,
    /// telling them with [`App::set_filesystems`].
    ReadFilesystems,
    /// Do `action` to the filesystem page's partition by its name, telling
    /// how it goes with [`App::part_work_started`], [`App::part_listed`],
    /// [`App::part_at`], [`App::part_ended`] and [`App::part_work_ended`];
    /// making the data partition afresh as another filesystem is a factory
    /// reset, which tells as erasing tells.
    Filesystem { name: String, action: FsAction },
    /// Save the logs chosen, the recovery's own and the kernel's, into a
    /// folder of their own in internal storage or the temporary folder,
    /// saying where with [`App::report`].
    ExportLogs { recovery: bool, kernel: bool, storage: bool },
    /// Back up the parts ticked, by their place in [`BACKUP_PARTS`], into
    /// a folder of their own in `place`, telling how it goes with
    /// [`App::backup_started`], [`App::backup_plan`], [`App::backup_at`]
    /// and [`App::backup_ended`].
    Backup { parts: [bool; 4], place: Place, compress: bool, verify: bool },
    /// Restore the parts ticked of the backup in the folder `name` of
    /// `place`, telling how it goes as a backup does.
    Restore { place: Place, name: String, parts: [bool; 4] },
    /// Look for backups in the places that can be read, telling what is
    /// found with [`App::set_backups`].
    FindBackups { storage: bool, usb: bool },
    /// Delete the backup in the folder `name` of `place`, telling how it
    /// went with [`App::backup_deleted`].
    DeleteBackup { place: Place, name: String },
    /// Stop the backup under way, or the check before a restore.
    StopBackup,
    /// Read where the update stands, telling it with [`App::set_update`].
    ReadUpdate,
    /// Turn the WLAN on or off; how it stands comes back with
    /// [`App::set_wlan`].
    /// Keep the screen lock that opens the user's files, or stop keeping it.
    Remember(bool),
    /// Serve the folder the file manager shows on the network, or stop.
    Serve(bool),
    /// Take `url` into Downloads, and stop the one under way.
    Fetch(String),
    StopFetch,
    /// Keep the file manager's order for the starts that follow.
    KeepSort(SortKey, bool, bool),
    /// Read verified boot's checks in the slot the phone runs from.
    AvbRead,
    /// Write both checks as the page has them.
    AvbSet(bool, bool),
    /// 把当前槽的 vbmeta 存一份到 `/sdcard/Skyward/vbmeta`。
    VbmetaDump,
    /// 列出存下来的每一份。
    VbmetaList,
    /// 把这一份写回去。
    VbmetaPutBack(String),
    Wlan(bool),
    /// Keep the WLAN open as the recovery starts, or stop keeping it so.
    WlanAuto(bool),
    /// Keep the password of the network joined last, or forget it.
    WlanKeep(bool),
    ScanWlan,
    JoinWlan { ssid: Vec<u8>, name: String, security: Security, password: Secret },
    LeaveWlan,
    /// Ask the update server for the newest system, telling what it
    /// answered with [`App::update_checked`].
    CheckUpdate,
    /// Download `release`'s full package into [`DOWNLOADS`], telling how it
    /// goes with [`App::download_progress`] and [`App::download_ended`].
    Download(Release),
    StopDownload,
    /// Ask GitHub for Skyward's newest release, telling what it answered
    /// with [`App::skyward_checked`].
    CheckSkyward,
    /// Download `build` and write it into the recovery partitions, telling
    /// how it goes with [`App::skyward_progress`] and
    /// [`App::skyward_ended`].
    UpdateSkyward(Build),
    StopSkywardUpdate,
    /// 开始录屏：文件和目录由拿到它的那一层建好。
    RecordStart,
    /// 停止录屏，把文件收好。
    RecordStop,
    /// 暂停或继续这一段：胶囊上的红点按一下。
    RecordPause(bool),
    /// Merge the update that waits in the slot the phone runs from,
    /// telling how it goes with [`App::set_update`].
    Merge,
    /// Give up an update that would not merge, so the slot starts from its
    /// own partitions again, telling how it goes with
    /// [`App::update_given_up`].
    GiveUp,
}

enum Drag {
    Idle,
    /// Finger down and not yet past the touch slop. `edge` is set when the
    /// press began at a screen edge (true for the left one).
    /// `at` is when it came down: held long enough over an entry of the file
    /// manager, it opens what can be done with that entry.
    Press { x: f32, y: f32, action: Option<Action>, edge: Option<bool>, at: u64 },
    /// A press held long enough to have done its work; nothing it does
    /// counts until it lifts.
    Held,
    Scroll { last: f32 },
    /// Swiping in from a screen edge, the finger now `x` across; the cue
    /// keeps to height `cy`, set by where the swipe began. Past the
    /// threshold, lifting goes back.
    Edge { start: f32, from_left: bool, armed: bool, x: f32, cy: f32 },
    Slider(Slider, Rect),
    /// Pulling the control centre: where it began, and how far out it was then.
    Shade { start: f32, from: f32 },
    Confirm { which: Confirm, track: Rect, grab: f32 },
    /// Scrolling a terminal's screen back through what the shell wrote, or
    /// the editor's text, `carry` being the part of a row not yet scrolled.
    History { last: f32, carry: f32, pitch: f32 },
    Pattern,
    /// A finger that landed just after the panel lit; nothing it does
    /// counts until it lifts.
    Waking,
    /// 拖着录屏的计时胶囊走：`grab` 是按下时手指在胶囊里的位置，`moved` 记
    /// 这一次是不是真的挪过（挪过就不算按了按钮），`dot` 是按下时落在暂停的
    /// 那一下上，`stop` 是落在右边的结束方块上。
    RecordPill { grab: (f32, f32), moved: bool, dot: bool, stop: bool },
}

/// 录屏那枚胶囊上一帧画在哪儿：整枚、左边暂停/继续那一下、右边结束那一下。
/// 手指按下时按这几个矩形认，拖动只认整枚。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pill {
    pub body: Rect,
    pub dot: Rect,
    pub stop: Rect,
}

/// Where the keys are on a page.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Focus {
    /// Nowhere yet: the first control in view lights once the keys work
    /// the page.
    Unset,
    /// On no control, none showing where the keys scrolled the page.
    Off,
    /// On this control, as it was when the page was last drawn.
    On(Spot),
}

/// A button held down, and what holding it does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Press {
    /// A volume key, moving the light down the page or up it, and again at
    /// `next`.
    Volume { down: bool, next: u64 },
    /// The power key on the control lit, from `at`, which it works as it
    /// comes up; held long enough over an entry of the file manager, it
    /// opens what can be done with the entry, as a long press does.
    Power { spot: Spot, at: u64 },
    /// The power key on a swipe-to-confirm control, whose knob slides along
    /// its track at `track` from `at` while the key is held; let go once
    /// it has slid far enough, the control confirms.
    Slide { which: Confirm, track: Rect, at: u64 },
    /// Done with: nothing more comes of it until it is let go.
    Spent,
}

const TOAST_MS: u64 = 2000;
/// How long a text field's cursor shows and then hides, as the stock
/// fields blink it.
const BLINK_MS: u64 = 500;
/// The spring the home page's search bar moves to the search page's field
/// on, and back: critically damped, fitted to frames of the stock settings
/// on a Xiaomi 17 Ultra.
const MORPH_PERIOD: f32 = 0.292;
/// Along the move, from 0 at the home page to 1 at the search page: where
/// the large title has faded away, and the stretch over which the button
/// leaving the search comes in.
const MORPH_TITLE_GONE: f32 = 0.6;
const MORPH_CANCEL: (f32, f32) = (0.45, 0.95);
/// How blurred that button is as it starts to show, in dp.
const MORPH_CANCEL_BLUR_DP: f32 = 8.0;
/// When what the search page shows under its field comes in after the move
/// starts, and how long it takes, in ms.
const MORPH_BODY_MS: (u64, u64) = (380, 120);
/// A row a finger lands on starts darkening this long after, as the stock
/// settings' rows do; a finger moving on before then scrolls the page, and
/// the row never shows pressed.
const ROW_PRESS_DELAY_MS: u64 = 90;
/// The spring a row held down darkens on, and fades back on once let go:
/// critically damped, fitted to frames of the stock settings on a Xiaomi
/// 17 Ultra.
const ROW_PRESS_PERIOD: f32 = 0.18;
/// A row let go before it showed pressed shows pressed from the lift this
/// long, as Android shows a quick tap.
const QUICK_TAP_MS: u64 = 64;
/// A page opening slides in on a critically damped spring of this period,
/// set off already moving this fast, in screen widths a second; a page
/// going back slides off on a spring of this period, from rest. Both fitted
/// to frames of the stock settings on a Xiaomi 17 Ultra.
const OPEN_PERIOD: f32 = 0.37;
const OPEN_SPEED: f32 = 2.94;
const BACK_PERIOD: f32 = 0.393;
/// While a page in front covers it wholly, the page behind stands this far
/// left, in screen widths, darkened this much.
const BEHIND_SHIFT: f32 = 0.25;
const BEHIND_DIM: f32 = 0.47;
/// The most lines of what an install script shows that its page keeps,
/// the latest; the log keeps them all.
const SCRIPT_LINES: usize = 500;
/// The lines under the bar while a package is written that the page keeps:
/// what update_engine is doing now, and the three things before it.
const INSTALL_LINES: usize = 4;
/// How long a finger rests on an entry before it counts as held, as the
/// system's long press does.
const LONG_PRESS_MS: u64 = 400;
/// A finger lifted slower than this, in dp per second, leaves the page
/// where it is.
const MIN_FLING_DP: f32 = 50.0;
/// The fastest a page starts gliding, in dp per second.
const MAX_FLING_DP: f32 = 8000.0;
/// A glide running into a page's end faster than this, in dp per second,
/// swings out on the slower spring.
const HARD_BOUNCE_DP: f32 = 1667.0;
/// Scroll, in dp, over which a large title fades out.
const TITLE_FADE_DP: f32 = 20.0;
/// Blur, in dp, of a title faded out completely.
const TITLE_BLUR_DP: f32 = 25.0 / 3.0;
/// A search bar folding away: its icon and words fade out over the first
/// this many dp of the fold, then the bar over as many more, gone however
/// faint once its foot reaches its top. A page let go with the bar folded
/// this share of the way or further settles folded, and unfolded short of
/// it, on a critically damped spring of this period set off from rest.
/// Fitted to frames of the stock settings on a Xiaomi 17 Ultra.
const FOLD_FADE_DP: f32 = 26.0;
const FOLD_SETTLE: f32 = 0.5;
const FOLD_PERIOD: f32 = 0.354;
/// How far, in dp, a swipe from an edge has to come in to go back.
const BACK_SWIPE_DP: f32 = 56.0;
/// The back cue sits this far, in dp, above where the finger came down,
/// and stays at that height however the finger then wanders.
const CUE_LIFT_DP: f32 = 9.0;
/// On the About page, where the wordmark's area ends, in dp from the top:
/// the version has faded out by the time it passes under the bar.
const ABOUT_LOGO_DP: f32 = 160.0;
/// Scroll, in dp, over which the wordmark then shrinks and fades.
const ABOUT_LOGO_FADE_DP: f32 = 40.0;
/// Scroll, in dp, over which the About page's field fades out; the field's
/// discs sit in the band this far below the bar.
const ABOUT_FIELD_DP: f32 = 389.0;
/// The field is worked out every this many pixels and filled in between.
const FIELD_CELL: usize = 24;
/// Before the panel goes off it stays dim this long, in milliseconds, or
/// a fifth of the screen-off time when that is shorter.
const DIM_MS: u64 = 7000;
/// The dim level is this much below the brightness set, no brighter than
/// the ceiling, and never under the slider's floor unless the brightness
/// set is.
const DIM_DROP: f32 = 0.04;
const DIM_CEILING: f32 = 0.05;
/// The lowest brightness the slider sets.
const MIN_BRIGHTNESS: f32 = 0.02;
/// How much of a slider's span a volume key moves it.
const SLIDER_STEP: f32 = 0.1;
/// Where both sliders start: one step of the keys from their left end.
pub const SLIDER_START: f32 = SLIDER_STEP;
/// While the backlight eases, it steps once a frame of a 120 Hz panel.
const BACKLIGHT_STEP_MS: u64 = 8;
/// Just after the panel lights, a touch or key only keeps it awake: the
/// second tap of a wake habit, or the hand reaching for a phone whose
/// screen an install just lit, should not act. Android's double-tap window.
const WAKE_GUARD_MS: u64 = 400;
/// A volume key held this long moves the light again, then once every
/// [`REPEAT_MS`], as the system repeats a key held.
const REPEAT_DELAY_MS: u64 = 400;
const REPEAT_MS: u64 = 50;
/// The power key and volume down pressed within this long of each other
/// take a screenshot, as the system's key combination does; the first of
/// the two waits this long, or until it is let go, before it does what it
/// does alone.
const CHORD_MS: u64 = 150;

/// How long the ring about the sign on the decrypt page takes to turn once.
const RING_MS: u64 = 1500;

/// How often the page showing an install redraws while one is writing, so
/// that the bar under it moves rather than jumping (2026-11-20).
const INSTALL_FRAME_MS: u64 = 100;

/// How much of the bar on the decrypt page a millisecond of waiting puts in: it
/// runs its whole length in about two and a half seconds -- about as long as
/// the phone's own parts take to come up -- and waits there, full, until the
/// opening is done. What it says is how long the page has been at the work, not
/// how much of the work is behind, so a page that is waiting is never shown
/// standing still.
///
/// It was 0.00017, which filled the bar in about six seconds, for parts that
/// took 5.5 s to come up. The directory was made to start again by hand rather
/// than through init's restart, which took 3.6 s out of that, and the bar was
/// left filling for a wait that no longer existed: it was a little over a third
/// of the way along when the page went (2026-11-15).
/// How far along its track the knob of a swipe-to-confirm control has to
/// be let go to confirm.
const SLID: f32 = 0.9;
/// How long the power key held takes the knob the whole way along.
const SLIDE_MS: u64 = 700;
/// The response, in seconds, of the spring the page scrolls on to show
/// the control the keys moved to.
const SEEK_PERIOD: f32 = 0.35;
/// How strongly the light tints the control the keys are on, and the one
/// the power key is held down on.
const LIGHT_ALPHA: u8 = 36;
const LIGHT_HELD_ALPHA: u8 = 72;
/// How far past the screen, in screens, a long list keeps its rows drawn.
const NEAR_SCREENS: f32 = 0.5;

/// How the screen's content goes dark as the panel goes off, and comes up
/// as it lights, as the phone's own system moves it: to black in 365 ms
/// slowing as the cube of the time left, from black over 520 ms slowing as
/// its square.
const DARKEN: Ease = Ease::Out(365, 3);
const BRIGHTEN: Ease = Ease::Out(520, 2);
/// The charging screen comes over the page as the phone's own does, over
/// 420 ms slowing as the square of the time left, shows this long from
/// when the cable went in, and goes in 300 ms.
const CHARGE_SHOW: Ease = Ease::Out(420, 2);
const CHARGE_HIDE: Ease = Ease::Out(300, 2);
const CHARGE_MS: u64 = 4000;

/// How a part of the screen eases between shown and hidden.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ease {
    /// A critically damped spring of this period, in seconds.
    Spring(f32),
    /// A straight run over this many milliseconds.
    Linear(u64),
    /// A run over this many milliseconds that slows to a stop, what is
    /// left of the way going as this power of the time left.
    Out(u64, i32),
}

impl Ease {
    fn progress(self, ms: u64) -> f32 {
        match self {
            Ease::Spring(period) => motion::settle(ms, period),
            Ease::Linear(span) => (ms as f32 / span as f32).min(1.0),
            Ease::Out(span, power) => 1.0 - (1.0 - (ms as f32 / span as f32).min(1.0)).powi(power),
        }
    }
}

/// The charging screen: when the cable went in, how far it shows over the
/// page, and whether the panel was dark then and goes dark again after.
struct Charging {
    since: u64,
    shown: Fade,
    dark_after: bool,
}

/// A part of the screen fading in or out: where it is headed, since when,
/// how far in it was then, and how it eases each way.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    on: bool,
    since: u64,
    from: f32,
    show: Ease,
    hide: Ease,
}

impl Fade {
    fn hidden(show: Ease, hide: Ease) -> Fade {
        Fade { on: false, since: 0, from: 0.0, show, hide }
    }

    fn shown(show: Ease, hide: Ease) -> Fade {
        Fade { on: true, since: 0, from: 1.0, show, hide }
    }

    /// How far in, 0 to 1, at `now`.
    fn level(&self, now: u64) -> f32 {
        let (target, ease) = if self.on { (1.0, self.show) } else { (0.0, self.hide) };
        self.from + (target - self.from) * ease.progress(now.saturating_sub(self.since))
    }

    fn set(&mut self, on: bool, now: u64) {
        if on != self.on {
            self.from = self.level(now);
            self.since = now;
            self.on = on;
        }
    }

    /// Stand fully in or out at once.
    fn jump(&mut self, on: bool) {
        self.on = on;
        self.from = if on { 1.0 } else { 0.0 };
    }

    fn moving(&self, now: u64) -> bool {
        self.level(now) != if self.on { 1.0 } else { 0.0 }
    }
}

/// How the bar's title comes and goes: sharpening in on a spring and
/// blurring away on a quicker one, or on the About page, fading in a
/// straight line together with the bar.
fn title_fade(hero: bool) -> Fade {
    if hero {
        Fade::hidden(Ease::Linear(150), Ease::Linear(150))
    } else {
        Fade::hidden(Ease::Spring(0.3), Ease::Spring(0.15))
    }
}

fn bar_fade(hero: bool) -> Fade {
    if hero {
        Fade::hidden(Ease::Linear(150), Ease::Linear(150))
    } else {
        Fade::hidden(Ease::Linear(100), Ease::Linear(100))
    }
}

/// The panel at full brightness: it dims over half a second and comes
/// back quickly when touched.
fn full_brightness() -> Fade {
    Fade::shown(Ease::Linear(150), Ease::Linear(500))
}

/// The level the panel dims to shortly before it goes off, for the
/// brightness set.
fn dim_level(full: f32) -> f32 {
    (full - DIM_DROP).clamp(MIN_BRIGHTNESS, DIM_CEILING).min(full)
}

/// How long the panel stays dim before going off, for a screen-off time.
fn dim_span(after: u64) -> u64 {
    DIM_MS.min(after / 5)
}

/// Where a pinned strip shows on a page scrolled to `offset`: with the page,
/// or just under the bar once the page has carried it that far.
fn pinned_top(t: &Theme, p: &Pinned, offset: f32) -> f32 {
    (p.top - offset).max(widgets::bar_bottom(t))
}

/// Whether `r` reaches into the band of the screen from `top` to `bottom`.
fn within(r: Rect, (top, bottom): (f32, f32)) -> bool {
    r.y < bottom && r.y + r.h > top
}

/// Lay the sprite `blurs` keeps, moved by (`dx`, `dy`), in `color` at
/// `level` of its strength, blurred by `blur` pixels.
fn draw_faded(out: &mut Canvas, blurs: &mut Blurs, dx: f32, dy: f32, color: Rgba, level: f32, blur: f32) {
    if level > 0.0 {
        blurs.get(blur).draw(out, dx, dy, color, level);
    }
}

/// The home page's search bar moving up to become the search page's
/// field, or back down, as the stock settings move it: the home page's
/// content goes at once on the way up and comes along under the bar on
/// the way down; the large title fades and blurs as it goes up with the
/// bar; the button leaving the search fades and sharpens in; the keyboard
/// rises with the bar; and what the search page shows under its field
/// comes in last.
struct Morph {
    /// Up to the search page.
    into: bool,
    /// How far along it is, from 0 at the home page to 1 at the search
    /// page, as a spring in thousandths settling on the end it goes to.
    spring: Spring,
    start: u64,
    /// How far the home page was scrolled, where its bar stood.
    scroll: f32,
    /// The search page's keyboard, and where it rests.
    keyboard: Option<(Canvas, i32)>,
    /// What the search page shows between its field and the keyboard, and
    /// where.
    body: Option<(Canvas, i32)>,
    /// The home page's large title, and the button's disc and cross.
    title: Option<Blurs>,
    cancel: Option<(Blurs, Blurs)>,
}

impl Morph {
    /// How far along it is at `now`, and whether it has settled.
    fn at(&self, now: u64) -> (f32, bool) {
        let (x, done) = self.spring.at(now);
        (if self.into { 1.0 } else { 0.0 } + x / 1000.0, done)
    }
}

/// A row of the page under a finger, darkening behind its words and icon:
/// where it lies on the page, when it starts darkening, and when it starts
/// fading back.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RowPress {
    rect: Rect,
    rise: u64,
    fall: Option<u64>,
}

impl RowPress {
    /// How dark the row is at `now`, from 0 to 1.
    fn level(&self, now: u64) -> f32 {
        if now < self.rise {
            return 0.0;
        }
        let up = |t: u64| motion::settle(t - self.rise, ROW_PRESS_PERIOD);
        match self.fall {
            Some(fall) if now >= fall => up(fall) * (1.0 - motion::settle(now - fall, ROW_PRESS_PERIOD)),
            _ => up(now),
        }
    }

    /// Whether it has faded back after being let go.
    fn gone(&self, now: u64) -> bool {
        self.fall.is_some_and(|fall| now >= fall && motion::settle(now - fall, ROW_PRESS_PERIOD) == 1.0)
    }

    /// Whether it is darkening or fading back at `now`.
    fn changing(&self, now: u64) -> bool {
        let dark = self.fall.is_none() && now >= self.rise && motion::settle(now - self.rise, ROW_PRESS_PERIOD) == 1.0;
        now >= self.rise && !dark && !self.gone(now)
    }

    /// The finger lifted at `now`: a row that had not shown pressed yet
    /// shows it a moment.
    fn lift(&mut self, now: u64) {
        if now < self.rise {
            self.rise = now;
            self.fall = Some(now + QUICK_TAP_MS);
        } else {
            self.fall = Some(now);
        }
    }
}

/// How a page showed on a frame: how far scrolled, and how far past an end
/// with that; how far its bar and the bar's title had come in; where the
/// keys' light was; whether the cursor showed; how dark the row a finger
/// held was; and the button a finger held.
#[derive(Clone, Copy, Debug, PartialEq)]
struct View {
    scroll: f32,
    offset: f32,
    bar: f32,
    title: f32,
    light: Option<(Group, Rect, f32, Rgba)>,
    caret: bool,
    row: f32,
    pressing: Option<Action>,
}

/// A page sliding over another as it opens or goes back, as the stock
/// settings slide them. A page opened comes in from the right edge over
/// the page it opens from, which shifts a quarter of the way left and
/// darkens; going back, the page left slides off to the right over the
/// page gone back to, which comes back from the left and brightens. The
/// page in front has its left corners rounded as the screen's are.
struct Turn {
    /// Going back: the page in front is the page left; else the page
    /// opened.
    back: bool,
    /// The page left, as it last showed.
    left: Canvas,
    /// How far from the left edge the page in front stands, in thousandths
    /// of the screen's width, as a spring settling where it goes; set off
    /// on the first frame of the slide.
    spring: Option<Spring>,
}

/// Whether a control is a row: as wide as the cards it lies on.
fn row_wide(t: &Theme, r: Rect) -> bool {
    let m = &t.m;
    (r.x - m.margin).abs() < 0.5 && (r.w - (m.w as f32 - m.margin * 2.0)).abs() < 0.5
}

/// The whole pixels of the page's `canvas` a held row's `rect` covers:
/// left, top, width and height.
fn row_part(r: Rect, canvas: &Canvas) -> (u32, u32, u32, u32) {
    let (x0, y0) = (r.x.floor().max(0.0) as u32, r.y.floor().max(0.0) as u32);
    let (x1, y1) = (((r.x + r.w).ceil() as u32).min(canvas.w), ((r.y + r.h).ceil() as u32).min(canvas.h));
    (x0, y0, x1 - x0, y1 - y0)
}

/// The keyboard at the foot of a typing page's `frame`, and where it rests.
fn keyboard_strip(t: &Theme, frame: &Frame) -> (Canvas, i32) {
    let top = (t.m.h as f32 - widgets::keyboard_height(t)).floor() as i32;
    let mut strip = Canvas::new(frame.canvas.w, (frame.canvas.h as i32 - top).max(0) as u32);
    strip.put_rows(&frame.canvas, top, frame.canvas.h as i32, 0);
    (strip, top)
}

/// How far the control centre has to be pulled, in dp, for it to stay open
/// once let go, and how long its settling takes.
const SHADE_PULL_DP: f32 = 110.0;
const SHADE_PERIOD: f32 = 1.8;
/// The share of the pull past which letting go opens the sheet.
const SHADE_KEEPS: f32 = 0.36;
/// How tall the control centre's sheet is, in dp, at full pull: the row the
/// time and the date share, the device's card, the twelve round tiles in
/// three rows of four, the two capsules, a little air under them, and the
/// rounded foot with the grabber.
const SHADE_HEIGHT_DP: f32 = 566.0;

/// What a finger can press in the control centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShadeHit {
    Wlan,
    Screenshot,
    Reboot,
    /// 录屏，替掉原来的熄屏磁贴：熄屏还有电源键。
    Record,
    /// 语言在三语之间轮转：简中 → 繁中 → English。
    Language,
    Files,
    Terminal,
    Mount,
    Update,
    Backup,
    Settings,
    Shutdown,
    Brightness,
    Vibration,
}

/// What a locked screen holds: when the panel went dark, and how far the knob
/// has been slid along.
struct Lock {
    since: u64,
    drag: f32,
    /// Whether the finger went down on the knob rather than elsewhere.
    held: bool,
}

/// How much of the track the knob must reach for the lock to give way.
const UNLOCK_AT: f32 = 0.96;

/// How long the power key is held on the lock for the reboot page to open.
const POWER_HOLD_MS: u64 = 1200;

/// How close together two taps on the dark panel have to be for it to light.
const DOUBLE_TAP_MS: u64 = 600;

/// What the decrypt page does once the bar on it has run its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataAfter {
    /// The partition is open: the page says so, with the way on to the menu.
    Open,
    /// The partition is open and the page is done with: the menu shows.
    Home,
}

/// How long the bar's last length may take before the page does what it is
/// for whether or not the bar got there: a page left waiting on a bar is a
/// page that never gets in.
const LAST_LEN_MS: u64 = 700;

pub struct App {
    /// The two answers the place question offers, where they were drawn.
    pub(super) place_hits: Vec<(crate::geom::Rect, Action)>,
    pub state: State,
    /// The pages open, the one shown last: each with how far it is
    /// scrolled and where the keys are on it.
    stack: Vec<(Screen, f32, Focus)>,
    /// The lock over the page once the panel went dark, until its knob is slid
    /// along; and when the power key went down on it.
    lock: Option<Lock>,
    lock_press: Option<u64>,
    /// Where the lock's knob was last drawn, which is where a finger finds it,
    /// and when the dark panel was last tapped.
    lock_rect: Option<Rect>,
    tap_at: Option<u64>,
    /// The control centre: how far it is pulled out, the pull it settles from,
    /// the finger that started it, and what can be pressed on it.
    /// The card asking to delete, and the swipe on it, as last drawn: a finger
    /// finds them where they are, not where the page has scrolled to.
    asking: Option<(Rect, Confirm)>,
    asking_track: Option<Rect>,
    /// 录屏的计时胶囊、左边暂停/继续那一下、右边结束那一下，按上一帧画出来
    /// 的位置：手指按下去要找的是画在屏幕上的那一枚，不是再算一遍。
    record_pill: Option<Pill>,
    shade: f32,
    /// Where a finger that came down near the top edge started: it becomes a
    /// pull only once it moves down, so a tap up there still reaches the page.
    shade_from: Option<f32>,
    shade_settle: Option<(u64, f32, f32, f32, f32)>,
    shade_last: Option<(u64, f32)>,
    shade_speed: f32,
    shade_press: Option<(ShadeHit, f32)>,
    shade_hits: Vec<(Rect, ShadeHit)>,
    /// When to take the screenshot a tile asked for, once the page under the
    /// sheet has been drawn without it.
    shot_when: Option<u64>,
    frame: Option<Frame>,
    /// The controls of the page drawn, in the order the keys go through
    /// them.
    spots: Option<Vec<Spot>>,
    /// The keys are working the pages: the control they are on is lit. A
    /// touch puts the light out, and a volume key brings it back.
    keys: bool,
    /// The button held down, and what holding it does.
    button: Option<(Key, Press)>,
    /// The first of the power key and volume down gone down, waiting for
    /// the other to take a screenshot, and when it went down.
    chord: Option<(Key, u64)>,
    /// The page scrolling of its own accord, to show the control the keys
    /// moved to or to fold its search bar whole or not at all: where it
    /// comes to rest, and the spring taking it there.
    seek: Option<(f32, Spring)>,
    /// A swipe-to-confirm control whose knob the power key moved, to draw
    /// again with the page.
    knob: Option<(Confirm, Rect)>,
    /// The slider lit takes the volume keys, until the power key lets it
    /// go.
    adjusting: bool,
    /// How tall the page was when last drawn: one being drawn again, as
    /// work under way tells how far it has come, scrolls as far meanwhile.
    height: f32,
    /// The rows of the page's long list drawn so far, by their place in it;
    /// those far off screen are let go, their canvases kept spare for the
    /// next rows to be drawn on.
    drawn: Vec<(usize, Canvas)>,
    spare: Vec<Canvas>,
    /// The current page's large and bar titles, for their words and the
    /// theme, with their blurred versions as they are needed. A page can
    /// change its title in place, as work on files does once it ends.
    titles: Option<((&'static str, bool), Blurs, Blurs)>,
    /// The icon and words of the current page's folding search bar, for
    /// its words and the theme.
    search_words: Option<((&'static str, bool), Canvas)>,
    /// The `<` chip for the theme (dark or not), and where it goes.
    chip: Option<(bool, Canvas, i32, i32)>,
    /// The chip behind the round button at the right end of the bar, as
    /// for `<`.
    button_chip: Option<(bool, Canvas, i32, i32)>,
    /// The folder the terminal page's shell last started in, where Enter
    /// starts it again once it has ended.
    shell_folder: String,
    /// When leaving the editor with its changes unsaved was last warned
    /// of: going back again while the warning shows leaves without them.
    unsaved_warned: Option<u64>,
    /// The About page field's dither, made for the screen once.
    dither: Option<glow::Dither>,
    toast: Option<(String, u64)>,
    drag: Drag,
    /// The step of the strength slider the motor was last played at while a
    /// finger drags it: a step crossed is a tap, so that the strength is felt
    /// as it is set rather than only once the finger is lifted (2026-11-19).
    vibe_step: i32,
    tracker: Tracker,
    /// The page gliding on after a flick.
    fling: Option<Fling>,
    /// A terminal's screen or the editor's text gliding on after a flick:
    /// the glide, a row's height, and how many rows it has scrolled so far.
    glide: Option<(Fling, f32, isize)>,
    /// How far a dragging finger has pulled the page past an end:
    /// negative past the top, positive past the bottom.
    pull: f32,
    /// When the decrypt page started waiting for the partition: the bar's
    /// sweep runs from here, and a fresh opening starts it afresh rather
    /// than carrying on from the last one.
    data_started: Option<u64>,
    /// When the bar on that page last moved, from which it steps on.
    data_moved: u64,
    /// The bytes the install last reported written, and when: what comes
    /// after goes at the pace between the two.
    install_seen: (u64, u64),
    /// The partition is open and the bar is running its last length: the page
    /// stays until the bar is full, then does what is asked of it, or it would
    /// leave with the bar part way.
    data_finish: Option<DataAfter>,
    /// When that last length began: a bar that stops moving for any reason
    /// still leaves the page in the end, so that the page can never be left
    /// standing on a partition that is already open.
    data_finish_at: Option<u64>,
    /// Where the bar stood when that last length began: the length is run
    /// from there, so that a bar part way up fills as the page leaves
    /// instead of arriving at the end in a jump (2026-11-19).
    data_finish_from: f32,
    /// The user whose screen lock is being tried behind the lock kept from
    /// before, and the control that lock is answered with: put back where the
    /// attempt fails, so the page asks for it again rather than waiting on a
    /// job that has ended.
    data_back: Option<(u32, Keys)>,
    /// The page springing back after being pulled or thrown past an end.
    bounce: Option<Spring>,
    /// The back cue shrinking away after a swipe that did not go back:
    /// its edge, where it sits, and its depth.
    cue: Option<(bool, f32, Spring)>,
    title: Fade,
    bar: Fade,
    /// A page was put up, or the file manager's folder changed: once it is
    /// rendered, the bar and its title stand as its scroll has them,
    /// without easing in or out.
    snap: bool,
    /// The About page's moving field, while that page shows.
    glow: Option<GlowClock>,
    /// The terminal page's shell, from when it is first opened; it goes on
    /// while other pages show.
    term: Option<Term>,
    display_on: bool,
    /// When the screen was last touched or a key pressed; set on the first
    /// tick, so the time before the UI started does not count.
    active: Option<u64>,
    /// When the panel last lit from dark, for the brief guard that keeps a
    /// stray touch or key from acting on a page just put up.
    woke_at: Option<u64>,
    /// The panel at full brightness, or easing to the dim level it takes
    /// shortly before going off.
    bright: Fade,
    /// The screen's content against black: it goes dark as the panel goes
    /// off, under the backlight held then, and comes up as the panel lights.
    curtain: Fade,
    darkening: Option<f32>,
    /// The charging screen, from when a cable goes in until it has gone,
    /// with the level and what the charger does drawn for it, and the
    /// screen it is drawn on while it comes or goes over the page.
    charging: Option<Charging>,
    charge_art: Option<charge::Art>,
    charge_canvas: Option<Canvas>,
    /// While a text field's cursor is up: the page, how many edits had
    /// been typed, and when it came up whole on them. It shows for the
    /// first half of each blink from then on, so it is whole again at each
    /// edit and on each page it comes up on.
    blink: Option<(Screen, u64, u64)>,
    /// Edits typed into any text field so far.
    edits: u64,
    /// Whether the frame last composed showed the cursor.
    caret_shown: bool,
    /// The home page's search bar on its way to the search page's field,
    /// or back.
    morph: Option<Morph>,
    /// The row a finger holds down, or let go of while it fades back, and
    /// the part of the page it lies on as drawn and as drawn held down.
    row_press: Option<RowPress>,
    row_art: Option<(Canvas, Canvas)>,
    /// How the page showed on the last frame, while it can be drawn so
    /// again: until it is rendered anew.
    shown: Option<View>,
    /// A page sliding in over the page left, or off over the page gone
    /// back to.
    turn: Option<Turn>,
    /// While a page slides: the page shown drawn on its own, and how it
    /// showed, to slide it again as it was while it shows the same.
    layer: Option<(View, Canvas)>,
    /// Bands of rows of the page drawn that show what has changed since,
    /// to draw again.
    stale: Vec<(u32, u32)>,
    /// How many columns from the left edge of the page's rows drawn are
    /// drawn; a page opened is drawn across as it slides in.
    cols: u32,
    /// The canvas the last page left was drawn on, to draw the next on.
    backdrop: Option<Canvas>,
    /// How round the screen's corners are, in pixels.
    corners: f32,
    dirty: bool,
    w: u32,
    h: u32,
    px_per_dp: f32,
}

impl App {
    pub fn new(start: Screen, w: u32, h: u32, px_per_dp: f32) -> Self {
        App {
            state: State::default(),
            stack: vec![(start, 0.0, Focus::Unset)],
            lock: None,
            lock_press: None,
            lock_rect: None,
            tap_at: None,
            asking: None,
            asking_track: None,
            record_pill: None,
            place_hits: Vec::new(),
            shade: 0.0,
            shade_from: None,
            shade_settle: None,
            shade_last: None,
            shade_speed: 0.0,
            shade_press: None,
            shade_hits: Vec::new(),
            shot_when: None,
            frame: None,
            spots: None,
            // The touchscreen may run from the partitions fastboot leaves
            // out, so its page starts worked with the keys.
            keys: start == Screen::Fastboot,
            button: None,
            chord: None,
            seek: None,
            knob: None,
            adjusting: false,
            height: 0.0,
            drawn: Vec::new(),
            spare: Vec::new(),
            titles: None,
            search_words: None,
            chip: None,
            button_chip: None,
            shell_folder: "/".into(),
            unsaved_warned: None,
            dither: None,
            toast: None,
            drag: Drag::Idle,
            vibe_step: -1,
            tracker: Tracker::default(),
            fling: None,
            glide: None,
            pull: 0.0,
            data_started: None,
            data_moved: 0,
            install_seen: (0, 0),
            data_finish: None,
            data_finish_at: None,
            data_finish_from: 0.0,
            data_back: None,
            bounce: None,
            cue: None,
            title: title_fade(start == Screen::About),
            bar: bar_fade(start == Screen::About),
            snap: false,
            glow: None,
            term: None,
            display_on: true,
            active: None,
            woke_at: None,
            bright: full_brightness(),
            curtain: Fade::shown(BRIGHTEN, DARKEN),
            darkening: None,
            charging: None,
            charge_art: None,
            charge_canvas: None,
            blink: None,
            edits: 0,
            caret_shown: false,
            morph: None,
            row_press: None,
            row_art: None,
            shown: None,
            turn: None,
            layer: None,
            stale: Vec::new(),
            cols: 0,
            backdrop: None,
            corners: 0.0,
            dirty: true,
            w,
            h,
            px_per_dp,
        }
    }

    /// The screen's corners are rounded to `radius` pixels: so are the
    /// leading corners of a page sliding over another.
    pub fn set_screen_corners(&mut self, radius: f32) {
        self.corners = radius;
    }

    pub fn theme(&self) -> Theme {
        if self.state.dark {
            Theme::dark(self.w, self.h, self.px_per_dp)
        } else {
            Theme::light(self.w, self.h, self.px_per_dp)
        }
    }

    /// Whether a download asked for on the download page is under way, which
    /// the page stops rather than starts.
    pub fn fetching(&self) -> bool {
        matches!(self.state.fetch, Fetch::Running { .. })
    }

    pub fn screen(&self) -> Screen {
        self.top().0
    }

    fn top(&self) -> &(Screen, f32, Focus) {
        self.stack.last().expect("the page stack always holds a page")
    }

    fn top_mut(&mut self) -> &mut (Screen, f32, Focus) {
        self.stack.last_mut().expect("the page stack always holds a page")
    }

    /// Whether the panel needs a new frame since the last call.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Whether an install on the page is writing, or looking at what it
    /// writes, which the bar under it follows.
    fn install_writing(&self) -> bool {
        matches!(
            &self.state.install,
            Install::Updating { stage: Some(_), .. }
                | Install::Merging { .. }
                | Install::Rooting { .. }
                | Install::Flashing { stage: Some(_), .. }
                | Install::Systeming { stage: Some(_), .. }
                | Install::Checking { stage: Some(_), .. }
        )
    }

    /// When time next has to pass through [`App::tick`]: at once while
    /// something on the lit screen moves on its own, a frame on while the
    /// backlight eases, else when a shown message should disappear or the
    /// panel should dim or go off.
    pub fn deadline(&self, now: u64) -> Option<u64> {
        let sliding = matches!(self.button, Some((_, Press::Slide { .. }))) && self.state.confirm < 1.0;
        let moving = self.display_on
            && (self.fling.is_some()
                || self.glide.is_some()
                || self.bounce.is_some()
                || self.seek.is_some()
                || sliding
                || self.cue.is_some()
                || self.title.moving(now)
                || self.bar.moving(now)
                || self.morph.is_some()
                || self.turn.is_some()
                || self.row_press.is_some_and(|p| p.changing(now) || p.gone(now))
                || self.turning()
                || self.shade_busy()
                || self.shot_when.is_some()
                || self.glowing());
        // The content going dark or coming up, and the charging screen's
        // tide, move a frame at a time while the panel is lit.
        let lit = self.panel(now).is_some();
        if moving || (lit && (self.curtain.moving(now) || self.charging.is_some())) {
            return Some(0);
        }
        let darkens = self.row_press.map(|p| p.rise).filter(|&rise| rise > now);
        let easing = self.bright.moving(now).then_some(now + BACKLIGHT_STEP_MS);
        // An install writes for minutes, and the bar under it follows what it
        // reports a moment behind: a page looked at once a second shows that
        // as a bar that jumps. A tenth of a second is enough for the eye, and
        // costs nothing while the page is the only thing on screen
        // (2026-11-20).
        if self.display_on && self.install_writing() {
            return Some(now + INSTALL_FRAME_MS);
        }
        let held = match self.drag {
            Drag::Press { action: Some(Action::OpenEntry(_)), at, .. } => Some(at + LONG_PRESS_MS),
            _ => None,
        };
        let repeat = match self.button {
            Some((_, Press::Volume { next, .. })) => Some(next),
            Some((_, Press::Power { spot: Spot { stop: Stop::Hit(Action::OpenEntry(_)), .. }, at })) => Some(at + LONG_PRESS_MS),
            _ => None,
        };
        let blink = self.blink_at(now).filter(|_| self.display_on).map(|(_, next)| next);
        let chord = self.chord.map(|(_, at)| at + CHORD_MS);
        [self.toast.as_ref().map(|t| t.1), easing, self.idle_due(), held, repeat, blink, darkens, chord].into_iter().flatten().min()
    }

    /// Whether the ring about the sign on the decrypt page is turning, which
    /// asks the loop for a frame every turn rather than a second apart.
    fn turning(&self) -> bool {
        self.display_on
            && self.screen() == Screen::Decrypt
            && (self.data_finish.is_some() || matches!(self.state.data, Data::Closed | Data::Opening))
    }

    /// Step the bar on the decrypt page on toward the end: one sweep from
    /// the moment the page starts waiting to the moment the partition is
    /// open. The work reports its own stages, and the sweep keeps moving
    /// past them rather than stopping on each one, so that the bar reads as
    /// one job instead of three (2026-11-19).
    fn step_data_bar(&mut self, now: u64) {
        /// How long the sweep takes to cover most of its length: the work
        /// behind it is seconds, and a bar that crawls reads as a phone that
        /// stopped.
        const SWEEP_S: f32 = 1.0;
        /// The rest of the length, run out over the wait of a slow opening,
        /// so that the bar keeps moving without ever claiming the end.
        const TAIL_S: f32 = 6.0;
        /// How long the last length takes once the partition is open: a pace
        /// the eye follows, where a bar that jumps to the end reads as a page
        /// that was never doing anything.
        const LAST_S: f32 = 0.55;
        let last = std::mem::replace(&mut self.data_moved, now);
        let dt = (now.saturating_sub(last) as f32 / 1000.0).clamp(0.0, 0.25);
        let here = self.state.data_progress;
        if self.data_finish.is_some() {
            // Its last length runs from wherever the sweep had got to, at a
            // pace of its own: the bar fills as the page is about to leave.
            let at = *self.data_finish_at.get_or_insert(now);
            let k = (now.saturating_sub(at) as f32 / (LAST_S * 1000.0)).clamp(0.0, 1.0);
            let eased = k * k * (3.0 - 2.0 * k);
            let from = self.data_finish_from.min(here);
            self.state.data_progress = from + (1.0 - from) * eased;
            return;
        }
        let started = *self.data_started.get_or_insert(now);
        let t = now.saturating_sub(started) as f32 / 1000.0;
        // One sweep: most of it in the first seconds, the last of it easing
        // out over however long the work takes.
        let sweep = 0.86 * (1.0 - (-t / SWEEP_S).exp()) + 0.134 * (1.0 - (-t / TAIL_S).exp());
        // What the work itself reported is a floor, never a ceiling: a page
        // that waits on a stage it has already passed still moves.
        let want = sweep.max(self.state.data_reported).min(0.994);
        if want <= here {
            return;
        }
        let ease = 1.0 - (-dt / 0.15).exp();
        self.state.data_progress = (here + (want - here) * ease).min(want);
    }

    /// Step the bar on the installing page on toward what update_engine last
    /// said it had written. It says so about every thirty seconds, so the bar
    /// eases toward each count instead of jumping to it, and creeps onward
    /// while no count comes: a bar that stands still for half a minute reads
    /// as a page that stopped (2026-11-20).
    fn step_install_bar(&mut self, now: u64) {
        /// The least the bar moves a second while a count is waited on.
        const CREEP: f32 = 0.0015;
        /// How far past the last count the bar may run before the next one
        /// comes, at the pace the counts have come at: no more than three
        /// percent, so that a count that fails to come cannot leave the bar
        /// far ahead of the work.
        const SLACK: f32 = 0.03;
        let writing = match &self.state.install {
            Install::Updating { stage: Some(Stage::Writing { done, total }), .. } if *total > 0 => Some((*done, *total)),
            _ => None,
        };
        let Some((done, total)) = writing else { return };
        let target = (done as f32 / total as f32).clamp(0.0, 1.0);
        let (seen, at) = self.install_seen;
        // The first turn has no turn behind it to measure against.
        let dt = if at == 0 { 0.016 } else { (now.saturating_sub(at) as f32 / 1000.0).clamp(0.0, 5.0) };
        if done > seen {
            // The first count seen is not a pace: the pace is worked out from
            // the one after it (2026-11-20).
            if at != 0 && dt >= 0.2 {
                let rate = (done - seen) as f32 / dt;
                self.state.install_rate =
                    if self.state.install_rate > 0.0 { self.state.install_rate * 0.6 + rate * 0.4 } else { rate };
            }
            self.install_seen = (done, now);
        }
        let here = self.state.install_share;
        // A step that starts again, as the write is followed by the read
        // back, puts the bar back where that step is.
        if target + 0.001 < here {
            self.state.install_share = target;
            self.dirty = true;
            return;
        }
        // The counts come half a minute apart, and a bar that stands still
        // that long reads as a page that stopped: it goes on from the last
        // count at the pace the counts have been coming at, and never more
        // than a little ahead of it (2026-11-20).
        let since = now.saturating_sub(at) as f32 / 1000.0;
        let pace = self.state.install_rate / total as f32;
        let ceiling = (target + (pace * since).min(SLACK) + 0.001).min(0.999);
        if here >= ceiling {
            return;
        }
        let ease = 1.0 - (-dt.max(0.016) / 0.5).exp();
        let step = ((ceiling - here) * ease).max(dt * CREEP);
        self.state.install_share = (here + step).min(ceiling);
        self.dirty = true;
    }

    /// Whether the lock is over the page, waiting for the knob: the panel must
    /// keep its touch controller awake then, or the taps that would light it
    /// are never reported.
    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }

    /// Whether the control centre is out, on its way out or being pulled, which
    /// the loop asks for a frame a turn while.
    pub fn shade_busy(&self) -> bool {
        self.shade > 0.0 || self.shade_settle.is_some() || matches!(self.drag, Drag::Shade { .. })
    }

    /// Whether a text field's cursor shows at `now`, and when it next turns;
    /// `None` while no cursor is up.
    fn blink_at(&self, now: u64) -> Option<(bool, u64)> {
        let (_, _, since) = self.blink?;
        let n = now.saturating_sub(since) / BLINK_MS;
        Some((n.is_multiple_of(2), since + (n + 1) * BLINK_MS))
    }

    /// When sitting idle next changes the lit panel: when it dims, or once
    /// it is dimming, when it goes off.
    fn idle_due(&self) -> Option<u64> {
        let (active, after) = (self.active?, SCREEN_OFF[self.state.screen_off]?);
        if !self.display_on {
            return None;
        }
        Some(if self.bright.on { active + after - dim_span(after) } else { active + after })
    }

    /// Let time pass: the panel dims and goes off once left alone long
    /// enough, a gliding page moves on, a page past its end springs back,
    /// the bar and its title follow the scroll, and a message past its
    /// deadline goes away.
    pub fn tick(&mut self, now: u64) -> Vec<Effect> {
        let mut fx = Vec::new();
        // The control centre, let go, settles into place in one move, easing
        // out rather than swinging about where it is headed for.
        if let Some((at, from, _speed, period, target)) = self.shade_settle {
            let t = now.saturating_sub(at) as f32 / 1000.0;
            let k = (t / period).clamp(0.0, 1.0);
            let eased = k * k * (3.0 - 2.0 * k);
            self.shade = ((target + from * (1.0 - eased)) / SHADE_PULL_DP).clamp(0.0, 1.0);
            if k >= 1.0 {
                self.shade_settle = None;
                self.shade = (target / SHADE_PULL_DP).clamp(0.0, 1.0);
            }
            self.dirty = true;
        }
        // 录屏时每秒换一次已录时长：页面和状态栏都读这一个串，免得两处各算
        // 一遍（2026-11-21）。暂停的那一段不算进去，停着的时候秒数不动。
        if self.state.record.on {
            let (since, held, paused_at, paused) = {
                let r = &self.state.record;
                (r.since, r.held, r.paused_at, r.paused)
            };
            let up_to = if paused { paused_at } else { now };
            let secs = up_to.saturating_sub(since).saturating_sub(held) / 1000;
            let text = format!("{}:{:02}", secs / 60, secs % 60);
            if text != self.state.record.elapsed {
                self.state.record.elapsed = text;
                self.dirty = true;
            }
        }
        // The installing page's bar follows what update_engine reports it
        // has written, at a pace the eye follows rather than in the jumps of
        // thirty seconds it reports them in (2026-11-20).
        if self.display_on {
            self.step_install_bar(now);
        }
        // The decrypt page while the partition is being opened, and while it
        // waits for the phone's own parts to come up: its ring turns every
        // frame, and the bar under it steps on toward the work's own stages.
        // With the partition open the page stays a moment longer, until the
        // bar has run its last length, so that it is never left part full.
        let decrypting = self.screen() == Screen::Decrypt
            && matches!(self.state.data, Data::Closed | Data::Opening)
            && self.display_on;
        let finishing = self.data_finish.is_some() && self.screen() == Screen::Decrypt && self.display_on;
        if decrypting || finishing {
            self.state.data_phase = (now % RING_MS) as f32 / RING_MS as f32;
            if decrypting {
                self.data_started.get_or_insert(now);
            }
            self.step_data_bar(now);
            let mut leaving = None;
            if let Some(after) = self.data_finish {
                let at = *self.data_finish_at.get_or_insert(now);
                // The bar's last length, and no longer than that: whatever
                // stops the bar, the page still does what it is for. A page
                // waiting for a bar that no longer moves is the one thing it
                // must never be left doing (2026-11-19).
                // The page goes with the frame it last showed, so it goes only
                // once that frame is back: the one let go for the ring to turn
                // is drawn again by the turn's own draw (2026-11-19).
                if (self.state.data_progress >= 0.999 || now.saturating_sub(at) >= LAST_LEN_MS) && self.frame.is_some() {
                    self.data_finish = None;
                    self.data_finish_at = None;
                    leaving = Some(after);
                }
            }
            match leaving {
                Some(DataAfter::Open) => {
                    self.state.data = Data::Open;
                    self.state.code.clear();
                    self.refresh();
                }
                Some(DataAfter::Home) => self.leave_decrypt(),
                None => {
                    // The frame drawn is the page as it stood: without letting
                    // it go, the ring is drawn once and never turns again. It
                    // is let go only while the page stays; leaving takes it.
                    self.frame = None;
                    self.dirty = true;
                }
            }
        } else if !matches!(self.state.data, Data::Closed | Data::Opening | Data::Locked { .. }) {
            // Nothing is opening the partition any more: the next page that
            // opens it starts the sweep afresh. A page left off the screen
            // while it still opens keeps its start, so that the bar goes on
            // from where it was rather than crawling up behind the work; a
            // page asking for a screen lock keeps it too, so that the bar
            // does not stand still while what was typed opens the files
            // behind it (2026-11-19).
            self.data_started = None;
        }
        // The key waiting for its partner in a screenshot, left alone long
        // enough, does what it does alone.
        // The sheet goes first, and the shot is taken a few frames later, once
        // the page under it has been drawn: taken at once, the sheet would be
        // in it.
        if let Some(at) = self.shot_when
            && now >= at
        {
            self.shot_when = None;
            fx.push(Effect::Screenshot { storage: self.state.place_open(Place::Storage) });
        }
        // 落空的翻页意图：列表一到就补这一下（点击发生时按键还没画出来）。
        if let Some(by) = self.state.page_pending.take() {
            if matches!(self.screen(), Screen::DataApps | Screen::Modules) && matches!(&self.state.apps, Some(Ok(_))) {
                let action = match self.screen() {
                    Screen::DataApps => Action::AppPage(by),
                    _ => Action::ModulePage(by),
                };
                fx.extend(self.act(action, now));
            }
        }
        if self.chord.is_some_and(|(_, at)| now >= at + CHORD_MS) {
            fx.extend(self.let_chord_go(now));
        }
        // An entry of the file manager held long enough, under a finger or
        // the power key, opens what can be done with it, a folder as much as
        // a file, with a tap of the motor.
        if let Drag::Press { action: Some(Action::OpenEntry(i)), at, .. } = self.drag
            && now >= at + LONG_PRESS_MS
        {
            self.drag = Drag::Held;
            fx.extend(self.held_entry(i));
        }
        // 模块长按 = 标记下次启动时移除（再长按一次取消），和 KSU 的做法一致。
        if let Drag::Press { action: Some(Action::ModuleToggle(i)), at, .. } = self.drag
            && now >= at + LONG_PRESS_MS
        {
            self.drag = Drag::Held;
            fx.push(Effect::Haptic);
            fx.extend(self.act(Action::ModuleRemove(i), now));
        }
        if let Some((key, Press::Power { spot: Spot { stop: Stop::Hit(Action::OpenEntry(i)), .. }, at })) = self.button
            && now >= at + LONG_PRESS_MS
        {
            self.button = Some((key, Press::Spent));
            fx.extend(self.held_entry(i));
        }
        let active = *self.active.get_or_insert(now);
        if let (true, Some(after)) = (self.display_on, SCREEN_OFF[self.state.screen_off]) {
            let idle = now.saturating_sub(active);
            if idle >= after {
                self.sleep(now);
            } else if idle >= after - dim_span(after) {
                self.bright.set(false, now);
            }
        }
        // The charging screen goes once it has shown long enough: on a
        // panel lit for it, with the panel, else for the page.
        if self.display_on && self.charging.as_ref().is_some_and(|c| c.shown.on && now >= c.since + CHARGE_MS) {
            let c = self.charging.as_mut().expect("the charging screen shows");
            if c.dark_after {
                self.sleep(now);
            } else {
                c.shown.set(false, now);
            }
        }
        if self.charging.as_ref().is_some_and(|c| !c.shown.on && !c.shown.moving(now)) {
            self.end_charging();
        }
        // Once the content is dark, the panel is off, and what showed goes
        // with it.
        if !self.display_on && self.darkening.is_some() && self.curtain.level(now) <= 0.0 {
            self.darkening = None;
            self.end_charging();
        }
        if self.curtain.moving(now) || self.charging.is_some() {
            self.dirty = true;
        }
        if self.toast.as_ref().is_some_and(|m| now >= m.1) {
            self.toast = None;
            self.dirty = true;
        }
        if self.blink_at(now).is_some_and(|(on, _)| on != self.caret_shown) || self.morph.is_some() || self.turn.is_some() {
            self.dirty = true;
        }
        if let Some(p) = self.row_press {
            if p.gone(now) {
                self.row_press = None;
                self.row_art = None;
                self.dirty = true;
            } else if p.changing(now) {
                self.dirty = true;
            }
        }
        if let Some(fling) = self.fling {
            let (y, done) = fling.at(now);
            let next = y.clamp(0.0, self.max_scroll());
            let top = self.top_mut();
            if next != top.1 {
                top.1 = next;
                self.dirty = true;
            }
            if next != y {
                // The glide ran into an end: the speed it had left throws
                // the page past it, and it springs back.
                let speed = fling.velocity(now);
                let period = if speed.abs() <= self.theme().m.dp(HARD_BOUNCE_DP) { 0.4 } else { 0.55 };
                self.bounce = Some(Spring::new(now, 0.0, speed, period));
                self.fling = None;
            } else if done {
                self.fling = None;
                self.settle_fold(now);
            }
        }
        if let Some((glide, pitch, scrolled)) = self.glide {
            let (y, done) = glide.at(now);
            let rows = (y / pitch).trunc() as isize;
            if rows != scrolled {
                self.scroll_rows(rows - scrolled);
            }
            self.glide = if done { None } else { Some((glide, pitch, rows)) };
        }
        if let Some(b) = self.bounce {
            self.dirty = true;
            if b.at(now).1 {
                self.bounce = None;
            }
        }
        if let Some((target, spring)) = self.seek {
            let (off, done) = spring.at(now);
            let at = if done { target } else { (target + off).clamp(0.0, self.max_scroll()) };
            let top = self.top_mut();
            if top.1 != at {
                top.1 = at;
                self.dirty = true;
            }
            if done {
                self.seek = None;
            }
        }
        // The power key held on a swipe-to-confirm control slides its knob.
        if let Some((_, Press::Slide { which, track, at })) = self.button {
            let along = (now.saturating_sub(at) as f32 / SLIDE_MS as f32).min(1.0);
            if along != self.state.confirm {
                self.state.confirm = along;
                self.knob = Some((which, track));
                self.dirty = true;
            }
        }
        // A volume key held moves the light on and on, once the page it
        // last moved on has been drawn.
        if let Some((key, Press::Volume { down, next })) = self.button
            && now >= next
            && self.frame.is_some()
        {
            self.button = Some((key, Press::Volume { down, next: now + REPEAT_MS }));
            fx.extend(self.volume(down, now));
        }
        if let Some((_, _, s)) = self.cue {
            self.dirty = true;
            if s.at(now).1 {
                self.cue = None;
            }
        }
        if self.screen() == Screen::About {
            let look = self.theme().hero.glow;
            match &mut self.glow {
                Some(g) if std::ptr::eq(g.look(), look) => g.advance(now),
                _ => self.glow = Some(GlowClock::new(look, now)),
            }
            if self.glowing() {
                self.dirty = true;
            }
        }
        self.follow_scroll(now);
        fx
    }

    /// Whether the About page's field is on screen, moving.
    fn glowing(&self) -> bool {
        self.glow.is_some() && self.display_on && self.top().1 < self.theme().m.dp(ABOUT_FIELD_DP)
    }

    /// Whether the bar's title and the bar belong over the rendered page
    /// where it is scrolled: the title once the large one has faded, the bar
    /// once the content slides under it. On the About page both come once
    /// the version has faded out.
    fn bar_wanted(&self) -> Option<(bool, bool)> {
        let fr = self.frame.as_ref()?;
        let t = self.theme();
        let scroll = self.top().1;
        Some(if fr.hero.is_some() {
            let gone = scroll >= t.m.dp(ABOUT_LOGO_DP) - widgets::bar_bottom(&t);
            (gone, gone)
        } else {
            let under = (fr.content_top - widgets::bar_bottom(&t) - t.m.dp(20.0)).max(0.0);
            (fr.header && fr.title_lift(scroll) >= t.m.dp(TITLE_FADE_DP), scroll > under)
        })
    }

    /// Bring the bar and its title in or out as the page scrolls.
    fn follow_scroll(&mut self, now: u64) {
        let Some((title, bar)) = self.bar_wanted() else { return };
        self.title.set(title, now);
        self.bar.set(bar, now);
        if self.title.moving(now) || self.bar.moving(now) {
            self.dirty = true;
        }
    }

    /// How far past its end the page shows at `now`: negative past the
    /// top, positive past the bottom.
    fn overscroll(&self, now: u64) -> f32 {
        if self.pull != 0.0 {
            motion::rubber(self.pull, self.h as f32)
        } else {
            self.bounce.map_or(0.0, |b| b.at(now).0)
        }
    }

    /// Something laid over the page changed (the clock): compose again.
    pub fn redraw(&mut self) {
        self.dirty = true;
    }

    /// The settings the pages show, as they are kept between starts.
    pub fn settings(&self) -> Settings {
        Settings::of(&self.state)
    }

    /// Take the settings `kept` from the starts before, and speak the
    /// language chosen then, or else the one the system `named`, or else
    /// the one the pages last spoke.
    pub fn take_settings(&mut self, kept: Option<&Settings>, named: Option<Lang>) {
        if let Some(kept) = kept {
            kept.apply(&mut self.state);
        }
        if !self.state.language_chosen
            && let Some(lang) = named.or(kept.and_then(Settings::spoken))
        {
            self.state.lang = lang;
        }
        self.refresh();
    }

    /// The page's readings changed (battery, storage): render it again.
    pub fn refresh(&mut self) {
        self.frame = None;
        self.stale.clear();
        self.spots = None;
        self.knob = None;
        self.row_art = None;
        self.shown = None;
        self.layer = None;
        self.spare.extend(self.drawn.drain(..).map(|(_, c)| c));
        self.dirty = true;
    }

    /// What the page shows changed only in rows `first..end`: draw those
    /// again, and the rest stays as drawn.
    fn refresh_rows(&mut self, first: u32, end: u32) {
        if self.frame.is_none() {
            return;
        }
        self.stale.push((first, end));
        self.spots = None;
        self.shown = None;
        self.layer = None;
        self.dirty = true;
    }
}

#[cfg(test)]
mod align_tests {
    //! 翻页按钮与标题的竖直位置：用像素量，不靠眼睛。

    use super::testing::*;
    use super::{Action, App, Effect, Screen};

    #[test]
    fn 翻页与标题在同一条水平线上() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::DataApps, W, H, PPD);
        let apps: Vec<super::dataapp::DataApp> = (0..25)
            .map(|k| super::dataapp::DataApp {
                package: format!("com.example.app{k}"),
                dir: format!("/data/app/~~x/com.example.app{k}-y"),
                label: format!("应用 {k}"),
                version: String::new(),
                installed: 0,
                updated: 0,
                size: 0,
                icon: Vec::new(),
                system: false,
            })
            .collect();
        app.state.apps = Some(Ok(apps));
        let frame = app.frame(&f, &i);
        let next = frame.hits.iter().find(|h| h.action == Action::AppPage(1)).expect("a next pill").rect;
        let t = app.theme();
        let sprite = crate::widgets::big_title(&t, &f, "应用管理");
        let mut first = None;
        let mut last = 0;
        for row in 0..sprite.h as usize {
            let covered = (0..sprite.w as usize).any(|col| sprite.cov[row * sprite.w as usize + col] > 8);
            if covered {
                first.get_or_insert(row);
                last = row;
            }
        }
        let ink = sprite.y + (first.unwrap_or(0) as f32 + last as f32) / 2.0;
        let pill = next.centre().1;
        println!("标题墨迹中心 {ink:.1}，翻页按钮中心 {pill:.1}，差 {:.1}", pill - ink);
        assert!((pill - ink).abs() <= 6.0, "翻页与标题应当在同一条水平线上，差 {:.1}px", pill - ink);
    }

    #[test]
    fn 点一次下一页就翻一页() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::DataApps, W, H, PPD);
        let apps: Vec<super::dataapp::DataApp> = (0..25)
            .map(|k| super::dataapp::DataApp {
                package: format!("com.example.app{k}"),
                dir: format!("/data/app/~~x/com.example.app{k}-y"),
                label: format!("应用 {k}"),
                version: String::new(),
                installed: 0,
                updated: 0,
                size: 0,
                icon: Vec::new(),
                system: false,
            })
            .collect();
        app.state.apps = Some(Ok(apps));
        for want in [1usize, 2] {
            tap(&mut app, &f, &i, Action::AppPage(1));
            assert_eq!(app.state.app_page, want, "点一次翻一页");
        }
    }

    #[test]
    fn 三个翻页控件不互相压住() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::DataApps, W, H, PPD);
        let apps: Vec<super::dataapp::DataApp> = (0..90)
            .map(|k| super::dataapp::DataApp {
                package: format!("com.example.app{k}"),
                dir: format!("/data/app/~~x/com.example.app{k}-y"),
                label: format!("应用 {k}"),
                version: String::new(),
                installed: 0,
                updated: 0,
                size: 0,
                icon: Vec::new(),
                system: false,
            })
            .collect();
        app.state.apps = Some(Ok(apps));
        app.state.app_page = 4;
        let fr = app.frame(&f, &i);
        let prev = fr.hits.iter().find(|h| h.action == Action::AppPage(-1)).expect("prev").rect;
        let next = fr.hits.iter().find(|h| h.action == Action::AppPage(1)).expect("next").rect;
        let t = app.theme();
        println!("m.w={} margin={:.1} 上一页 {:?}  下一页 {:?}", t.m.w, t.m.margin, prev, next);
        assert!(prev.x + prev.w < next.x, "上一页不能压到下一页：{:?} / {:?}", prev, next);
    }

    #[test]
    fn 翻页按钮的位置不随页码变() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::DataApps, W, H, PPD);
        let apps: Vec<super::dataapp::DataApp> = (0..30)
            .map(|k| super::dataapp::DataApp {
                package: format!("com.example.app{k}"),
                dir: format!("/data/app/~~x/com.example.app{k}-y"),
                label: format!("应用 {k}"),
                version: String::new(),
                installed: 0,
                updated: 0,
                size: 0,
                icon: Vec::new(),
                system: false,
            })
            .collect();
        app.state.apps = Some(Ok(apps));
        let first = app.frame(&f, &i).hits.iter().find(|h| h.action == Action::AppPage(1)).expect("next on page 1").rect;
        app.state.app_page = 1;
        let second = app.frame(&f, &i).hits.iter().find(|h| h.action == Action::AppPage(1)).expect("next on page 2").rect;
        assert_eq!(first, second, "「下一页」不能在翻页后挪位置");

        // 模块页同样。
        let mut m = App::new(Screen::Modules, W, H, PPD);
        let mods: Vec<super::ModuleRow> = (0..20)
            .map(|k| super::ModuleRow {
                id: format!("m{k}"),
                name: format!("模块 {k}"),
                note: "v1".into(),
                about: String::new(),
                enabled: true,
                removing: false,
            })
            .collect();
        m.state.modules = Some(Ok(mods));
        let a = m.frame(&f, &i).hits.iter().find(|h| h.action == Action::ModulePage(1)).expect("next on page 1").rect;
        m.state.module_page = 1;
        let b = m.frame(&f, &i).hits.iter().find(|h| h.action == Action::ModulePage(1)).expect("next on page 2").rect;
        assert_eq!(a, b, "模块页的「下一页」也不能挪位置");
    }

    #[test]
    fn 模块页的翻页也在标题那条线上() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Modules, W, H, PPD);
        let mods: Vec<super::ModuleRow> = (0..16)
            .map(|k| super::ModuleRow {
                id: format!("m{k}"),
                name: format!("模块 {k}"),
                note: "v1.0".into(),
                about: String::new(),
                enabled: true,
                removing: false,
            })
            .collect();
        app.state.modules = Some(Ok(mods));
        let frame = app.frame(&f, &i);
        let next = frame.hits.iter().find(|h| h.action == Action::ModulePage(1)).expect("a next pill").rect;
        let t = app.theme();
        let sprite = crate::widgets::big_title(&t, &f, "模块管理");
        let mut first = None;
        let mut last = 0;
        for row in 0..sprite.h as usize {
            let covered = (0..sprite.w as usize).any(|col| sprite.cov[row * sprite.w as usize + col] > 8);
            if covered {
                first.get_or_insert(row);
                last = row;
            }
        }
        let ink = sprite.y + (first.unwrap_or(0) as f32 + last as f32) / 2.0;
        let pill = next.centre().1;
        println!("模块页：标题墨迹中心 {ink:.1}，翻页按钮中心 {pill:.1}，差 {:.1}", pill - ink);
        assert!((pill - ink).abs() <= 6.0, "模块页的翻页也应当在标题那条线上，差 {:.1}px", pill - ink);
    }

    /// 系统应用点得开：详情页给的是导出安装包，不是卸载（分区只读）。
    #[test]
    fn 系统应用点得开详情并能导出安装包() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::SystemApps, W, H, PPD);
        app.set_sys_apps(vec![super::dataapp::DataApp {
            package: "com.android.settings".into(),
            dir: "/mnt/system/system/priv-app/Settings".into(),
            label: "com.android.settings".into(),
            version: String::new(),
            installed: 0,
            updated: 0,
            size: 4096,
            icon: Vec::new(),
            system: true,
        }]);
        assert_eq!(tap(&mut app, &f, &i, Action::SysAppOpen(0)), Vec::new());
        assert_eq!(app.screen(), Screen::DataAppInfo, "点一行进的是详情");
        let page = app.frame(&f, &i);
        let export = Action::SysAppExport(0);
        assert!(page.hits.iter().any(|h| h.action == export), "详情里有导出");
        assert!(
            page.hits.iter().all(|h| !matches!(h.action, Action::DataAppRemove(_) | Action::DataAppOpen(_))),
            "系统应用不从这里卸"
        );
        assert_eq!(
            tap(&mut app, &f, &i, export),
            vec![Effect::ExportApk { from: "/mnt/system/system/priv-app/Settings".into(), name: "com.android.settings".into() }]
        );
    }
}
