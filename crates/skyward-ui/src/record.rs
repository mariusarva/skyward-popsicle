//! 录屏：把页面画出来的帧缩到选好的大小、转成 YUV420、用 openh264 编成
//! H.264 Baseline（CAVLC），装进 MP4 —— `avc1` + `avcC` 的标准写法，一段录屏
//! 一个文件，后缀 `.mp4`。
//!
//! 为什么不是 MJPEG：2026-11-21 的真机包（v106）录出来的 `mp4v` + `esds`
//! 对象类型 0x6C，ffmpeg 与 VLC 放得开，**相册一打开就说文件损坏**。原因是
//! Android 的解析器不看 `esds` 里的对象类型，`mp4v` 这个 4CC 被
//! `FourCC2MIME` 硬映射成 `video/mpeg4`（MPEG-4 Part 2），于是系统拿
//! `c2.android.mpeg4.decoder` 去解 JPEG，第一帧就返回 `ERROR_MALFORMED`。
//! 相册没有 MJPEG 解码器，换容器写法也没用，只能换码流。
//!
//! recovery 里没有 MediaCodec，也没有 ffmpeg，能用的只有 CPU，所以 H.264
//! 编码器是静态链进来的 openh264（`native/h264_shim.cpp` 是它的 C 包装）。
//! 一帧 1200×2608 的 RGBA 是 12.5 MB，几个线程各自缩放、转 YUV、编码，另有
//! 一个线程按时间轴把编好的帧写进文件。
//!
//! 三个地方是 2026-11-21 那次录屏踩出来的（`Screenrecord_2026-11-21-10-45-57.avi`
//! 有 118 秒是一张不动的图）：
//!
//! * 写线程一味按时间轴补帧，追不上就一直补，越补越落后，最后到收尾才把
//!   几百兆的重复帧一口气倒出来。现在落后超过两格就把时间轴往前挪，不再补。
//! * 编码线程和写线程之间是一条只有几格的通道，写盘被安装抢了 I/O 的时候
//!   编码线程堵在 `send` 上，队列满了页面就开始丢帧，一丢就再也没恢复。
//!   现在编码结果先收下、只留最新的那一帧，谁也不堵。
//! * 一帧 1200×2608 在三个线程上只编得出 16 帧/秒，30 帧/秒的时间轴只好
//!   靠补帧填。现在默认按面板的一半录（600×1304），编码有余量，画面真在动。
//! * 录屏的帧率一直跟着页面重绘走：页面不动就没有帧。文件因此是变帧率的
//!   —— 设定 30 帧，一段安静的录屏平均只有 17 帧，开头还留着两个空洞。
//!   现在编码线程自己按 `1/fps` 出帧：这一格没有新画面就再编一遍上一帧
//!   （P 帧整帧 skip，只有几十字节），文件是恒定帧率。
//!
//! 文件本身是标准的 MP4：`ftyp` + `mdat`（一帧一个 sample，AVCC 长度前缀）
//! + 收尾的 `moov`（`stsd` 里 `avc1` + `avcC`，`stss` 记着哪些是 IDR，
//! sample 表一秒一个 chunk，时基 90 kHz）。ffmpeg、VLC、**手机相册**都放得开。

use anyhow::{Context, Result};
use std::ffi::c_void;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::canvas::Canvas;

/// 分辨率选项，乘在面板的宽高上：原画、四分之三、一半、八分之三。
pub const SCALES: [(u32, u32); 4] = [(1, 1), (3, 4), (1, 2), (3, 8)];

/// 画质选项。以前这是 JPEG 的质量，现在是每像素给多少比特（见
/// [`Settings::bitrate`]）：88 高、75 中、60 低。
pub const QUALITIES: [u8; 3] = [88, 75, 60];

/// 帧率选项，帧每秒。
pub const RATES: [u32; 4] = [60, 30, 24, 15];

/// 默认挑的那一档：一半、中、30 帧。600×1304 在 30 帧上目标码率约 3.8 Mbps，
/// 每分钟约 28 MB（MJPEG 那套是 90 MB）。
pub const DEFAULT_SCALE: usize = 2;
pub const DEFAULT_QUALITY: usize = 1;
pub const DEFAULT_FPS: usize = 1;

/// 最多在内存里攒几帧等编码：再多就是攒垃圾，宁可丢。
const POOL: usize = 6;

/// MP4 的 `stco` 是 32 位偏移，写到这里就停，免得到 4 GB 处把文件写坏。
const MOST_BYTES: u64 = 3_900_000_000;

/// 录一段用的大小、画质与帧率。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub fps: u32,
}

impl Settings {
    /// 面板 `panel` 上的第 `scale` 档大小，第 `quality` 档画质，第 `fps` 档帧率。
    pub fn of(panel: (u32, u32), scale: usize, quality: usize, fps: usize) -> Settings {
        let (n, d) = SCALES[scale.min(SCALES.len() - 1)];
        let even = |v: u32| ((v / 2) * 2).max(2);
        Settings {
            width: even((panel.0 * n + d / 2) / d),
            height: even((panel.1 * n + d / 2) / d),
            quality: QUALITIES[quality.min(QUALITIES.len() - 1)],
            fps: RATES[fps.min(RATES.len() - 1)],
        }
    }

    /// 这个大小是多少像素。
    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// 目标码率，每像素给多少比特。录屏的画面大半是不动的，H.264 靠帧间
    /// 预测能把静止部分压得很小，所以同样的画质比 MJPEG 省七成。
    ///
    /// 0.10 / 0.16 / 0.28 bit 每像素每帧是实测：600×1304 @ 30 帧，低档 2.3
    /// Mbps、中档 3.8 Mbps、高档 6.6 Mbps；原画 1200×2608 @ 30 帧高档 26 Mbps。
    pub fn bitrate(&self) -> u32 {
        let bits = match self.quality {
            0..=65 => 0.10,
            66..=80 => 0.16,
            _ => 0.28,
        };
        let raw = (self.pixels() as f64 * self.fps as f64 * bits) as u64;
        raw.clamp(800_000, 48_000_000) as u32
    }

    /// 一帧大约多大，按目标码率算 —— 页面拿它估「每分钟多少 MB」。
    pub fn per_frame(&self) -> u64 {
        self.bitrate() as u64 / 8 / self.fps.max(1) as u64
    }
}

// ---------------------------------------------------------------- openh264

/// shim 里的编码器。见 `native/h264_shim.cpp`。
struct H264 {
    raw: *mut c_void,
}

// 一个实例只被一条线程用，`Send` 是安全的（里面全是 openh264 自己的堆）。
unsafe impl Send for H264 {}

