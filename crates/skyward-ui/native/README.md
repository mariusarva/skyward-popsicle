# native/

录屏的 H.264 编码器。`crates/skyward-ui/build.rs` 会把 `h264_shim.cpp` 编出来，
跟 `aarch64-linux-android/libopenh264.a` 合成一个静态库链进 recovery。

## 为什么在这儿

recovery 里没有 `MediaCodec`（`mediaserver` 根本不启动），也没有 ffmpeg，
录屏只能靠 CPU 编。MJPEG 那条路 2026-11-21 走到头了：真机 v106 录出来的
`mp4v` + `esds` 对象类型 0x6C，ffmpeg 和 VLC 放得开，**相册一打开就说文件损坏**。
原因是 Android 的 `MPEG4Extractor` 不看 `esds` 里的对象类型，`mp4v` 这个 4CC 被
`FourCC2MIME` 直接映射成 `video/mpeg4`，系统于是拿 `c2.android.mpeg4.decoder`
去解 JPEG，第一帧就 `ERROR_MALFORMED`。相册没有 MJPEG 解码器，换容器写法也没用。

换成 `avc1` + H.264 Baseline（CAVLC）就落在 `c2.android.avc.decoder` 上，
Android 4.1 起每台机器都有，相册、缩略图、拖动全都正常。

## 里面是什么

| 文件 | 说明 |
| --- | --- |
| `h264_shim.cpp` | openh264 的 C 包装：建编码器、喂 I420、收 AVCC。顺手补了 `operator new/delete`、`__cxa_guard_*`、`__cxa_pure_virtual`，所以整包不依赖 libc++ |
| `include/` | openh264 2.4.1 的四个公开头（`codec_api.h`、`codec_app_def.h`、`codec_def.h`、`codec_ver.h`） |
| `aarch64-linux-android/libopenh264.a` | 交叉编好的静态库，1.7 MB，87 个对象 |

## 库是怎么来的

`libopenh264.a` 是提交进仓库的二进制，构建时不现编（要 NDK 的 clang、八十多个
源文件、两分钟）。重编用 `scripts/build_openh264.py`：

```bash
# 源：https://codeload.github.com/cisco/openh264/tar.gz/refs/tags/v2.4.1
# 解到 D:/WorkBuddyAI/downloads/openh264/openh264-2.4.1
python scripts/build_openh264.py          # → D:/WorkBuddyAI/tmp/oh264/libopenh264.a
```

脚本解析 `codec/*/targets.mk` 拿源文件列表（openh264 自己的 Makefile 在 Windows
中文路径下会让 `mingw32-make` 崩），用 NDK r29 的 `clang++ --target=aarch64-linux-android30`
逐个编，`llvm-ar` 打包。几条硬性要求：

* **不带 `-DHAVE_NEON_AARCH64`**。openh264 2.4.1 的 arm64 汇编不完整：
  `get_intra_predictor.cpp` 在 `HAVE_NEON_AARCH64` 下引用
  `WelsI4x4LumaPredDDLTop_AArch64_neon` 这类函数，`codec/common/arm64/*.S` 里
  一个都没实现，链不上。纯 C++ 在 600×1304 上够 30 帧/秒。
* **每个文件都带那六个 `-Dandroid_getCpu*=wels_*`**。`cpu-features.c` 从 NDK 的
  `sources/android/cpufeatures` 拉进来编，函数被改名；`cpu.cpp` 那头不跟着改名
  就找不到 `android_getCpuCount`。
* `-fno-rtti -fno-exceptions`：一是省体积，二是避开 Windows 上 host 侧验证时的
  `__gxx_personality_seh0`。
* `-ffunction-sections -fdata-sections`：链接时靠 `--gc-sections` 丢掉没用到的
  decoder / processing。

## 编码参数

`sky_h264_new(width, height, fps, bitrate)` 里定的：

| 项 | 值 | 为什么 |
| --- | --- | --- |
| `iUsageType` | `CAMERA_VIDEO_REAL_TIME` | 实时档，不做两遍编码 |
| `iComplexityMode` | `LOW_COMPLEXITY` | 少跑几遍模式选择 |
| `uiProfileIdc` | `PRO_BASELINE` (66) | 相册、硬解、软解都认 |
| `iEntropyCodingModeFlag` | 0 | baseline 必须 CAVLC |
| `uiIntraPeriod` | `fps` | 一秒一个 IDR，缩略图与拖动都靠它 |
| `iRCMode` | `RC_BITRATE_MODE` | 码率由 `Settings::bitrate()` 按像素与帧率推 |
| `iMultipleThreadIdc` | 1 | 只能有一个实例（见下） |

**只能有一个编码器实例。** H.264 的 P 帧引的是同一个实例的上一帧；两条线程各编
各的、写线程再按到达顺序混着写，解码器看到的就是两条互不相干的参考链，ffmpeg 报
`reference picture missing during reorder`，相册黑屏。MJPEG 一帧一张图才可以拆开。

## 输出

每个 sample 是 AVCC：`[4 字节大端长度][NAL]…`，SPS(7)/PPS(8) 不写进 sample，
单独放进 `moov` 的 `avcC`。写线程不做补帧 —— 丢一帧或重发一帧都会断参考链，
编码跟不上就让 `stts` 的间隔变大，时长照样对。
