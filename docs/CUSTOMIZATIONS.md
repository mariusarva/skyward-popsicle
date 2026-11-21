# 本分支改了什么（相对「上游 v0.3.0 + 布丁 v0.2.49」）

共五处：三处是解决这台机器「震动不响 / 触控失灵」的硬问题，两处是品牌。跨版本升级时，**这五处需要逐条确认还在**。

> ⚠️ 2026-11-19 的教训：v38 / v40 两次「把整棵树换成本仓库」之后，第 1、2、3 条加上第 4 条的默认强度**全部丢了**，
> 真机表现是「一直自动解密 → 黑屏 → 自动解密 → 黑屏，进不去首页」+「震动没了」+ 主屏点不动（v46 才找回来）。
> **整树同步之后不要只看 `cargo check` 通过，要对着这份清单逐条 `git diff 8abc6b2^ -- <文件>` 核一遍。**

> 另有两条不是「同步丢的」，但同样不能被上游覆盖（v48/v49 补的）：
>
> - **解密页离开前不许把 `self.frame` 扔掉**。`App::tick` 为了让转圈每帧重画，每帧 `self.frame = None`；
>   而 `leave()` 要用这个帧做滑出快照，判完 `data_finish` 就 `leave_decrypt()` 会 panic 在
>   `draw.rs` 的 `a page is drawn from its frame`——`catch_unwind` 接住后页面永远停在解密页
>   （条是满的，下面还挂着「已记住密码」）。所以：先定去留，要走就留着帧、等它随绘制回来再走。
> - **每一下点按都要回一次马达**（`Touch::Up` 命中控件后 `Effect::Haptic`，控件自己震过的不重复）。
>   上游只有滑动确认 / 解锁 / 边缘返回手势会震，点按钮完全没反馈。

---

## 1. 震动：cs40l26 用 `FF_SINE`，不是 `FF_CUSTOM`

文件：`crates/skyward/src/haptics.rs`

```rust
/// FF_SINE is 0x5a here, not the usual 0x5b (from the popsicle
/// cs40l26-core.ko's capability table: FF_PERIODIC 0x51, FF_SINE 0x5a,
/// FF_CUSTOM 0x5d, FF_GAIN 0x60).
const FF_SINE: u16 = 0x5a;
```

- 上游（以及大多数 Android 内核）拿 `FF_CUSTOM` 发自定义波形，Xiaomi 这颗 cs40l26 的 FF 表里 `FF_CUSTOM = 0x5d`、`FF_SINE = 0x5a`（标准值是 `0x5b`）。用 `FF_CUSTOM` 时驱动不翻译成 BUZZGEN，效果就是「毫无震动」或极弱。
- 改法：把自定义采样按**周期 6 ms、幅度 = 强度 × 255** 的周期波交给 `FF_SINE`，`replay.length` 取采样总时长；驱动会按周期切成 BUZZGEN 音调播放。
- 症状对照：只改了 `FF_CUSTOM` 的值、没改成周期波时，返回键会「很轻微地抖一下」。

## 2. 触控：双面板按分辨率打分选屏

文件：`crates/skyward/src/input.rs`，`fn scan()`

```rust
// The panel the display's size fits exactly (the axis tops are the
// resolution less one) beats any other; the rest are compared by
// area, as a coarse fallback.
let score = |p: &Panel| -> i64 {
    let exact = ((p.x_range.1 as i64 + 1 == self.w as i64)
              && (p.y_range.1 as i64 + 1 == self.h as i64)) as i64;
    (exact << 60) + (p.x_range.1 - p.x_range.0) as i64 * (p.y_range.1 - p.y_range.0) as i64
};
```

- 这台机器有**两个**触控面板：Synaptics 主屏 `1200×2608`，Goodix 副屏 `976×596`。两个都注册成 `Xiaomi_Touch_Input_N`，副屏的 input 节点**先**创建（约 4.336 s vs 4.395 s）。
- 上游「取第一个 DIRECT + 多点的设备」会抓到副屏，于是主屏点不动。
- 改法：把所有候选收集起来打分（分辨率完全匹配给 `1<<60` 的权重，否则按面积），并且**已经在用某个面板时**，若出现更高分的候选就替换（`chosen.replace(...)`），这样热插拔 / 迟到注册也能纠正。

## 3. 稳定性：震动设备打开失败不致命 + 懒重试

文件：`crates/skyward/src/ui.rs`

```rust
// The motor is not fatal: the modules may still be loading, and a
// motor that never comes is nothing for the pages to die over.
let mut haptics = match cfg.haptics.as_ref().map(Haptics::open) {
    Some(Ok(h)) => Some(h),
    Some(Err(e)) => {
        let _ = skyward_sys::kmsg(&format!("the motor is not up yet: {e:#}"));
        None
    }
    None => None,
};
```