unsafe extern "C" {
    fn sky_h264_new(width: i32, height: i32, fps: i32, bitrate: i32) -> *mut c_void;
    fn sky_h264_free(h: *mut c_void);
    fn sky_h264_encode(
        h: *mut c_void,
        y: *const u8,
        u: *const u8,
        v: *const u8,
        stride_y: i32,
        stride_uv: i32,
        timestamp_ms: i64,
        out: *mut u8,
        cap: i32,
        key: *mut i32,
    ) -> i32;
    fn sky_h264_parameter_sets(
        h: *mut c_void,
        sps: *mut u8,
        sps_cap: i32,
        sps_len: *mut i32,
        pps: *mut u8,
        pps_cap: i32,
        pps_len: *mut i32,
    ) -> i32;
    fn sky_h264_force_idr(h: *mut c_void) -> i32;
}

impl H264 {
    fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<H264> {
        let raw = unsafe { sky_h264_new(width as i32, height as i32, fps as i32, bitrate as i32) };
        if raw.is_null() {
            anyhow::bail!("the H.264 encoder would not start ({width}×{height} @ {fps} fps)");
        }
        Ok(H264 { raw })
    }

    /// 编一帧 YUV420。返回 AVCC 字节流（4 字节长度 + NAL）与「是不是 IDR」；
    /// 空的那一帧是编码器按码率丢的，`None` 就是。
    fn encode(&mut self, yuv: &Yuv420, ts_ms: i64, out: &mut Vec<u8>) -> Result<Option<(bool, usize)>> {
        let need = yuv.y.len() + 4096;
        if out.len() < need {
            out.resize(need, 0);
        }
        let mut key = 0i32;
        let n = unsafe {
            sky_h264_encode(
                self.raw,
                yuv.y.as_ptr(),
                yuv.u.as_ptr(),
                yuv.v.as_ptr(),
                yuv.stride_y as i32,
                yuv.stride_uv as i32,
                ts_ms,
                out.as_mut_ptr(),
                out.len() as i32,
                &mut key,
            )
        };
        if n < 0 {
            anyhow::bail!("the H.264 encoder rejected a frame");
        }
        if n == 0 {
            return Ok(None);
        }
        Ok(Some((key == 1, n as usize)))
    }

    /// 第一帧编完之后才有。返回 (SPS, PPS) 的裸 NAL。
    fn parameter_sets(&mut self) -> Option<(Vec<u8>, Vec<u8>)> {
        let mut sps = [0u8; 256];
        let mut pps = [0u8; 256];
        let (mut sl, mut pl) = (0i32, 0i32);
        let ok = unsafe {
            sky_h264_parameter_sets(
                self.raw,
                sps.as_mut_ptr(),
                sps.len() as i32,
                &mut sl,
                pps.as_mut_ptr(),
                pps.len() as i32,
                &mut pl,
            )
        };
        if ok != 1 || sl <= 0 || pl <= 0 {
            return None;
        }
        Some((sps[..sl as usize].to_vec(), pps[..pl as usize].to_vec()))
    }

    /// 让下一帧编成 IDR。预热帧占掉了初始化那次 IDR，第一帧真实画面得再要
    /// 一个，否则它编成 P 帧，`avcC` 里就没有 SPS/PPS。
    fn force_idr(&mut self) -> bool {
        unsafe { sky_h264_force_idr(self.raw) == 1 }
    }
}

impl Drop for H264 {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { sky_h264_free(self.raw) };
        }
    }
}

/// 一帧 YUV420 平面。宽高都是偶数，stride 就是宽。
struct Yuv420 {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
    stride_y: usize,
    stride_uv: usize,
}

impl Yuv420 {
    fn new(width: u32, height: u32) -> Yuv420 {
        let w = width as usize;
        let h = height as usize;
        Yuv420 {
            y: vec![0u8; w * h],
            u: vec![128u8; (w / 2) * (h / 2)],
            v: vec![128u8; (w / 2) * (h / 2)],
            stride_y: w,
            stride_uv: w / 2,
        }
    }
}

/// 一帧装进哪种容器。这里只有 MP4：里面放的是 H.264 Baseline 基本流
/// （`avc1` + `avcC`）。AVI 那套容器连同它的索引代码已整段删掉，文件名后缀、
/// 规格行、日志都只认 `.mp4`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Mp4,
}

impl Container {
    /// 这一段唯一的那种容器。
    pub const ONLY: Container = Container::Mp4;

    /// 文件名的后缀，不带点。
    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
        }
    }

    /// 页面上与规格行里的叫法：容器加上里面的码流。
    pub fn label(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4 (H.264)",
        }
    }

    /// 只报容器时的短名字。
    pub fn short(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4",
        }
    }

    /// 规格行尾巴上那句。
    pub fn spec(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4 (H.264)",
        }
    }
}

/// 一个等着写的东西：已经打开的文件和它的路径。
pub struct Take {
    pub file: File,
    pub path: PathBuf,
}

/// 一段录好的东西：写到哪个文件、多少帧、多少字节。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub path: PathBuf,
    pub frames: u64,
    pub bytes: u64,
    /// 时长，按帧率算。
    pub millis: u64,
    /// 没写进文件的帧：到了容器上限之后剩下的那些。
    pub dropped: u64,
}

