//! What the pages show and the user changes, all in one place.

use super::*;

/// The folder served to the network the phone is on: not served, served and
/// where, or why it could not be served.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Serving {
    #[default]
    Off,
    Running {
        /// The address a browser on the same network opens.
        at: String,
        /// How many requests it has answered, so the page shows it is alive.
        served: u64,
    },
    Failed(String),
}

/// How a download asked for on the download page stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Fetch {
    /// Nothing asked for yet.
    #[default]
    Idle,
    /// How many bytes have come, and how many the server named where it did.
    Running { done: u64, total: Option<u64> },
    Done { path: String, bytes: u64 },
    Failed(String),
    Stopped,
}

/// 应用管理看的是用户装的应用，还是系统分区里的。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppsScope {
    User,
    System,
}

/// 一份留在机主存储里的 vbmeta 备份，页面上按这个列出。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VbmetaKept {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    pub rollback_index: u64,
    pub slot: String,
}

/// 拼音输入：开没开、正在打的串、候选翻到第几页。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pinyin {
    /// 中文模式开着（键盘上那条的「中/EN」键切）。
    pub on: bool,
    /// 正在打的拼音，全是小写字母。
    pub composing: String,
    /// 候选翻到第几页，从 0 起。
    pub page: usize,
}

impl Default for Pinyin {
    /// 默认就是中文：页面本来就是中文的，一进输入框就能打拼音。以前默认关着，
    /// 而那条只在开着时才画，于是「中/EN」这个开关永远没机会按到 —— v82 的
    /// 键盘上方什么都没有，就是这个（2026-11-21）。
    fn default() -> Self {
        Pinyin { on: true, composing: String::new(), page: 0 }
    }
}

/// 录屏这一页和状态栏要的东西：在不在录、从什么时候起、写哪个文件，
/// 以及录成多大、多清楚、多少帧。
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// 正在录。
    pub on: bool,
    /// 这一段开始的时间，毫秒。
    pub since: u64,
    /// 写进去的文件，显示用。
    pub file: String,
    /// 规格，如 `1080×2400 · 30 帧`。
    pub spec: String,
    /// 已录多久，形如 `0:42`；由 `tick` 每秒换一次。
    pub elapsed: String,
    /// 写进文件的帧数，收尾时由外面报回来。
    pub frames: u64,
    /// 分辨率，[`crate::record::SCALES`] 里的第几个。
    pub scale: usize,
    /// 画质，[`crate::record::QUALITIES`] 里的第几个。
    pub quality: usize,
    /// 帧率，[`crate::record::RATES`] 里的第几个。
    pub fps: usize,
    /// 计时胶囊摆在哪儿：左上角的 dp 坐标，`None` 是默认的右上角。拖过一次
    /// 就记住，下一段还从这儿开始。
    pub pill: Option<(f32, f32)>,
    /// 按了暂停，这一段停在那儿。
    pub paused: bool,
    /// 停下的时刻，毫秒。
    pub paused_at: u64,
    /// 这一段一共停了多久，毫秒：已录时长要把它减掉。
    pub held: u64,
}

impl Default for Record {
    /// 一上来是标准档：面板的一半、中等画质、30 帧 —— 编码有富余，画面真在动。
    fn default() -> Self {
        Record {
            on: false,
            since: 0,
            file: String::new(),
            spec: String::new(),
            elapsed: String::new(),
            frames: 0,
            scale: crate::record::DEFAULT_SCALE,
            quality: crate::record::DEFAULT_QUALITY,
            fps: crate::record::DEFAULT_FPS,
            pill: None,
            paused: false,
            paused_at: 0,
            held: 0,
        }
    }
}

/// 录屏页上那三条能改的：分辨率、画质、帧率。容器只有 MP4，没得选。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordSetting {
    Scale,
    Quality,
    Fps,
}

/// 搜索框找的是哪一类东西：整台机器的功能，还是某一页里的条目。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScope {
    Features,
    Apps,
    Modules,
}