外加 `fn gentle_click(haptics: &mut Option<Haptics>, cfg: &DeviceConfig, vibration: f32, ticks: bool)`：每次要响的时候如果句柄还是 `None`，就再尝试 `Haptics::open` 一次；打开失败把句柄丢掉，下一次再试。

- 为什么必须这样：布丁把「页面先画出来、模块后台加载」提到了启动早期。`Haptics::open` 在模块没就绪时会返回 `no input device takes force-feedback effects`。上游用 `?` 直接冒泡，于是整个 UI 线程在 `synctics_tcm2` 正在 insmod（约 800 ms）时崩掉 → 模块加载半途而废 → 模块被标成已加载（`EEXIST` 当成功）但没 probe → **主屏触控彻底消失**。
- 这处和 2 是两回事：3 修的是「启动时把触控模块搞死」，2 修的是「选错面板」。
- **v114 起不再只在点击时试**：`Pages::show` 每帧调一次 `take_up_motor()`，一秒最多试一次、两分钟为限（`MOTOR_ATTEMPTS = 120`），马达一到位就装上。v112 的启动日志里 `the motor came up` 落在 **40.87 s** —— 正是第一次点屏幕的时刻，说明 `gentle_click` 那条路只在有人点时走；而 cs40l26 的 FF 设备要等模块装完、`cs40l26.wmfw` 下载完才出现，开机后头几十秒的点击因此全没有震动。`gentle_click` 里的兜底留着（句柄被丢掉之后还能再拿回来）。

## 4. 交互：点击有震动 + 默认强度 0.7

文件：`crates/skyward-ui/src/app.rs`

```rust
// 触摸抬起且这一下真的触发了动作
Drag::Press { action: Some(a), .. } => {
    ...
    // A tap that does something confirms itself to the
    // finger, the way a hardware button would.
    if self.state.vibrate {
        fx.push(Effect::Haptic);
    }
    fx.extend(self.act(a, now));
}
```

```rust
vibration: 0.7,   // 原来是 SLIDER_START（0.1），滑块拉到最小才 0.45
```

`ui.rs` 侧对应地把 `Effect::Haptic` / `Effect::Ticks` 的处理改成 `if let Some(h) = &mut haptics { ... }`，并把 `?` 去掉，转交给 `gentle_click`。

### 4b. 滑块的每一档都按力度回震（v25）

文件：`crates/skyward-ui/src/app.rs`、`devices/popsicle.toml`

- 控制中心的振动滑块原来在 `shade_touch` 的 `Touch::Up` 里只做 `self.drag = Drag::Idle` 就返回，
  **拉完不震**，用户听到的全是别的按钮按默认 0.7 的固定震动，所以「拉到哪都一样」。
- 现在拖动每跨一档（`slider_step`，0..1 分 10 档）推一次 `Effect::Haptic`，强度取当前档；
  松手再震一次确认。设置页滑块共用 `set_level` 的返回值，行为一致。
- `strength` 从 `0.45-1.0` 改为 `0.30-1.0`：两端差 3.3 倍，0.7 处仍是 0.70 满强度，默认手感不变。
- 验强度别只看代码：真机上把滑块拖到最左、最右各松一次手，两下应当明显不同。

## 5. 品牌

文件：`crates/skyward-ui/src/pages.rs`

```rust
pub const HOME_TITLE: &str = brand(option_env!("SKYWARD_BRAND_TITLE"), "卑鄙坑Baby");
const WORDMARK: &str = brand(option_env!("SKYWARD_BRAND_WORDMARK"), "叼着奶瓶逛酒吧");
const AUTHOR: &str = brand(option_env!("SKYWARD_BRAND_AUTHOR"), "© 2026 卑鄙坑Baby");
```

- 三个字串都可以在**编译时**用环境变量顶掉，同一份源码出不同品牌的包（v32 起）：
  ```bash
  SKYWARD_BRAND_TITLE=Skyward SKYWARD_BRAND_WORDMARK=Skyward \
  SKYWARD_BRAND_AUTHOR="© 2026 Kirk Lin" \
  CARGO_TARGET_DIR=D:/WorkBuddyAI/tmp/target-v33 \
  python scripts/local_build.py --name v33
  ```
  `option_env!` 会被 cargo 记进指纹，改了变量就会重编 `skyward-ui`，不用 `touch`。
- 大字用 `widgets::wordmark()`（纯文字排，按屏宽自动缩字号；`wordmark_px` 在 37.8 dp 起步）。
- v10 曾做过「矢量奶瓶」版本：`widgets::wordmark_bottle()` + `bottle()` + `BOTTLE_ADVANCE`，因为 MiSansVF 的 cmap 最大只到 `U+2CE93`，**没有 emoji 字形**，直接写 🍼 只会画成豆腐块。那套代码保留着（标了 `#[allow(dead_code)]`），想切回图形版把 `pages.rs` 里的调用换成 `wordmark_bottle(...)` 即可。
- ⚠️ **不要改 `LICENSE_ORIGINAL`**：GPL 许可页里 `Skyward / Copyright (C) 2026 Kirk Lin` 是许可声明，按 GPL 必须保留原文。