impl Summary {
    /// 写出来的文件。
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 交给页面的名字，就是文件名。
    pub fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// 写了多少帧。
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// 文件有多大。
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// 正在录的一段。放下它而不 [`Recorder::finish`]，写线程会把它收到一半的
/// 文件补成能放的 MP4 再退出。
pub struct Recorder {
    /// 页面递来的最新一帧，最多一份。页面按屏幕刷新率画（120 Hz），编码只
    /// 按 `1/fps` 取；中间那些被后一张顶掉，还回池子，不排队。
    mail: Arc<(Mutex<Option<Job>>, Condvar)>,
    /// 暂停与继续交给写线程自己记：时间轴在它手上，只有它能算准暂停了多久。
    ctl: Sender<bool>,
    /// 暂停时页面连画布都不抄：一帧十二兆，抄了也是丢。
    paused: Arc<AtomicBool>,
    pool: Arc<Mutex<Vec<Vec<u8>>>>,
    writer: Option<JoinHandle<Result<Summary>>>,
    encoder: Option<JoinHandle<()>>,
    /// 收尾：置真之后编码线程交出手里这一格就退出，写线程跟着补索引。
    stop: Arc<AtomicBool>,
    source: (u32, u32),
    size: (u32, u32),
    path: PathBuf,
    /// 编码器建好、预热过了没有。建 openh264 要几百毫秒，这期间收到的帧
    /// 只会把时间轴撑开 —— 页面照画，帧丢掉，等它就绪再开始记。
    ready: Arc<AtomicBool>,
}

struct Job {
    /// 采集这一帧的时刻。
    at: Instant,
    rgba: Vec<u8>,
}

/// 等下一格，或者等页面递来新的一帧：`push` 会 `notify_one` 把它提前叫醒。
fn wait_for(mail: &(Mutex<Option<Job>>, Condvar), dur: Duration) {
    if let Ok(slot) = mail.0.lock() {
        let _ = mail.1.wait_timeout(slot, dur);
    }
}

impl Recorder {
    /// 在已经打开的 `take` 上开始录。`source` 是页面画布的大小，`settings`
    /// 是录出来的大小、画质与帧率。容器只有 MP4 一种，一段录屏一个文件。
    pub fn start(take: Take, source: (u32, u32), settings: Settings) -> Result<Recorder> {
        let fps = settings.fps.max(1);
        let (w, h) = (settings.width, settings.height);
        let bitrate = settings.bitrate();
        let mp4 = Mp4::new(take.file, take.path, w, h, fps).context("write the take's header")?;
        let path = mp4.path();
        // 只能有一条编码线程。H.264 的 P 帧引的是「同一个编码器实例的上一帧」，
        // 两条线程各编各的、写线程再按到达顺序混着写，解码器看到的就是两条
        // 互不相干的参考链 —— ffmpeg 报 `reference picture missing during
        // reorder`，相册黑屏。MJPEG 那套一帧一张图，才可以拆开并行。
        //
        // 单实例在 600×1304 上有 30 帧/秒的余量（arm64 带 NEON）；真跟不上
        // 也没关系，写线程按采集时刻摊帧间隔，文件帧率低一点，时长不变。
        let pool: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let sets: Arc<Mutex<(Vec<u8>, Vec<u8>)>> = Arc::new(Mutex::new((Vec::new(), Vec::new())));
        let ready = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        // 编好的帧交给写线程；通道短，写盘抖一下就等一等，不把内存堆起来。
        let (done_tx, done_rx) = mpsc::sync_channel::<(Instant, Vec<u8>, bool)>(8);
        let (ctl, ctl_rx) = mpsc::channel::<bool>();
        let paused = Arc::new(AtomicBool::new(false));
        // 页面递来的最新一帧，最多一份：画得比编码快，旧的被新的顶掉。
        let mail: Arc<(Mutex<Option<Job>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
        let xs = spans(source.0, w);
        let ys = spans(source.1, h);
        let gap = Duration::from_nanos(1_000_000_000 / fps as u64);
        let (sw, sh) = source;
        let encoder = {
            let mail = Arc::clone(&mail);
            let pool = Arc::clone(&pool);
            let sets = Arc::clone(&sets);
            let ready = Arc::clone(&ready);
            let stop = Arc::clone(&stop);
            let paused = Arc::clone(&paused);
            std::thread::Builder::new()
                .name("record encode".into())
                .spawn(move || {
                    let mut h264 = match H264::new(w, h, fps, bitrate) {
                        Ok(e) => e,
                        Err(e) => {
                            let _ = skyward_kmsg(&format!("record: the encoder did not start: {e:#}"));
                            return;
                        }
                    };
                    let mut rgb = Vec::new();
                    let mut yuv = Yuv420::new(w, h);
                    let mut out = Vec::with_capacity((w as usize * h as usize) / 4 + 4096);
                    let mut got_sets = false;
                    // 预热：先编一帧黑，把 openh264 的表、参考帧、码率控制都
                    // 建起来。这一帧不进文件 —— 真机上第一帧编了 638 ms，
                    // 写进时间轴就是开头一个半秒的空洞。
                    let warm = Yuv420::new(w, h);
                    let mut warm_out = Vec::with_capacity((w as usize * h as usize) / 4 + 4096);
                    if h264.encode(&warm, 0, &mut warm_out).is_err() {
                        let _ = skyward_kmsg("record: the encoder would not warm up");
                    }
                    warm_out.clear();
                    // 预热占掉了初始化那次 IDR，第一帧真实画面要重新要一个。
                    let _ = h264.force_idr();
                    // 两张 1.2 MB 的图用完就还，别陪跑一整段录像。
                    drop(warm);
                    drop(warm_out);
                    ready.store(true, Ordering::Relaxed);

                    // 从这一刻起自己按 `1/fps` 出帧，不跟页面的重绘走。页面这一格
                    // 没画新东西，就再编一遍上一帧 —— P 帧整帧 skip，只有几十字节，
                    // 换来的是恒定帧率：设的 30 帧，文件里就是 30 帧，安静的一段
                    // 也是 30 帧，不会掉到十几帧。
                    let mut next = Instant::now();
                    let mut have = false;
                    loop {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        // 取页面递来的最新一帧（有就换掉手里这张，并转成 YUV）。
                        let fresh = mail.0.lock().ok().and_then(|mut slot| slot.take());
                        if let Some(job) = fresh {
                            shrink(&job.rgba, sw as usize, sh as usize, &xs, &ys, &mut rgb);
                            to_i420(&rgb, w as usize, h as usize, &mut yuv);
                            have = true;
                            // 缓冲还回池子：每帧 12 MB，不能每帧都向系统要。
                            if let Ok(mut pool) = pool.lock()
                                && pool.len() < POOL
                            {
                                pool.push(job.rgba);
                            }
                        }
                        if !have {
                            // 第一帧还没来（页面还没画完）：等它，别空转。
                            wait_for(&mail, Duration::from_millis(200));
                            continue;
                        }
                        let now = Instant::now();
                        if paused.load(Ordering::Relaxed) {
                            // 暂停：不出帧，时间轴也不走；继续之后从当下重新起算。
                            next = now + gap;
                            wait_for(&mail, gap);
                            continue;
                        }
                        if now < next {
                            wait_for(&mail, next - now);
                            continue;
                        }
                        // 参数集还没出来，说明手里的都是 P 帧：让这一帧编成 IDR。
                        if !got_sets {
                            let _ = h264.force_idr();
                        }
                        // 这一格的时间：按点出帧就记原定的那一格（`next` 是整格
                        // 累加出来的，`stts` 因此是干净的 3000）；编码迟到了就按
                        // 当下算，差的这一点写进 `stts`，时长才对得上表。
                        let at = if now.saturating_duration_since(next) <= gap / 2 { next } else { now };
                        match h264.encode(&yuv, 0, &mut out) {
                            Ok(Some((key, len))) => {
                                if !got_sets && let Some((sps, pps)) = h264.parameter_sets() {
                                    got_sets = true;
                                    if let Ok(mut slot) = sets.lock() {
                                        *slot = (sps, pps);
                                    }
                                }
                                // 写线程不再收新的也没关系：这一段到此为止。
                                if done_tx.send((at, out[..len].to_vec(), key)).is_err() {
                                    break;
                                }
                            }
                            Ok(None) => {
                                // 编码器自己按码率省了这一帧：不送，下一格的间隔
                                // 照算，时长不变。
                            }
                            Err(e) => {
                                let _ = skyward_kmsg(&format!("record: a frame was not encoded: {e:#}"));
                            }
                        }
                        next = at + gap;
                    }
                })
                .context("start a thread to encode the frames")?
        };
        let writer = std::thread::Builder::new()
            .name("record mux".into())
            .spawn(move || mux(mp4, done_rx, ctl_rx, fps, sets))
            .context("start the thread that writes the file")?;
        Ok(Recorder {
            mail,
            ctl,
            paused,
            pool,
            writer: Some(writer),
            encoder: Some(encoder),
            stop,
            source,
            size: (w, h),
            path,
            ready,
        })
    }

    /// 这一段写到哪个文件。
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 录出来的画面有多宽多高。
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// 暂停或继续这一段。暂停期间页面不再抄画布，写线程也不写格子；继续之后
    /// 时间轴从停下的地方接上，文件里不会有空档，也不会有停住的一张图。
    pub fn set_paused(&self, on: bool) {
        self.paused.store(on, Ordering::Relaxed);
        let _ = self.ctl.send(on);
    }

    /// 页面画完一帧就叫一次：画布整块抄进回收来的缓冲，放进「最新一帧」那一格。
    /// 抄一块内存比逐像素去掉 alpha 快一个数量级，缩放、转 YUV 与编码都在编码
    /// 线程上做，页面这一下只花两三毫秒。
    pub fn push(&self, canvas: &Canvas) {
        // 编码器还在建（openh264 初始化几百毫秒）：这一阵收到的帧一律不记，
        // 免得第一帧进去了、后面几百毫秒全堵在通道里，时间轴上留个大洞。
        if !self.ready.load(Ordering::Relaxed) {
            return;
        }
        if self.paused.load(Ordering::Relaxed) {
            return;
        }
        let (w, h) = (canvas.w, canvas.h);
        if (w, h) != self.source || canvas.data.len() != (w as usize) * (h as usize) * 4 {
            return;
        }
        let (lock, cvar) = &*self.mail;
        // 编码线程还没取走上一张：它马上要编的就是那张，这一张连抄都不必
        // 抄。屏是 120 帧/秒，编码只取 30 张，四分之三的抄写本来就会被顶掉
        // —— 一帧十二兆，白抄三遍就是白占一半内存带宽，页面还要等它
        // （2026-11-21）。格子空着才抄，抄完再放；这中间编码线程取走的可能
        // 性由下面那次 `replace` 兜住。
        match lock.lock() {
            Ok(slot) if slot.is_some() => return,
            Ok(_) => {}
            Err(_) => return,
        }
        let mut job = self.spare(canvas.data.len());
        job.rgba.copy_from_slice(&canvas.data);
        job.at = Instant::now();
        let Ok(mut slot) = lock.lock() else {
            return;
        };
        // 上一张还没编就被这一张顶掉：还回池子，不排队。页面是跟着屏幕刷新率
        // 画的（这块屏 120 Hz），编码只按 1/fps 取，中间那些本来就不该进文件。
        if let Some(old) = slot.replace(job)
            && let Ok(mut pool) = self.pool.lock()
            && pool.len() < POOL
        {
            pool.push(old.rgba);
        }
        cvar.notify_one();
    }

    /// 从池子里拿一个够大的缓冲，没有就新开一个。
    fn spare(&self, len: usize) -> Job {
        let mut rgba = self.pool.lock().ok().and_then(|mut pool| pool.pop()).unwrap_or_default();
        if rgba.len() != len {
            rgba = vec![0u8; len];
        }
        Job { at: Instant::now(), rgba }
    }

    /// 收尾：不再收帧，等编码线程交出手里这一格，写线程把索引补上。
    pub fn finish(mut self) -> Result<Summary> {
        self.stop.store(true, Ordering::Relaxed);
        self.mail.1.notify_all();
        if let Some(e) = self.encoder.take() {
            let _ = e.join();
        }
        match self.writer.take() {
            Some(w) => w.join().map_err(|_| anyhow::anyhow!("the thread that writes the file panicked"))?,
            None => anyhow::bail!("the file was already written"),
        }
    }
}

impl Drop for Recorder {
    /// 页面没叫 `finish` 就把它丢了：叫编码线程收手，它一走写线程就收到通道
    /// 关闭、把索引补上。这里不等线程结束 —— 丢下它的地方可能是页面的收尾
    /// 路径，等下去只会更难收拾。
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.mail.1.notify_all();
    }
}

/// 一帧 RGBA 缩成 `xs`×`ys` 那样多格，每格取平均数，写成 RGB。文字缩小时
/// 取平均比丢点清楚得多：丢点会让笔画断，平均只是糊。
fn shrink(src: &[u8], sw: usize, _sh: usize, xs: &[(u32, u32)], ys: &[(u32, u32)], out: &mut Vec<u8>) {
    let (dw, dh) = (xs.len(), ys.len());
    out.clear();
    out.resize(dw * dh * 3, 0);
    if xs.iter().all(|&(_, n)| n == 1) && ys.iter().all(|&(_, n)| n == 1) {
        // 原画：只去掉 alpha，按下标写，比逐像素 extend 快。
        let mut at = 0;
        for px in src.chunks_exact(4) {
            out[at] = px[0];
            out[at + 1] = px[1];
            out[at + 2] = px[2];
            at += 3;
        }
        return;
    }
    let mut at = 0;
    for &(y0, ny) in ys {
        for &(x0, nx) in xs {
            let mut sum = [0u32; 3];
            for y in y0..y0 + ny {
                let row = ((y as usize) * sw + x0 as usize) * 4;
                for x in 0..nx as usize {
                    let p = row + x * 4;
                    sum[0] += src[p] as u32;
                    sum[1] += src[p + 1] as u32;
                    sum[2] += src[p + 2] as u32;
                }
            }
            let n = (nx * ny).max(1);
            out[at] = (sum[0] / n) as u8;
            out[at + 1] = (sum[1] / n) as u8;
            out[at + 2] = (sum[2] / n) as u8;
            at += 3;
        }
    }
}

/// RGB 转 YUV420：Y 逐像素，U/V 由 2×2 平均之后算。系数是 BT.601 的整数近似，
/// 手机播放器、相册、ffmpeg 都按这个解。
fn to_i420(rgb: &[u8], w: usize, h: usize, out: &mut Yuv420) {
    for row in 0..h {
        let src = row * w * 3;
        let dst = row * w;
        for col in 0..w {
            let p = src + col * 3;
            let (r, g, b) = (rgb[p] as i32, rgb[p + 1] as i32, rgb[p + 2] as i32);
            out.y[dst + col] = ((77 * r + 150 * g + 29 * b) >> 8) as u8;
        }
    }
    let cw = w / 2;
    for row in 0..(h / 2) {
        let r0 = row * 2 * w * 3;
        let r1 = r0 + w * 3;
        for col in 0..cw {
            let p0 = r0 + col * 6;
            let p1 = r1 + col * 6;
            let mut r = 0i32;
            let mut g = 0i32;
            let mut b = 0i32;
            for p in [p0, p0 + 3, p1, p1 + 3] {
                r += rgb[p] as i32;
                g += rgb[p + 1] as i32;
                b += rgb[p + 2] as i32;
            }
            let (r, g, b) = (r / 4, g / 4, b / 4);
            let at = row * cw + col;
            out.u[at] = (((-43 * r - 85 * g + 128 * b) >> 8) + 128).clamp(0, 255) as u8;
            out.v[at] = (((128 * r - 107 * g - 21 * b) >> 8) + 128).clamp(0, 255) as u8;
        }
    }
}

/// 目标 `dst` 每一格取源 `src` 的哪一段：起点与格数。
fn spans(src: u32, dst: u32) -> Vec<(u32, u32)> {
    let dst = dst.max(1);
    (0..dst)
        .map(|d| {
            let a = (d as u64 * src as u64 / dst as u64) as u32;
            let b = (((d as u64 + 1) * src as u64 + dst as u64 - 1) / dst as u64) as u32;
            let b = b.max(a + 1).min(src);
            (a, b - a)
        })
        .collect()
}

/// 写线程：收到一帧就写一帧，样本的时长按**这一格的时间**算。
///
/// 编码线程按 `1/fps` 出帧，所以正常情况每一帧都是标称时长；编码慢了一格、
/// 时间轴跳过一格时，这一帧的时长就是两格 —— 时长照样对得上手上的表。
///
/// 收尾时补 `moov`，`avcC` 里的 SPS/PPS 由编码线程填在 `sets` 里。
fn mux(
    mut mp4: Mp4,
    rx: Receiver<(Instant, Vec<u8>, bool)>,
    ctl: Receiver<bool>,
    fps: u32,
    sets: Arc<Mutex<(Vec<u8>, Vec<u8>)>>,
) -> Result<Summary> {
    // 一帧的标称时长，90 kHz 的刻度。
    let nominal = (90_000u64 / fps.max(1) as u64).max(1) as u32;
    // 单帧最多占 30 秒，免得暂停之后那一格把时间轴撑爆。
    let cap = 90_000u32 * 30;
    let mut closed = false;
    let mut full = false;
    let mut paused = false;
    let mut pauses = 0u64;
    let mut last_at: Option<Instant> = None;
    let mut left_out = 0u64;
    loop {
        while let Ok(on) = ctl.try_recv() {
            if on != paused {
                paused = on;
                if !paused {
                    // 继续：上一帧的时刻作废，下一帧按标称时长接上，
                    // 暂停的那一段不进时间轴。
                    last_at = None;
                    pauses += 1;
                }
            }
        }
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok((at, bytes, key)) => {
                let delta = match last_at {
                    Some(prev) => {
                        let secs = at.saturating_duration_since(prev).as_secs_f64();
                        ((secs * 90_000.0).round() as i64).clamp(1, cap as i64) as u32
                    }
                    None => nominal,
                };
                if mp4.bytes() >= MOST_BYTES {
                    if !full {
                        full = true;
                        let _ = skyward_kmsg(
                            "record: the take is as big as its container allows; the frames after this are left out",
                        );
                    }
                    left_out += 1;
                } else {
                    mp4.sample(&bytes, key, delta)?;
                    last_at = Some(at);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => closed = true,
        }
        if closed {
            break;
        }
    }
    let path = mp4.path();
    let (sps, pps) = sets.lock().map(|s| s.clone()).unwrap_or_default();
    let frames = mp4.frames();
    let millis = mp4.millis();
    let bytes = mp4.finish(&sps, &pps)?;
    let _ = skyward_kmsg(&format!(
        "record: {frames} frames, {:.1} MB in {} · {}",
        bytes as f64 / 1e6,
        path.display(),
        Container::ONLY.label(),
    ));
    let _ = skyward_kmsg(&format!(
        "record: stopped after {frames} frames, {:.1} s, {:.1} MB, {:.1} MB/s ({left_out} frames left out, {pauses} pauses)",
        millis as f64 / 1000.0,
        bytes as f64 / 1e6,
        bytes as f64 / 1e6 / (millis as f64 / 1000.0).max(0.001),
    ));
    Ok(Summary { path, frames, bytes, millis, dropped: left_out })
}

/// 打一条内核日志。`skyward-sys` 不在这个 crate 的依赖里（UI 要在主机上也能
/// 编），所以录屏自己写 `/dev/kmsg`，写不了就算了。
fn skyward_kmsg(line: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new().write(true).open("/dev/kmsg")?;
    writeln!(f, "<6>skyward: {line}")
}

/// MP4 文件：`ftyp` + `mdat`（一帧一个 AVCC sample，直接贴着写）+ 收尾的
/// `moov`。`mdat` 的长度字段先占住，写完回来补；sample 表在收尾时一次写出，
/// 那时候每一帧的偏移才全部定下来。
struct Mp4 {
    out: BufWriter<File>,
    path: PathBuf,
    width: u32,
    height: u32,
    fps: u32,
    /// `mdat` 盒子的起点，收尾回来补长度。
    mdat_at: u64,
    /// 第一个 sample 的字节位置。
    data_at: u64,
    pos: u64,
    /// 每一帧有多少字节，`stsz` 与 chunk 偏移都靠它。
    sizes: Vec<u32>,
    /// 哪几帧是 IDR（1 起算的序号），写 `stss` 用。
    sync: Vec<u32>,
    /// 每一帧显示多久（90 kHz 刻度）。编码跟不上时这一格会变大，时长照样对。
    times: Vec<u32>,
    bytes: u64,
}

impl Mp4 {
    fn new(file: File, path: PathBuf, w: u32, h: u32, fps: u32) -> Result<Mp4> {
        let mut mp4 = Mp4 {
            out: BufWriter::with_capacity(1 << 20, file),
            path,
            width: w,
            height: h,
            fps: fps.max(1),
            mdat_at: 0,
            data_at: 0,
            pos: 0,
            sizes: Vec::new(),
            sync: Vec::new(),
            times: Vec::new(),
            bytes: 0,
        };
        // `ftyp` 先说自己是 isom，兼容 iso2 / avc1 / mp41：手机相册、VLC、
        // ffmpeg 都认。
        let mut brand = Vec::new();
        brand.extend_from_slice(b"isom");
        brand.extend_from_slice(&0x0200u32.to_be_bytes());
        brand.extend_from_slice(b"isomiso2avc1mp41");
        mp4.put(&boxed(b"ftyp", brand))?;
        mp4.mdat_at = mp4.pos;
        // 长度先写 0，收尾补成真的；补的时候只改这 4 个字节，sample 不挪窝。
        mp4.put(&0u32.to_be_bytes())?;
        mp4.put(b"mdat")?;
        mp4.data_at = mp4.pos;
        Ok(mp4)
    }

    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    fn bytes(&self) -> u64 {
        self.bytes
    }

    fn frames(&self) -> u64 {
        self.sizes.len() as u64
    }

    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        self.out.write_all(bytes).context("write the MP4")?;
        self.pos += bytes.len() as u64;
        Ok(())
    }

    /// 收一帧：AVCC 字节流原样落进 `mdat`，大小记进 sample 表。MP4 里一帧
    /// 就是一个 sample，所以暂停、跳格这些都不需要填充 —— 写了几帧就是几帧。
    fn sample(&mut self, bytes: &[u8], key: bool, delta: u32) -> Result<()> {
        if key {
            self.sync.push(self.sizes.len() as u32 + 1);
        }
        self.put(bytes)?;
        self.sizes.push(bytes.len() as u32);
        self.times.push(delta.max(1));
        self.bytes += bytes.len() as u64;
        Ok(())
    }

    /// 这段录像有多长（毫秒），按每一帧的间隔累加。
    fn millis(&self) -> u64 {
        let ticks: u64 = self.times.iter().map(|&d| d as u64).sum();
        ticks * 1000 / 90_000
    }

    /// 补 `mdat` 长度、写 `moov`，交出文件有多少字节。
    fn finish(&mut self, sps: &[u8], pps: &[u8]) -> Result<u64> {
        let end = self.pos;
        self.out.flush().context("flush the MP4")?;
        self.patch32(self.mdat_at, (end - self.mdat_at) as u32)?;
        let moov = self.moov(sps, pps);
        self.out.write_all(&moov).context("write the MP4's index")?;
        self.pos += moov.len() as u64;
        self.out.flush().context("flush the MP4")?;
        Ok(self.pos)
    }

    /// `moov`：一条视频轨。`stsd` 里是 `avc1` 加 `avcC`（SPS/PPS 与 4 字节
    /// 长度前缀），`stss` 记着哪些帧是 IDR。
    ///
    /// 时间轴用 90 kHz：90 000 能被 60 / 30 / 24 / 15 整除，每一格的时长都是
    /// 整数。sample 表一秒一个 chunk，`stco` 只记每个 chunk 的第一个字节，
    /// 播放器拖动时不必从头加一遍。
    fn moov(&self, sps: &[u8], pps: &[u8]) -> Vec<u8> {
        let media: u64 = self.times.iter().map(|&d| d as u64).sum();
        let millis = media * 1000 / 90_000;
        let mut matrix = Vec::new();
        for v in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
            matrix.extend_from_slice(&v.to_be_bytes());
        }

        // mvhd：整段多长、时间刻度 1 ms、下一条轨的编号。
        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&0u32.to_be_bytes()); // creation_time
        b.extend_from_slice(&0u32.to_be_bytes()); // modification_time
        b.extend_from_slice(&1000u32.to_be_bytes());
        b.extend_from_slice(&(millis.min(u32::MAX as u64) as u32).to_be_bytes());
        b.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate 1.0
        b.extend_from_slice(&0x0100u16.to_be_bytes()); // volume 1.0
        b.extend_from_slice(&0u16.to_be_bytes());
        b.extend_from_slice(&[0u8; 8]);
        b.extend_from_slice(&matrix);
        b.extend_from_slice(&[0u8; 24]);
        b.extend_from_slice(&2u32.to_be_bytes());
        let mvhd = boxed(b"mvhd", b);