/// One module the phone's own system would load: KernelSU's and Magisk's keep
/// them the same way, a folder each with a `module.prop` beside it, and a
/// `disable` file inside a folder that is switched off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleRow {
    /// The folder's name, which is how the module is known.
    pub id: String,
    pub name: String,
    /// What module.prop says of it: its version and who made it.
    pub note: String,
    /// Its own words about what it does.
    pub about: String,
    pub enabled: bool,
    /// Marked to be removed the next time the phone starts.
    pub removing: bool,
}

/// Choices the pages show and the user changes, and where installing
/// stands.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// The language the pages speak.
    pub lang: Lang,
    /// The language was chosen on the language page, now or at an earlier
    /// start, where otherwise it is the one the system names.
    pub language_chosen: bool,
    pub dark: bool,
    pub brightness: f32,
    /// How long the screen stays on with nothing touched, by its place in
    /// [`SCREEN_OFF`].
    pub screen_off: usize,
    /// The time zone the clock and the times on the pages keep, by its
    /// place in [`TIME_ZONES`].
    pub time_zone: usize,
    pub vibrate: bool,
    /// How strongly the motor taps, from 0 at the slider's left end to 1
    /// at its right.
    pub vibration: f32,
    /// A computer connected over USB is offered internal storage.
    pub file_transfer: bool,
    pub reboot: usize,
    /// What the wipe page has chosen, by place in [`WIPES`].
    pub wipe: [bool; WIPES.len()],
    /// The phone has a cache partition, which the wipe page then offers.
    pub cache_partition: bool,
    /// How many folders of its own the phone keeps logs in on the data
    /// partition, which erasing the logs takes in.
    pub phone_logs: usize,
    pub pattern: Vec<usize>,
    /// How far the swipe-to-confirm knob is dragged, 0 to 1.
    pub confirm: f32,
    pub install: Install,
    /// Where the package being installed comes from.
    pub source: Source,
    /// The name of the file being installed, when it is one on the phone.
    pub package_name: Option<String>,
    /// How far the bar on the installing page is drawn, 0 to 1: it follows
    /// what the installer reports a moment behind rather than jumping to
    /// it, so that a count written every thirty seconds still reads as one
    /// long job (2026-11-20).
    pub install_share: f32,
    /// The bytes a second the install is going at, as its reports tell.
    pub install_rate: f32,
    /// What update_engine is doing, its latest lines, the newest last: the
    /// page shows them under the bar and lets the older ones go up out of
    /// sight, so a nine-gigabyte package reads as work still going on.
    pub install_log: Vec<String>,
    /// What became of the phone's root as the update was installed: put
    /// back into the boot image it wrote, left for its own manager to
    /// patch, or missed (2026-11-20).
    pub root: Option<skyward_install::root::Rooted>,
    /// What the install script last run showed, its latest lines.
    pub script_output: Vec<String>,
    /// A zip's install script runs only if the zip was signed with one of
    /// the phone's update keys.
    pub check_signature: bool,
    /// The phone keeps its recovery in a partition of each slot, which the
    /// install page then offers to keep Skyward in.
    pub recovery_partitions: bool,
    /// Skyward goes back into the recovery partitions an install writes
    /// over.
    pub keep_recovery: bool,
    /// How Skyward went back into the recovery partitions the install
    /// wrote over, where it did.
    pub reflash: Option<Reflash>,
    /// The mount page's partitions, as the runtime last read them, or why
    /// it could not.
    pub mounts: Result<Vec<MountRow>, String>,
    pub mount_failure: Option<MountFailure>,
    /// Which of [`BACKUP_PARTS`] a backup takes.
    pub backup: [bool; 4],
    /// Where a backup goes.
    pub backup_to: Place,
    pub compress: bool,
    pub verify: bool,
    /// The backups found, newest first, those that cannot be read last.
    pub backups: Vec<BackupEntry>,
    /// The backup the restore page shows, as it was found, and which of its
    /// parts to restore.
    pub restore: Option<(BackupEntry, [bool; 4])>,
    /// Backing up or restoring under way, or how it last ended.
    pub backup_job: Option<BackupJob>,
    /// Which logs to export: the recovery's, the kernel's.
    pub logs: [bool; 2],
    /// Where the logs go: 0 for the temporary folder, 1 for internal
    /// storage.
    pub logs_to: usize,
    /// What the phone's boot control holds of the slots, as the runtime
    /// last read it for the slot or reboot page, or why it could not.
    pub boot_slots: Option<Result<BootSlots, String>>,
    /// The slot picked on the slot page to start next; with none picked,
    /// the page shows the one the boot control starts next.
    pub next_slot: Option<Slot>,
    /// The phone has what a second system needs: boot and init_boot
    /// partitions in both slots, which a switch writes, and recovery
    /// partitions apart from them, which keep Skyward through it.
    pub dual_boot: bool,
    /// Whether this build offers to update itself, as its device says.
    pub updates: bool,
    /// The modules installed for the phone's own system, read when the page
    /// opens: the folder each sits in, what its module.prop says of it, and
    /// whether it is switched on.
    pub modules: Option<Result<Vec<ModuleRow>, String>>,
    /// The module the detail page shows, by its place in the list.
    pub module_at: Option<usize>,
    /// The apps installed in the data partition, read when the page opens.
    pub apps: Option<Result<Vec<crate::app::dataapp::DataApp>, String>>,
    /// The app the detail page shows, by its place in the list.
    pub app_at: Option<usize>,
    /// Each app's icon as pixels ready to draw: width, height and RGBA, the
    /// same order as `apps`.
    pub app_icons: Vec<(u32, u32, Vec<u8>)>,
    /// 模块管理这一页看到的第几屏。
    pub module_page: usize,
    /// 应用管理这一页看到的第几屏（一屏装不下一百多个应用）。
    pub app_page: usize,
    /// 应用管理当前显示哪一类：用户应用或系统应用。
    pub apps_scope: AppsScope,
    /// 系统分区的预装应用，只读页用；没有读过就是 None。
    pub sys_apps: Option<Vec<crate::app::dataapp::DataApp>>,
    /// 只读页看到第几屏。
    pub sys_page: usize,
    /// 系统应用详情页显示的是哪一个：有了它，详情页就从系统应用里取，
    /// 没有就从用户应用里取。
    pub sys_at: Option<usize>,
    /// 点过多少次翻页：用来判断点击到底有没有进来。
    pub app_page_taps: u32,
    /// 进过多少次应用管理这一页。
    pub app_arrivals: u32,
    /// 上一次落在翻页键上的点击：坐标与方向，用来核对是不是点错了键。
    pub page_tap: Option<(f32, f32, i32)>,
    /// 点在翻页那一条带上却没命中任何控件时的坐标。
    pub page_miss: Option<(f32, f32)>,
    /// 点在翻页带上却没命中时记下的意图：右半边 +1，左半边 -1。等翻页键画出来再执行。
    pub page_pending: Option<i32>,
    /// 最近一次画出来的总页数（用于核对翻页键是否已经在画面上）。
    pub pages_shown: usize