---

## 6. 录屏：H.264 + 提交进仓库的 openh264 + 定速出帧（v108 / v112 / v114）

文件：`crates/skyward-ui/src/record.rs`、`crates/skyward-ui/build.rs`、
`crates/skyward-ui/native/`（`h264_shim.cpp`、`include/`、`aarch64-linux-android/libopenh264.a`、`README.md`）、
`scripts/build_openh264.py`

- v84–v107 的录屏装的是 MJPEG，容器一改成 MP4，相册就报「文件损坏」：Android 的
  `FourCC2MIME` 把 `mp4v` 硬映射成 MPEG-4，`esds` 里写 `0x6C` 也不看。只能换码流。
- v108 起录 H.264 Baseline：`avc1` + `avcC` 落在 `c2.android.avc.decoder` 上，
  Android 4.1 起每台设备都有。
- **`libopenh264.a` 是提交进仓库的二进制**（1,719,444 B，87 个对象，openh264 2.4.1，
  纯 C++、不带 arm64 汇编）。它没法在构建时现编：要 NDK 的 clang、八十多个源文件、
  几分钟。重编命令与参数表在 `crates/skyward-ui/native/README.md`，脚本是
  `scripts/build_openh264.py`。
- `build.rs` 只在 aarch64 target 下干活，且只在 shim 或那个 `.a` 的 mtime 变了才重跑；
  主机上（`cargo test`、`skyward-pack`）直接 return。
- ⚠️ **整树同步上游会把这些当成「多余文件」删掉或覆盖**（`build.rs`、`native/` 整目录、
  `record.rs` 里 `H264` / `to_i420` / 单实例那句注释）。同步完照这份清单核一遍，
  `git status` 里看到 `native/` 或 `build.rs` 不见了就是被吞了。
- 三条不能改的参数：`iMultipleThreadIdc = 1` 与**只有一个编码器实例**（P 帧引同一实例的
  上一帧，多实例混写会让 ffmpeg 报 `reference picture missing during reorder`）；
  `iEntropyCodingModeFlag = 0`（Baseline 只有 CAVLC）；`uiIntraPeriod = fps`（每秒一个 IDR）。
- **ready 门**：编码器建好、预热过之前 `push` 直接返回，时间轴不从建编码器那几百毫秒起算。
  `H264::force_idr`（shim 里的 `sky_h264_force_idr`）保证预热之后第一帧真实画面是 IDR，
  否则它是 P 帧、`avcC` 空、相册缩略图黑。
- **定速出帧（v112 起）**：`push` 只把画布放进「最新一帧」那一格（`Mutex<Option<Job>>` +
  `Condvar`），旧的一张还回池子；**编码线程自己按 `1/fps` 起表**，到点就编，这一格没有
  新画面就再编一遍手里的 `Yuv420`（P 帧整帧 skip）。文件因此是恒定帧率，`stts` 正常是
  一格 `90000/fps`；迟到的帧按实际发出时刻记 `at`，所以编码跟不上时是 2 格、3 格，时长
  仍然对。v110 那套 `due()` / `next` / 帧通道已经删掉 —— **加回来会让页面静止的一段
  掉到十几帧**（v110 实测 44 秒 762 帧 = 17.28 fps）。
- `Recorder::push` 里的顺序别换：`spare()` 先拿缓冲（那时不持锁），再锁 `mail`；编码线程
  是先取 `mail` 再锁 `pool`。反过来写就是死锁。
- **v114 起 `push` 只在那一格空着时才抄画布**：不空就直接丢这一张。屏 120 Hz、编码 30，
  四分之三的 `copy_from_slice`（默认 600×1304 时一帧 12.5 MB）本来就会被下一张顶掉，
  白抄三遍是每秒多占约一吉字节内存带宽，页面线程还得等它抄完。放进时撞上编码线程刚取走
  的那一下由 `replace` 兜住（旧的还回池子）。画面语义不变：每个编码格拿到的是「上一格
  取走之后的第一张」，正好是原来会被留在格子里的那张。
  ⚠️ 同步上游时这三处连同 `force_idr` 一起核，删掉任何一处都会回到「67.7 fps / 6.6 Mbps /
  开头 638 ms 空洞」。

---

## 另外两处非代码改动

- `device.toml`（打包进 ramdisk 的 `skyward/device.toml`）：

  ```toml
  [haptics]
  resonance = "/skyward/resonance"
  strength  = { lightest = 0.45, strongest = 1.0 }

  [data]
  weaver_first = true
  ```

- ramdisk 里追加 `lib/firmware/cs40l26*.bin|wmfw`（取自本机 `odm/firmware`）与 `skyward/resonance`（内容 `1724\n`，谐振频率）。没有这些，震动马达初始化会失败。