        // tkhd：轨号 1，时长按影片刻度，宽高是 16.16 定点。
        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 7]); // enabled | in movie | in preview
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&(millis.min(u32::MAX as u64) as u32).to_be_bytes());
        b.extend_from_slice(&[0u8; 8]);
        b.extend_from_slice(&0i16.to_be_bytes()); // layer
        b.extend_from_slice(&0i16.to_be_bytes()); // alternate_group
        b.extend_from_slice(&0u16.to_be_bytes()); // volume
        b.extend_from_slice(&0u16.to_be_bytes());
        b.extend_from_slice(&matrix);
        b.extend_from_slice(&(self.width << 16).to_be_bytes());
        b.extend_from_slice(&(self.height << 16).to_be_bytes());
        let tkhd = boxed(b"tkhd", b);

        // mdhd：媒体自己的刻度是 90 kHz。
        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&90_000u32.to_be_bytes());
        b.extend_from_slice(&(media.min(u32::MAX as u64) as u32).to_be_bytes());
        b.extend_from_slice(&0x55C4u16.to_be_bytes()); // 'und'
        b.extend_from_slice(&0u16.to_be_bytes());
        let mdhd = boxed(b"mdhd", b);

        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(b"vide");
        b.extend_from_slice(&[0u8; 12]);
        b.extend_from_slice(b"VideoHandler\0");
        let hdlr = boxed(b"hdlr", b);

        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 1]); // flags: no compression
        b.extend_from_slice(&0u16.to_be_bytes()); // graphicsmode
        b.extend_from_slice(&[0u8; 6]); // opcolor
        let vmhd = boxed(b"vmhd", b);

        // dinf：数据就在这个文件里。
        let mut url = Vec::new();
        url.extend_from_slice(&[0, 0, 0, 1]); // self-contained
        let mut dref = Vec::new();
        dref.extend_from_slice(&[0, 0, 0, 0]);
        dref.extend_from_slice(&1u32.to_be_bytes());
        dref.extend_from_slice(&boxed(b"url ", url));
        let dinf = boxed(b"dinf", boxed(b"dref", dref));

        // avcC：profile / level 从前三个字节抄，长度前缀 4 字节，一组 SPS/PPS。
        let mut c = Vec::new();
        c.push(1); // configurationVersion
        c.push(sps.get(1).copied().unwrap_or(0x42)); // AVCProfileIndication
        c.push(sps.get(2).copied().unwrap_or(0x00)); // profile_compatibility
        c.push(sps.get(3).copied().unwrap_or(0x1F)); // AVCLevelIndication
        c.push(0xFF); // 6 位保留 + lengthSizeMinusOne = 3
        c.push(0xE1); // 3 位保留 + numOfSequenceParameterSets = 1
        c.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        c.extend_from_slice(sps);
        c.push(1); // numOfPictureParameterSets
        c.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        c.extend_from_slice(pps);
        let avcc = boxed(b"avcC", c);

        // avc1 的视觉 sample entry：宽高、72 dpi、一帧一块、24 位深。
        let mut e = Vec::new();
        e.extend_from_slice(&[0u8; 6]); // reserved
        e.extend_from_slice(&1u16.to_be_bytes()); // data_reference_index
        e.extend_from_slice(&0u16.to_be_bytes()); // pre_defined
        e.extend_from_slice(&0u16.to_be_bytes()); // reserved
        e.extend_from_slice(&[0u8; 12]); // pre_defined
        e.extend_from_slice(&(self.width as u16).to_be_bytes());
        e.extend_from_slice(&(self.height as u16).to_be_bytes());
        e.extend_from_slice(&0x0048_0000u32.to_be_bytes()); // horizresolution
        e.extend_from_slice(&0x0048_0000u32.to_be_bytes()); // vertresolution
        e.extend_from_slice(&0u32.to_be_bytes()); // reserved
        e.extend_from_slice(&1u16.to_be_bytes()); // frame_count
        e.extend_from_slice(&[0u8; 32]); // compressorname
        e.extend_from_slice(&0x0018u16.to_be_bytes()); // depth
        e.extend_from_slice(&0xFFFFu16.to_be_bytes()); // pre_defined
        e.extend_from_slice(&avcc);
        let entry = boxed(b"avc1", e);

        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&1u32.to_be_bytes());
        b.extend_from_slice(&entry);
        let stsd = boxed(b"stsd", b);

        // stts：连续相同的间隔并成一条，采集抖动不会把这张表撑爆。
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for &d in &self.times {
            match runs.last_mut() {
                Some((count, seen)) if *seen == d => *count += 1,
                _ => runs.push((1, d)),
            }
        }
        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&(runs.len() as u32).to_be_bytes());
        for (count, delta) in &runs {
            b.extend_from_slice(&count.to_be_bytes());
            b.extend_from_slice(&delta.to_be_bytes());
        }
        let stts = boxed(b"stts", b);

        // stss：哪几帧是 IDR。全篇都是 P 帧的时候没有 IDR 就不写这一盒，
        // ISO 里缺省就是「全部同步」。
        let mut stss = Vec::new();
        if !self.sync.is_empty() && self.sync.len() < self.sizes.len() {
            let mut b = Vec::new();
            b.extend_from_slice(&[0, 0, 0, 0]);
            b.extend_from_slice(&(self.sync.len() as u32).to_be_bytes());
            for n in &self.sync {
                b.extend_from_slice(&n.to_be_bytes());
            }
            stss = boxed(b"stss", b);
        }

        // 一秒一个 chunk：chunk 的偏移由每帧大小累加出来，不另存。
        let per = self.fps as usize;
        let n = self.sizes.len();
        let mut chunks: Vec<u32> = Vec::new();
        let mut at = self.data_at;
        let mut i = 0;
        while i < n {
            chunks.push(at as u32);
            let take = per.min(n - i);
            for k in 0..take {
                at += self.sizes[i + k] as u64;
            }
            i += take;
        }
        let mut runs: Vec<(u32, u32)> = Vec::new();
        if n > 0 {
            let last = n - per * (chunks.len() - 1);
            if chunks.len() == 1 {
                runs.push((1, last as u32));
            } else {
                runs.push((1, per as u32));
                if last != per {
                    runs.push((chunks.len() as u32, last as u32));
                }
            }
        }
        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&(runs.len() as u32).to_be_bytes());
        for (first, count) in &runs {
            b.extend_from_slice(&first.to_be_bytes());
            b.extend_from_slice(&count.to_be_bytes());
            b.extend_from_slice(&1u32.to_be_bytes());
        }
        let stsc = boxed(b"stsc", b);

        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&0u32.to_be_bytes()); // sample_size 0：逐个给
        b.extend_from_slice(&(n as u32).to_be_bytes());
        for size in &self.sizes {
            b.extend_from_slice(&size.to_be_bytes());
        }
        let stsz = boxed(b"stsz", b);

        let mut b = Vec::new();
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&(chunks.len() as u32).to_be_bytes());
        for at in &chunks {
            b.extend_from_slice(&at.to_be_bytes());
        }
        let stco = boxed(b"stco", b);

        let mut stbl = Vec::new();
        for part in [stsd, stts, stss, stsc, stsz, stco] {
            stbl.extend_from_slice(&part);
        }
        let stbl = boxed(b"stbl", stbl);
        let mut minf = Vec::new();
        minf.extend_from_slice(&vmhd);
        minf.extend_from_slice(&dinf);
        minf.extend_from_slice(&stbl);
        let minf = boxed(b"minf", minf);
        let mut mdia = Vec::new();
        mdia.extend_from_slice(&mdhd);
        mdia.extend_from_slice(&hdlr);
        mdia.extend_from_slice(&minf);
        let mdia = boxed(b"mdia", mdia);
        let mut trak = Vec::new();
        trak.extend_from_slice(&tkhd);
        trak.extend_from_slice(&mdia);
        let trak = boxed(b"trak", trak);
        let mut moov = Vec::new();
        moov.extend_from_slice(&mvhd);
        moov.extend_from_slice(&trak);
        boxed(b"moov", moov)
    }

    fn patch32(&mut self, at: u64, value: u32) -> Result<()> {
        self.out.seek(SeekFrom::Start(at)).context("seek in the MP4")?;
        self.out.write_all(&value.to_be_bytes()).context("patch the MP4")?;
        self.out.seek(SeekFrom::Start(self.pos)).context("seek back in the MP4")?;
        Ok(())
    }
}