,
    /// 应用管理里等确认卸载的那个应用。
    pub app_confirm: Option<usize>,
    /// 模块管理里等确认卸载的那个模块。
    pub module_confirm: Option<usize>,
    /// A copy or a move waiting to be told where it goes: the phone or the
    /// drive, asked before the folder it lands in is walked to.
    pub place_ask: Option<crate::app::FileOp>,
    /// The systems, as the runtime last read them, or why it could not;
    /// none until it has.
    pub systems: Option<Result<Overview, String>>,
    /// The system picked on the systems page to start next; with none
    /// picked, the page shows the one that starts.
    pub system_pick: Option<Who>,
    /// Work on the systems under way.
    pub systems_work: Option<SystemsWork>,
    /// Why the second system's start stopped, as its first program told
    /// it, which the systems page shows.
    pub system_notice: Option<String>,
    /// Where the data partition stands, and what is typed at the screen
    /// lock its users' own files wait on.
    pub data: Data,
    /// How far the ring about the decrypt page's sign has turned, 0 to 1.
    pub data_phase: f32,
    /// How far opening the data partition has come, 0 to 1, as the work last
    /// reported it. The bar on the page runs toward it and never past it by
    /// much, so that it cannot promise work that is not done.
    pub data_reported: f32,
    /// Where that bar stands, 0 to 1: it only ever moves forward.
    pub data_progress: f32,
    /// Whether the phone's own parts -- its partitions, its drivers and its
    /// services -- are up. The decrypt page says it waits for them until they
    /// are, rather than that it is at the partition.
    pub prepared: bool,
    pub code: String,
    /// What is typed in the search field.
    pub search: String,
    /// 搜索页在找什么：功能，还是应用、模块列表里的条目。
    pub search_scope: SearchScope,
    /// The on-screen keyboard's layer.
    pub layer: Layer,
    /// 拼音输入的状态，键盘上方那条读它。
    pub pinyin: Pinyin,
    /// 录屏的状态，页面与状态栏读它。
    pub record: Record,
    /// The key of the on-screen keyboard, or the choice of a list open over
    /// the page, a finger holds down.
    pub held: Option<Action>,
    /// The folder the file manager shows.
    pub files: Folder,
    /// The folder the OTG page shows.
    pub otg: Folder,
    /// The screen lock is about to be kept, and the risk of keeping it is
    /// shown before it is.
    pub asking_remember: bool,
    /// The entry the file page shows was opened on the OTG page, so its path
    /// is measured from the top of the drive rather than the owner's storage.
    pub file_on_drive: bool,
    /// The date the lock screen shows under the clock, as the phone's own
    /// lock screen shows it.
    pub date: String,
    /// The order the file manager shows entries in, kept between starts.
    pub sort: SortKey,
    pub sort_desc: bool,
    pub folders_first: bool,
    /// The folder of internal storage a file to install is chosen in, its
    /// path from the top of the storage.
    pub storage: Folder,
    /// The USB drive plugged in, as the runtime last found it, and the
    /// folder of it a file to install is chosen in, its path from the top
    /// of the drive.
    pub usb_drive: Option<UsbDrive>,
    pub drive: Folder,
    /// The entry the file page describes.
    pub file: Option<FileRow>,
    pub fastboot: Fastboot,
    pub erase: Erase,
    pub asked: Asked,
    /// The entry picked to copy or move, while a folder for it is looked
    /// for.
    pub clip: Option<Clip>,
    /// What is typed as an entry's new name.
    pub new_name: String,
    /// The text file the editor page shows, as typed so far.
    pub editor: Option<Editor>,
    /// Deleting the entry the file page shows, or the backup the restore
    /// page shows, was asked for; a swipe confirms it.
    pub asking_delete: bool,
    /// The picture the phone shows for its owner, and the name it gives
    /// itself: both are on the encrypted data partition, so they come once it
    /// is open, and the home page shows a row of them once they do.
    pub avatar: Option<crate::avatar::Avatar>,
    pub device_name: Option<String>,
    /// The file manager is choosing several entries: the names it has chosen,
    /// in the folder it shows.
    pub choosing: bool,
    pub chosen: std::collections::BTreeSet<String>,
    /// Deleting the entries chosen was asked for: their full paths, waiting on
    /// the swipe that confirms it.
    pub asking_many: Option<Vec<String>>,
    /// A copy or move waiting on what to do with the names taken where it
    /// goes, and those names.
    pub clash: Option<(Clip, String, Vec<String>)>,
    /// Work on files under way, or how it last ended.
    pub job: Option<FileJob>,
    /// Giving the data partition's files their SELinux labels under way, or
    /// how it last ended.
    pub relabel: Option<RelabelJob>,
    /// The filesystem page's partitions, as the runtime last read them, or
    /// why it could not; none until it has.
    pub filesystems: Option<Result<Vec<FsRow>, String>>,
    /// Which of them the page for one shows, and what it is to do to it:
    /// a repair, which leaves what it holds as it is, until another is
    /// chosen.
    pub fs_chosen: usize,
    pub fs_action: FsAction,
    /// Work on whole partitions under way, or how it last ended.
    pub part_work: Option<PartWork>,
    /// Where an update the system installed stands, as the runtime last
    /// read it.
    pub update: Standing,
    /// The WLAN, as the runtime last told it; none on a phone whose WLAN the
    /// recovery does not bring up.
    pub wlan: Option<Wlan>,
    /// Whether the screen lock that opened the user's files is kept for the
    /// starts that follow, so the decrypt page needs nothing typed.
    pub remember: bool,
    /// The folder served to the network, where it is served and how many
    /// requests it has answered; and whether it is being chosen, the file
    /// manager shown for it.
    pub serving: Serving,
    pub picking: bool,
    /// Both of verified boot's checks in the slot the phone runs from, as
    /// read, as the page has turned them before the swipe, and what the last
    /// write said.
    pub avb: Option<Result<(bool, bool), String>>,
    pub avb_pending: Option<(bool, bool)>,
    pub avb_notice: Option<String>,
    /// 存下来的 vbmeta 备份，新的在前。
    pub vbmeta_kept: Option<Result<Vec<VbmetaKept>, String>>,
    /// 备份、列出、写回之后的一句话。
    pub vbmeta_note: Option<String>,
    /// 选中要写回的是第几份。
    pub vbmeta_choice: Option<usize>,
    /// The address typed on the download page, how its download stands, and
    /// where the page another device types an address for it is.
    pub fetch_url: String,
    pub fetch: Fetch,
    pub fetch_portal: Option<String>,
    /// Whether the WLAN opens as the recovery starts, and whether the
    /// password of the network joined last is kept with the network itself.
    pub wlan_auto: bool,
    pub wlan_keep: bool,
    /// The network the password page is for, and what is typed there.
    pub joining: Option<Network>,
    /// The network the details page is of, as it was found.
    pub details: Option<Network>,
    /// The network the page that adds one is adding.
    pub adding: Option<Adding>,
    /// The password typed on the password page or the page that adds a
    /// network, and whether it shows.
    pub password: String,
    pub show_password: bool,
    /// The phone maker's update server is known, and the update page asks
    /// it.
    pub update_server: bool,
    pub check: Check,
    pub have: Have,
    /// Downloading the newest system's full package, or how that last
    /// ended.
    pub download: Option<DownloadJob>,
    pub skyward_check: SkywardCheck,
    /// Updating Skyward, or how that last ended.
    pub skyward_job: Option<SkywardJob>,
    /// How fast data went over the network joined in the last second.
    pub traffic: Traffic,
}

