# Skyward Popsicle

**小米 17 Pro Max**（代号 `popsicle`，系统 OS4.0.6.0.XPBCNXM）的定制 recovery。

基线 [kirklin/skyward](https://github.com/kirklin/skyward) **v0.3.0**，合并 [mariusarva/skyward-pudding](https://github.com/mariusarva/skyward-pudding) 的功能补丁到 **v0.3.23**（`aa4b52a`），再加本机的触感 / 触控修复、录屏、自写安装器与品牌替换。当前版本 **v114 / v115**。

![license](https://img.shields.io/badge/license-GPL--3.0-blue)
![release](https://img.shields.io/badge/release-v114%20%2F%20v115-success)
![upstream](https://img.shields.io/badge/upstream-v0.3.23-informational)
![device](https://img.shields.io/badge/device-popsicle%20%C2%B7%2017%20Pro%20Max-orange)

| 我要… | 看这里 |
|---|---|
| 刷进手机 | [快速开始](#快速开始) |
| 知道这版改了什么 | [功能](#功能) · [本机修复](#本机修复) |
| 核对下载到的文件 | [镜像](#镜像) |
| 自己出一版 | [构建](#构建) |
| 读懂某个子系统 | [文档](#文档) |

```
上游主仓库   kirklin/skyward             →  recovery 本体
功能补丁库   mariusarva/skyward-pudding  →  控制中心 / WiFi 记忆 / 中文输入
镜像留档     mariusarva/skyward-popsicle →  v40 的 device.toml 与解密说明
本仓库       Ff428/skyward-popsicle      →  源码 + 镜像留档（上面三者 + 本机改动）
```

## 快速开始

```sh
fastboot flash recovery_b v114-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img
```

- 只写 `recovery` 分区。刷之前先备份原厂 recovery。
- **不要用 recovery 里的「检查更新」**：它会把 Skyward 官方镜像写回分区，本分支的定制会被覆盖。
- 文件名里的短号是出包所用的提交；本页两版都出自 `23f9600`，二进制只差品牌串。
- 两版一起挂在 Release [v114](https://github.com/Ff428/skyward-popsicle/releases/tag/v114)：`v114` 是品牌版、`v115` 是官方版。

## 功能

| 线 | 本分支做的 | 细节 |
|---|---|---|
| 录屏 | 分辨率 4 档 / 画质 3 档 / 帧率 4 档；码流 **H.264 Baseline**（静态 openh264），容器只有 **MP4**；相册、ffmpeg、VLC 都能直接放；可拖到任意位置的计时胶囊，点红点暂停继续、点白方块结束 | [docs/录屏.md](docs/录屏.md) |
| 安装系统 | 自写 payload applier：多线程解 xz / bz2 直写目标槽、不建 COW、并行读回校验、同会话补 root | [docs/安装.md](docs/安装.md) |
| 控制中心 | 下拉 SHADE，12 圆磁贴 + 设备卡 | — |
| 中文输入 | 拼音栏，候选与词频来自 Unihan ∩ GB2312 + jieba 词表 | — |
| 无线 | WiFi 持久化并自动连接、网络下载、无线传输 | — |
| 其它页 | 任务 / 分区管理 / 关机 | — |
| 逐版记录 | v40 → v115，每版的原因、日志原文与代码位置 | [CHANGES.md](CHANGES.md) · [docs/变更历史-v41-v97.md](docs/变更历史-v41-v97.md) |

录屏在 **v84 之前不存在**；安装从 **v76 起**不再把包交给 `update_engine`。两条线的 v40 → v97 对比表在各自文档的第一节。

## 本机修复

| 分类 | 内容 |
|---|---|
| 触感 | cs40l26 用 `FF_SINE (0x5a)` 的 BUZZGEN 发震动（`FF_CUSTOM` 在这台机器上几乎没反应），返回键与点击都有震动，默认强度 0.7 |
| 触控 | 双触控面板（Synaptics 主屏 1200×2608 + Goodix 副屏 976×596）按分辨率打分选屏；后注册的面板若打分更高会自动替换，热插拔也能纠正 |
| 稳定性 | 震动设备打开失败**不再致命**（懒重试），避免模块加载半途崩溃导致触控彻底失效 |
| 品牌 | 见下节 |
| 出包 | boot image v4 + LZ4 legacy ramdisk + AVB（vbmeta / footer 重算） |

### 两个品牌

同一份代码出两版，二进制除品牌串外一致：

| 版本 | 首页 / hero | 署名 |
|---|---|---|
| **v114**（偶数） | 卑鄙坑Baby ·「叼着奶瓶逛酒吧」 | © 2026 卑鄙坑Baby |
| **v115**（奇数） | Skyward | © 2026 Kirk Lin |

## 镜像

| 版本 | 文件 | 大小 | md5 |
|---|---|---|---|
| v114 | `v114-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img` | 104,857,600 B | `6cb3a9167b419d550906eaab097b5d52` |
| v115 | `v115-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img` | 104,857,600 B | `a7c8f84a1c0cf65ef9e2c024f86941de` |

<details>
<summary>sha256 与内部体积</summary>

| 项 | v114（品牌） | v115（官方品牌） |
|---|---|---|
| sha256 | `a0f3c1f0a04a7c874ab04de93f4365af430d9e02ff928af3ccf867e3860924e5` | `68c8300c001f12bb5503afda169373cfd52974eebc4f8f8f380206a344a52b33` |
| 压缩后 ramdisk | 67,295,840 B | 67,295,767 B |
| 主程序 | `/system/bin/recovery` 11,562,544 B | 同左 |
| 代码 | `23f9600` | 同左 |
| stock 基线 | OS4.0.6.0.XPBCNXM（recovery sha256 `cfd845ae…`） | 同左 |

校验和另见 [archive/v114/SHA256SUMS](archive/v114/SHA256SUMS)，两版文件在 Release [v114](https://github.com/Ff428/skyward-popsicle/releases/tag/v114)。

</details>

<details>
<summary>镜像内部结构</summary>

4 KiB 头（`ANDROID!`）+ `0x1000` 起的 LZ4 legacy 流（15 块）→ 124,568,396 B 的 newc cpio（1,279 条目）。

| 路径 | 内容 |
|---|---|
| `/system/bin/recovery` | 主程序 |
| `/skyward/system-init` | 早期 init |
| `/skyward/device.toml` | 配置 |

</details>

## 解密

这一版显式配置 **weaver 优先**，NXP 链路（17 Ultra 是 Goodix，不适用）：

```toml
[data]
weaver_first = true
```

- **首选**：`<id>.weaver` 记录用户在 eSE 里的槽号 → `android.hardware.weaver-service.nxp-qti` 经 OMAPI 打开 eSE1 的 applet 取槽 → keymint 解 `<id>.spblob`
- **兜底**：Android 为同一用户另留的一份 blob 绑定在 `<id>.secdis` 字节上，由 keymint + gatekeeper 用同一口令打开，结果等价

<details>
<summary>这条链必须凑齐的服务与两个已知坑</summary>

| 服务 | 注册名 |
|---|---|
| `qseecomd` | — |
| `hlosminkdaemon` | — |
| `android.hardware.security.onekeymint-service-qti` | keymint / RKP / secureclock / sharedsecret |
| `android.hardware.gatekeeper-rust-service-qti` | gatekeeper |
| `android.hardware.weaver-service.nxp-qti` | `IWeaver/default` |
| `android.hardware.secure_element-service.qti` | `ISecureElement/SIM1`、`/SIM2`、**`/eSE1`** |
| `android.hardware.boot-service.qti` | `IBootControl/default` |
| `se_omapi` | `android.se.omapi.ISecureElementService/default` |

外加两处：`modules.load` 要含 `nxp-nci`；`/system/etc/vintf/manifest/se_omapi.xml` 必须存在 —— vendor 自带的 `secure_element-service.xml` 把 name 与 instance 分写在两个标签，`secure::describes()` 匹配不到，weaver 会报 `Failed to start omapiSeService null`，`/data` 锁死。

完整分析见 [docs/解密方式.md](docs/解密方式.md)。

</details>

## 源码

本仓库带着 v114 的完整源码树（`23f9600`），镜像就是从这棵树里出的。

| 目录 | 内容 |
|---|---|
| `crates/` | 23 个 workspace 成员：`skyward-ui` 界面、`skyward-install` 自写 applier、`skyward-net` 更新检查、`skyward-crypto` / `skyward-omapi` / `skyward-lp` / `skyward-block` 等 |
| `scripts/` | 出包链：`local_build.py`、`unpack_image.py`、`repack_image.py`、`patch_avb.py`、`genpinyin.py`、`ccheck.py`、`check_record.py`、`build_openh264.py` |
| `devices/` | 21 台机型的配置；构建读 `devices/popsicle.toml`（`local_build.py` 里 `DEVICE = "popsicle"`） |
| `resources/` | `fonts/MiSansVF.ttf`（19.2 MB）、`popsicle/recovery-hal.zip`（13.9 MB） |
| `docs/` | 构建、打包、同步上游、解密、录屏、安装、逐版记录 |
| `patches/` | 布丁补丁副本，升级时复用 |
| `archive/` | 留档：`v40/`（`device.toml` + 校验和）、`v98/`、`v100/`、`v102/`、`v104/`、`v106/`、`v108/`、`v110/`、`v112/`、`v114/`（两版镜像的 sha256），不参与构建 |

改动集中在 `crates/skyward/src/haptics.rs`（cs40l26 震动）、`crates/skyward/src/input.rs`（触控面板打分）、`crates/skyward-ui/src/app/`（录屏、控制中心、各页）、`crates/skyward-ui/native/`（openh264 静态库 + C 包装）、`crates/skyward-install/`（自写 applier）。

> `archive/v40/device.toml` 是从 v40 镜像里取出来的那一份，只作留档，构建不读它。

## 构建

```sh
CARGO_TARGET_DIR=<目标目录> python scripts/local_build.py \
  --name v114 --deliver --deliver-tag v114 --keep-old
```

<details>
<summary>出官方品牌（v115）</summary>

先 `touch crates/skyward-ui/src/pages.rs` —— 不 touch 不会重编那一页：

```sh
touch crates/skyward-ui/src/pages.rs

SKYWARD_BRAND_TITLE=Skyward SKYWARD_BRAND_WORDMARK=Skyward \
SKYWARD_BRAND_AUTHOR="© 2026 Kirk Lin" \
CARGO_TARGET_DIR=<目标目录> python scripts/local_build.py \
  --name v115 --deliver --deliver-tag v115 --keep-old
```

不带这三个变量就是 `卑鄙坑Baby` 品牌。交叉编译环境见 [docs/BUILD.md](docs/BUILD.md)。

</details>

## 文档

| 文档 | 内容 |
|---|---|
| [RESTORE.md](RESTORE.md) | 换机 / 重装 / 任务丢失后怎么恢复环境与出包链路 |
| [docs/BUILD.md](docs/BUILD.md) | 工具链、交叉编译、常见报错 |
| [docs/PACK-IMAGE.md](docs/PACK-IMAGE.md) | 从 boot 镜像出包的完整流程与校验点 |
| [docs/SYNC-UPSTREAM.md](docs/SYNC-UPSTREAM.md) | 怎么同步上游主仓库与布丁仓库 |
| [docs/录屏.md](docs/录屏.md) | 录屏：三档设置、计时胶囊、H.264 + MP4 逐字段、线程模型、为什么不是 MJPEG |
| [docs/安装.md](docs/安装.md) | 安装：自写 applier、门槛、进度、并行校验、回写 root / vbmeta / 合并 |
| [docs/解密方式.md](docs/解密方式.md) | weaver / OMAPI 链路与 eSE 槽位 |
| [docs/变更历史-v41-v97.md](docs/变更历史-v41-v97.md) | 逐版原因、日志原文与代码位置 |
| [archive/README.md](archive/README.md) | 留档文件索引：哪份是什么、对应哪次出包 |

## 出处

- 本体：[kirklin/skyward](https://github.com/kirklin/skyward)（GPL-3.0）
- 功能补丁：[mariusarva/skyward-pudding](https://github.com/mariusarva/skyward-pudding)
- 早期镜像与解密记录：[mariusarva/skyward-popsicle](https://github.com/mariusarva/skyward-popsicle)

许可证：[GPL-3.0](LICENSE)。