/// 一个盒子：4 字节长度 + 4 字节名字 + 身子。MP4 的字段全是大端。
fn boxed(name: &[u8; 4], body: Vec<u8>) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + body.len());
    v.extend_from_slice(&((8 + body.len()) as u32).to_be_bytes());
    v.extend_from_slice(name);
    v.extend_from_slice(&body);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一段 MP4 里 `mdat` 的长度、`moov` 与 sample 表都按写进去的来。
    #[test]
    fn a_finished_mp4_names_its_frames_and_its_size() {
        let path = PathBuf::from("/tmp/skyward-record-test.mp4");
        let mut mp4 = Mp4::new(File::create(&path).unwrap(), path.clone(), 1080, 2400, 30).unwrap();
        // 两帧 AVCC：一个 IDR 加一个 P。
        mp4.sample(&[0, 0, 0, 2, 0x65, 0x88], true, 3000).unwrap();
        mp4.sample(&[0, 0, 0, 2, 0x41, 0x9a], false, 3000).unwrap();
        let bytes = mp4.finish(&[0x67, 0x42, 0xC0, 0x1F], &[0x68, 0xCE]).unwrap();
        let text = std::fs::read(&path).unwrap();
        assert_eq!(bytes, text.len() as u64);
        assert_eq!(&text[4..8], b"ftyp");
        assert!(text.windows(4).any(|w| w == b"mdat"));
        assert!(text.windows(4).any(|w| w == b"moov"));
        assert!(text.windows(4).any(|w| w == b"avc1"));
        assert!(text.windows(4).any(|w| w == b"avcC"));
        assert!(text.windows(4).any(|w| w == b"stss"));
        // mdat 的长度是 8 字节头 + 两帧。
        let at = text.windows(4).position(|w| w == b"mdat").unwrap() - 4;
        let len = u32::from_be_bytes([text[at], text[at + 1], text[at + 2], text[at + 3]]);
        assert_eq!(len as usize, 8 + 6 + 6);
    }

    /// 一帧都没有也把 `moov` 写出来，文件至少是完整的空片。
    #[test]
    fn an_empty_take_still_gets_an_index() {
        let path = PathBuf::from("/tmp/skyward-record-empty.mp4");
        let mut mp4 = Mp4::new(File::create(&path).unwrap(), path.clone(), 600, 1304, 30).unwrap();
        let bytes = mp4.finish(&[0x67, 0x42, 0xC0, 0x1F], &[0x68, 0xCE]).unwrap();
        let text = std::fs::read(&path).unwrap();
        assert_eq!(bytes, text.len() as u64);
        assert!(text.windows(4).any(|w| w == b"moov"));
        // 没有 IDR 就不写 stss（缺省全是同步）。
        assert!(!text.windows(4).any(|w| w == b"stss"));
    }

    /// 交出去的 `Summary` 只认自己那个文件。
    #[test]
    fn a_summary_names_its_own_file() {
        let s = Summary {
            path: PathBuf::from("/sdcard/DCIM/Screenrecords/Screenrecord_2026-11-21-17-59-10.mp4"),
            frames: 371,
            bytes: 26_940_729,
            millis: 12_366,
            dropped: 3,
        };
        assert_eq!(s.name(), "Screenrecord_2026-11-21-17-59-10.mp4");
        assert_eq!(s.frames(), 371);
        assert_eq!(s.bytes(), 26_940_729);
    }

    /// 容器只有 MP4，后缀、短名、规格行三处都得对上。
    #[test]
    fn the_only_container_is_mp4() {
        let c = Container::ONLY;
        assert_eq!(c.extension(), "mp4");
        assert_eq!(c.short(), "MP4");
        assert_eq!(c.label(), "MP4 (H.264)");
        assert_eq!(c.spec(), "MP4 (H.264)");
    }

    /// 一半的面板缩成 600×1304，一格对一格，颜色取平均。
    #[test]
    fn shrink_halves_exactly() {
        let src: Vec<u8> = (0..(4 * 2 * 4))
            .map(|i| match i % 4 {
                0 => 10,
                1 => 20,
                2 => 30,
                _ => 255,
            })
            .collect();
        let xs = spans(2, 1);
        let ys = spans(2, 1);
        let mut out = Vec::new();
        shrink(&src, 2, 2, &xs, &ys, &mut out);
        assert_eq!(out, vec![10, 20, 30]);
    }

    /// 原画那一档只去掉 alpha，一个像素都不动。
    #[test]
    fn shrink_at_full_size_only_drops_alpha() {
        let src: Vec<u8> = (0..(3 * 4)).map(|i| (i * 7) as u8).collect();
        let xs = spans(3, 3);
        let ys = spans(1, 1);
        let mut out = Vec::new();
        shrink(&src, 3, 1, &xs, &ys, &mut out);
        assert_eq!(out.len(), 9);
        assert_eq!(out[0], src[0]);
        assert_eq!(out[4], src[5]);
        assert_eq!(out[8], src[10]);
    }

    /// 灰一点的颜色应该落在 YUV 的中段，纯红在 V 上，纯蓝在 U 上。
    #[test]
    fn rgb_turns_into_yuv420() {
        let (w, h) = (2usize, 2usize);
        let grey = vec![128u8; w * h * 3];
        let mut yuv = Yuv420::new(w as u32, h as u32);
        to_i420(&grey, w, h, &mut yuv);
        assert!((yuv.y[0] as i32 - 128).abs() <= 1, "grey Y {}", yuv.y[0]);
        assert!((yuv.u[0] as i32 - 128).abs() <= 1, "grey U {}", yuv.u[0]);
        assert!((yuv.v[0] as i32 - 128).abs() <= 1, "grey V {}", yuv.v[0]);

        let mut red = vec![0u8; w * h * 3];
        for px in red.chunks_exact_mut(3) {
            px[0] = 255;
        }
        to_i420(&red, w, h, &mut yuv);
        assert!(yuv.v[0] > 200, "red V {}", yuv.v[0]);
        assert!(yuv.u[0] < 110, "red U {}", yuv.u[0]);
    }

    /// 码率按像素与帧率推，画质档只是每像素多少比特。
    #[test]
    fn the_bitrate_follows_the_size_and_the_rate() {
        let small = Settings::of((1200, 2608), 2, 2, 1);
        let big = Settings::of((1200, 2608), 0, 1, 0);
        assert_eq!(small.width, 600);
        assert_eq!(small.height, 1304);
        assert_eq!(small.fps, 30);
        assert_eq!(big.width, 1200);
        assert_eq!(big.height, 2608);
        assert!(big.bitrate() > small.bitrate() * 4, "{} vs {}", big.bitrate(), small.bitrate());
        assert!(small.bitrate() >= 600_000);
    }
}