impl Default for State {
    fn default() -> Self {
        State {
            lang: Lang::Hans,
            language_chosen: false,
            dark: false,
            brightness: SLIDER_START,
            // Two minutes.
            screen_off: 3,
            time_zone: 0,
            vibrate: true,
            // The default tap is 0.7 of the full strength: at the slider's
            // own start the motor is barely felt on this phone (2026-11-13).
            vibration: 0.7,
            file_transfer: true,
            reboot: 0,
            wipe: [false; WIPES.len()],
            cache_partition: false,
            phone_logs: 0,
            pattern: Vec::new(),
            confirm: 0.0,
            install: Install::Idle,
            source: Source::Computer,
            package_name: None,
            install_share: 0.0,
            install_rate: 0.0,
            install_log: Vec::new(),
            root: None,
            script_output: Vec::new(),
            check_signature: false,
            recovery_partitions: false,
            keep_recovery: true,
            reflash: None,
            mounts: Ok(Vec::new()),
            mount_failure: None,
            backup: [true, true, false, true],
            backup_to: Place::Storage,
            compress: false,
            verify: true,
            backups: Vec::new(),
            restore: None,
            backup_job: None,
            logs: [true, true],
            logs_to: 0,
            boot_slots: None,
            next_slot: None,
            dual_boot: false,
            updates: true,
            modules: None,
            module_at: None,
            apps: None,
            app_at: None,
            app_icons: Vec::new(),
            module_page: 0,
            app_page: 0,
            apps_scope: AppsScope::User,
            sys_apps: None,
            sys_page: 0,
            sys_at: None,
            app_page_taps: 0,
            app_arrivals: 0,
            page_tap: None,
            page_miss: None,
            page_pending: None,
            pages_shown: 0,
            app_confirm: None,
            module_confirm: None,
            place_ask: None,
            systems: None,
            system_pick: None,
            systems_work: None,
            system_notice: None,
            data: Data::Closed,
            data_phase: 0.0,
            data_reported: 0.0,
            data_progress: 0.0,
            prepared: false,
            code: String::new(),
            search: String::new(),
            search_scope: SearchScope::Features,
            layer: Layer::Letters,
            pinyin: Pinyin::default(),
            record: Record::default(),
            held: None,
            files: Folder::new("/"),
            otg: Folder::new(""),
            asking_remember: false,
            file_on_drive: false,
            date: String::new(),
            sort: SortKey::Name,
            sort_desc: false,
            folders_first: true,
            storage: Folder::new(""),
            usb_drive: None,
            drive: Folder::new(""),
            file: None,
            fastboot: Fastboot { running: false, link: UsbLink::Unplugged },
            erase: Erase::default(),
            asked: Asked::default(),
            clip: None,
            new_name: String::new(),
            editor: None,
            asking_delete: false,
            avatar: None,
            device_name: None,
            choosing: false,
            chosen: std::collections::BTreeSet::new(),
            asking_many: None,
            clash: None,
            job: None,
            relabel: None,
            filesystems: None,
            fs_chosen: 0,
            fs_action: FsAction::Repair,
            part_work: None,
            update: Standing::None,
            wlan: None,
            remember: false,
            serving: Serving::Off,
            picking: false,
            avb: None,
            avb_pending: None,
            avb_notice: None,
            vbmeta_kept: None,
            vbmeta_note: None,
            vbmeta_choice: None,
            fetch_url: String::new(),
            fetch: Fetch::Idle,
            fetch_portal: None,
            wlan_auto: false,
            wlan_keep: false,
            joining: None,
            details: None,
            adding: None,
            password: String::new(),
            show_password: false,
            update_server: false,
            check: Check::Idle,
            have: Have::Nothing,
            download: None,
            skyward_check: SkywardCheck::Idle,
            skyward_job: None,
            traffic: Traffic::default(),
        }
    }
}

impl State {
    /// The key of the on-screen keyboard a finger holds down.
    /// 拼音候选的全部、每一页几个、当前页（已经夹在范围内）。
    pub fn pinyin_view(&self) -> (Vec<crate::pinyin::Pick>, usize) {
        let picks = crate::pinyin::options(&self.pinyin.composing);
        let per = crate::pinyin::PER_PAGE;
        let pages = picks.len().div_ceil(per).max(1);
        (picks, pages)
    }

    /// 这一页要显示的候选，最多 `PER_PAGE` 个。
    pub fn pinyin_page(&self) -> Vec<String> {
        let (picks, pages) = self.pinyin_view();
        let per = crate::pinyin::PER_PAGE;
        let at = self.pinyin.page.min(pages - 1);
        picks.iter().skip(at * per).take(per).map(|p| p.text.clone()).collect()
    }

    pub fn key_held(&self) -> Option<Cap> {
        match self.held {
            Some(Action::Type(cap)) => Some(cap),
            _ => None,
        }
    }

    /// The folder the list on page `screen` shows: the file manager's, or
    /// a place's to choose a file to install in.
    pub fn folder(&self, screen: Screen) -> &Folder {
        match screen {
            Screen::Files => &self.files,
            Screen::FilesOtg => &self.otg,
            Screen::Choose(Place::Storage) => &self.storage,
            Screen::Choose(Place::UsbDrive) => &self.drive,
            other => unreachable!("{other:?} lists no folder"),
        }
    }

    pub(super) fn folder_mut(&mut self, screen: Screen) -> &mut Folder {
        match screen {
            Screen::Files => &mut self.files,
            Screen::FilesOtg => &mut self.otg,
            Screen::Choose(Place::Storage) => &mut self.storage,
            Screen::Choose(Place::UsbDrive) => &mut self.drive,
            other => unreachable!("{other:?} lists no folder"),
        }
    }

    /// Whether the files of `place` can be read: internal storage once it
    /// is open, a USB drive once the runtime shows it.
    /// The folder the page shows, as the phone knows it: the OTG page keeps
    /// the folder it shows from the top of the drive, and the file manager the
    /// whole of the path.
    pub fn folder_here(&self, screen: Screen) -> String {
        match screen {
            Screen::FilesOtg => {
                let at = self.otg.path.trim_matches('/');
                match at.is_empty() {
                    true => super::USB_DRIVE.to_string(),
                    false => format!("{}/{}", super::USB_DRIVE, at),
                }
            }
            // A page that lists no folder has nothing to paste into.
            other => match other {
                Screen::Files => self.files.path.clone(),
                Screen::Choose(Place::Storage) => self.storage.path.clone(),
                Screen::Choose(Place::UsbDrive) => self.drive.path.clone(),
                _ => String::new(),
            },
        }
    }

    pub fn place_open(&self, place: Place) -> bool {
        match place {
            Place::Storage => self.data.storage_open(),
            Place::UsbDrive => self.usb_drive == Some(UsbDrive::Shown),
        }
    }

    /// Whether a network is joined and has given an address.
    pub fn online(&self) -> bool {
        matches!(&self.wlan, Some(Wlan { joined: Some(j), .. }) if matches!(j.stage, Joining::Connected(_)))
    }

    /// The newest system the update server told of.
    pub fn newest(&self) -> Option<&Release> {
        match &self.check {
            Check::Answered(Some(r)) => Some(r),
            _ => None,
        }
    }

    /// Whether the full package is being downloaded.
    pub fn downloading(&self) -> bool {
        self.download.as_ref().is_some_and(|d| d.outcome.is_none())
    }

    /// Skyward's newest release, as GitHub told of it.
    pub fn skyward_newest(&self) -> Option<&Newest> {
        match &self.skyward_check {
            SkywardCheck::Answered(n) => Some(n),
            _ => None,
        }
    }

    /// Whether Skyward is being updated.
    pub fn updating_skyward(&self) -> bool {
        self.skyward_job.as_ref().is_some_and(|j| j.outcome.is_none())
    }

    /// Whether the phone has the page `screen`: the WLAN page where the
    /// recovery brings the phone's WLAN up, and updating Skyward where the
    /// phone also keeps its recovery in a partition of each slot.
    /// The apps read for the page, or why they could not be.
    pub fn set_apps(&mut self, list: Result<Vec<crate::app::dataapp::DataApp>, String>) {
        self.apps = Some(list);
    }

    /// The modules read for the page, or why they could not be.
    pub fn set_modules(&mut self, rows: Result<Vec<ModuleRow>, String>) {
        self.modules = Some(rows);
    }

    pub fn offers(&self, screen: Screen) -> bool {
        match screen {
            Screen::Wlan => self.wlan.is_some(),
            Screen::SkywardUpdate => self.updates && self.wlan.is_some() && self.recovery_partitions,
            Screen::Systems => self.dual_boot,
            _ => true,
        }
    }
}
