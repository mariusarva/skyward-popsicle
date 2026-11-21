# archive · 留档

仓库里唯一不参与构建的目录。放的是「镜像做出来时的那份元数据」，不是源码，
也不是可刷的镜像本体——镜像在 Releases 与 `output/`（后者不进仓库）。

```
archive/
├── v40/
│   ├── device.toml     从 v40 镜像的 ramdisk 里取出的 /skyward/device.toml
│   └── SHA256SUMS      v40 镜像的 sha256
├── v96/
│   └── SHA256SUMS      v96（品牌）与 v97（官方）两版镜像的 sha256
├── v98/
│   └── SHA256SUMS      v98（品牌）与 v99（官方）两版镜像的 sha256
├── v100/
│   └── SHA256SUMS      v100（品牌）与 v101（官方）两版镜像的 sha256
├── v102/
│   └── SHA256SUMS      v102（品牌）与 v103（官方）两版镜像的 sha256
├── v104/
│   └── SHA256SUMS      v104（品牌）与 v105（官方）两版镜像的 sha256
├── v106/
│   └── SHA256SUMS      v106（品牌）与 v107（官方）两版镜像的 sha256
├── v108/
│   └── SHA256SUMS      v108（品牌）与 v109（官方）两版镜像的 sha256
├── v110/
│   └── SHA256SUMS      v110（品牌）与 v111（官方）两版镜像的 sha256
├── v112/
│   └── SHA256SUMS      v112（品牌）与 v113（官方）两版镜像的 sha256
└── v114/
    └── SHA256SUMS      v114（品牌）与 v115（官方）两版镜像的 sha256
```

## v40

第一个留档版本，在 `OS4.0.6.0.XPBCNXM` 上由外部 fork 构建，weaver 优先
（NXP weaver + `se_omapi` / eSE1），gatekeeper / secdis 兜底，触感已经改成
Cirrus CS40L26 + `FF_SINE`。这份 `device.toml` 只作对照——`scripts/local_build.py`
读的是 `devices/popsicle.toml`（`DEVICE = "popsicle"`），**根目录的配置不参与构建**。

对应的解密方式写在 [`docs/解密方式.md`](../docs/解密方式.md)。镜像本体不在仓库里：
GitHub 上那个「任务备份 2026-11-14（镜像）」Release（tag `backup-2026-11-14`，
15 个资产 / 1.3 GB）已于 2026-11-21 删除，旧镜像只留在出包那台机器上，
`v40/SHA256SUMS` 是核对身份的凭据。

## v98

v98 与 v99 一同挂在 Release [v98](https://github.com/Ff428/skyward-popsicle/releases/tag/v98)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v98 | 卑鄙坑Baby | `v98-skyward-popsicle-OS4.0.6.0.XPBCNXM-5be6a16.img` | `c4c403f73238a85618d6b1e4e55b634738e9aef98bf776142342afd3ead48c7d` |
| v99 | Skyward / © 2026 Kirk Lin | `v99-skyward-popsicle-OS4.0.6.0.XPBCNXM-5be6a16.img` | `e02f8852fd02224e6310856dca0cf2b2081c6ddfe17655636753bd61ad4d84a4` |

两版都从 `5be6a16` 这棵树出，二进制除品牌串外一致。

## v100

v100 与 v101 挂在 Release [v100](https://github.com/Ff428/skyward-popsicle/releases/tag/v100)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v100 | 卑鄙坑Baby | `v100-skyward-popsicle-OS4.0.6.0.XPBCNXM-b51ba9d.img` | `151e8341ecaa75165a0e9d6c87d34e47a3dcea4d1455c21e9a1e065ed3f91614` |
| v101 | Skyward / © 2026 Kirk Lin | `v101-skyward-popsicle-OS4.0.6.0.XPBCNXM-b51ba9d.img` | `c9050a68571ba6dd18320c4b4d3d6c0251a12c82e5f4f39af83c7e9488ac504b` |

两版都从 `b51ba9d` 这棵树出（当时录屏是 AVI / MP4 / 两个都写三档；v104 起回到两档）。

## v102

v102 与 v103 挂在 Release [v102](https://github.com/Ff428/skyward-popsicle/releases/tag/v102)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v102 | 卑鄙坑Baby | `v102-skyward-popsicle-OS4.0.6.0.XPBCNXM-2541087.img` | `3df79984efb394806bd4a53df67e29babbaeac42f351cb4c39a79b7003b8ab08` |
| v103 | Skyward / © 2026 Kirk Lin | `v103-skyward-popsicle-OS4.0.6.0.XPBCNXM-2541087.img` | `f1aaa3242295ccb5c3f4cc15fc0eabce832a43b6f9fb13bca81aa2ab6362636f` |

两版都从 `2541087` 这棵树出（版本号报到上游 `0.3.23`，录屏页写清 MP4 的相册兼容性与「索引在末尾」）。

## v104

v104 与 v105 挂在 Release [v104](https://github.com/Ff428/skyward-popsicle/releases/tag/v104)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v104 | 卑鄙坑Baby | `v104-skyward-popsicle-OS4.0.6.0.XPBCNXM-b14b7d1.img` | `a3e9bb8a0fb12a3b6c4cd8e4e757d3be80246f34a79127f36026b7ec8ce5ac68` |
| v105 | Skyward / © 2026 Kirk Lin | `v105-skyward-popsicle-OS4.0.6.0.XPBCNXM-b14b7d1.img` | `9a4108f8b2b8aa076501b0af38a1ef6d9c53e1662ba1fa7523c3b4c172fec81a` |

两版都从 `b14b7d1` 这棵树出（录屏容器两档：AVI 或 MP4，一次录屏一个文件，都是 MJPEG）。

## v106

v106 与 v107 挂在 Release [v106](https://github.com/Ff428/skyward-popsicle/releases/tag/v106)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v106 | 卑鄙坑Baby | `v106-skyward-popsicle-OS4.0.6.0.XPBCNXM-2ba53fb.img` | `0d47f9244fa5361f2ce9e512f6b2058294766a63d4df1f42ae3e5c8daeb61a01` |
| v107 | Skyward / © 2026 Kirk Lin | `v107-skyward-popsicle-OS4.0.6.0.XPBCNXM-2ba53fb.img` | `8dd5f94ef4f16a75280af9db201594d228e872cba779620525a904e0a5dd5742` |

两版都从 `2ba53fb` 这棵树出：录屏只剩 MP4（MJPEG），`Avi` 那套 `RIFF` / `hdrl` / `movi` / `idx1`
代码整段删掉，`recovery` 因此从 11,262,432 B 缩到 11,253,024 B。校验命令：

```powershell
Get-FileHash .\v106-skyward-popsicle-OS4.0.6.0.XPBCNXM-2ba53fb.img -Algorithm SHA256
Get-FileHash .\v107-skyward-popsicle-OS4.0.6.0.XPBCNXM-2ba53fb.img -Algorithm SHA256
```

## v108

两版一同挂在 Release [v108](https://github.com/Ff428/skyward-popsicle/releases/tag/v108)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v108 | 卑鄙坑Baby | `v108-skyward-popsicle-OS4.0.6.0.XPBCNXM-c2a3a05.img` | `4cf8ca5f3c3492972e405915c15ac25a62a2ebfad3c0a11c8a8a567abaef7a24` |
| v109 | Skyward / © 2026 Kirk Lin | `v109-skyward-popsicle-OS4.0.6.0.XPBCNXM-c2a3a05.img` | `8eca5293829d25e8101acbbb6ec73f7751baf728d003e5e572da46219545ba01` |

两版都从 `c2a3a05` 这棵树出：录屏的码流从 MJPEG 换成 **H.264 Baseline**（静态
openh264 2.4.1，`avc1` + `avcC`），容器仍是 MP4 一个。`recovery` 从 11,253,024 B
涨到 11,572,784 B（+319,760 B 是编码器），压缩后 ramdisk 67,085,065 → 67,301,444 B。
校验命令：

```powershell
Get-FileHash .\v108-skyward-popsicle-OS4.0.6.0.XPBCNXM-c2a3a05.img -Algorithm SHA256
Get-FileHash .\v109-skyward-popsicle-OS4.0.6.0.XPBCNXM-c2a3a05.img -Algorithm SHA256
```

## v110

上一版，两版挂在 Release [v110](https://github.com/Ff428/skyward-popsicle/releases/tag/v110)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v110 | 卑鄙坑Baby | `v110-skyward-popsicle-OS4.0.6.0.XPBCNXM-f8bad13.img` | `2910615b069d77457cdff75ead6742133d44b22ea9b95669222381639d7fda15` |
| v111 | Skyward / © 2026 Kirk Lin | `v111-skyward-popsicle-OS4.0.6.0.XPBCNXM-f8bad13.img` | `e6f55fccb0e2f635b7efb2e1e58beb211fec61ca7fcbbd7588c09503aa023504` |

两版都从 `f8bad13` 这棵树出，跟 v108 只差录屏的帧率处理：`push` 按 `1/fps` 放行、
编码线程先编一帧预热、第一帧真实画面强制 IDR。`recovery` 11,572,784 → 11,574,736 B，
压缩后 ramdisk 67,302,095 / 67,302,067 B。

v108 那份真机录的 22 秒片子量出来是 67.7 fps / 6.6 Mbps / IDR 每 0.25 秒一个，
开头还有 638 ms + 474 ms 两个空洞 —— 帧率设定根本没生效。详见
[docs/CHANGELOG.md](../docs/CHANGELOG.md) 的「v110：帧率设的 30，录出来 67.7」。

## v112

当前出货的两版，一同挂在 Release [v112](https://github.com/Ff428/skyward-popsicle/releases/tag/v112)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v112 | 卑鄙坑Baby | `v112-skyward-popsicle-OS4.0.6.0.XPBCNXM-4dc5055.img` | `0dfd4aabc31d1601a86a2be6250bebf90d57d07f51d99f28b90fb24beda29696` |
| v113 | Skyward / © 2026 Kirk Lin | `v113-skyward-popsicle-OS4.0.6.0.XPBCNXM-4dc5055.img` | `3ee27291d986facd7829d3ee6340b39a97692fc34ca853fa1032cae42381b555` |

两版都从 `4dc5055` 这棵树出，跟 v110 只差录屏的出帧方式：`push` 只把画布放进
「最新一帧」那一格，编码线程自己按 `1/fps` 起表，这一格没有新画面就重编上一帧
（P 帧整帧 skip）。文件因此是恒定帧率。`recovery` 11,574,736 → 11,557,328 B，
压缩后 ramdisk 67,296,547 / 67,296,574 B。校验命令：

```powershell
Get-FileHash .\v112-skyward-popsicle-OS4.0.6.0.XPBCNXM-4dc5055.img -Algorithm SHA256
Get-FileHash .\v113-skyward-popsicle-OS4.0.6.0.XPBCNXM-4dc5055.img -Algorithm SHA256
```

v110 那份真机录的 44 秒片子量出来是 762 帧 / 17.28 fps，`stts` 里 ≥ 66 ms 的 89 格
占掉 21.5 秒 —— 页面不重绘就没有帧。详见 [docs/CHANGELOG.md](../docs/CHANGELOG.md)
的「v112：设的 30 帧，文件里只有 17.28」。

## v114

当前出货的两版，一同挂在 Release [v114](https://github.com/Ff428/skyward-popsicle/releases/tag/v114)：

| 版本 | 品牌 | 文件 | sha256 |
|---|---|---|---|
| v114 | 卑鄙坑Baby | `v114-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img` | `a0f3c1f0a04a7c874ab04de93f4365af430d9e02ff928af3ccf867e3860924e5` |
| v115 | Skyward / © 2026 Kirk Lin | `v115-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img` | `68c8300c001f12bb5503afda169373cfd52974eebc4f8f8f380206a344a52b33` |

两版都从 `23f9600` 这棵树出。v112 的启动日志（2026-11-21 20:27，19:44 那次开机
到 67 s）里 `the motor came up` 落在 40.87 s —— 正是第一次点屏幕的时刻，说明马达
只在点击时才试一次。v114 让页面每秒试一次，马达一到位就装上；`push` 也只在
「最新一帧」那格空着时才抄画布（屏 120 Hz、编码 30，四分之三的 12.5 MB 抄写本来
就被顶掉）；没有 ready 条件的 program 改记 `started`，不再读成「0 秒就绪」。
`recovery` 11,557,328 → 11,562,544 B。校验命令：

```powershell
Get-FileHash .\v114-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img -Algorithm SHA256
Get-FileHash .\v115-skyward-popsicle-OS4.0.6.0.XPBCNXM-23f9600.img -Algorithm SHA256
```

v112 的录屏至今没有真机样本：20:27 那份日志只到启动与联网，没有 `record:` 行。
帧率仍按 v112 的设计走（编码线程按 `1/fps` 自驱出帧），真机量法写在
[docs/录屏.md](../docs/录屏.md) 的「怎么验」。

## 出新版时怎么加

镜像本体不进仓库（每个 100 MB，`out/` 与 `output/` 已在 `.gitignore` 里）。
新版只加一个目录、一份 `SHA256SUMS`：

```sh
mkdir -p archive/v108
sha256sum out/v108-*.img > archive/v108/SHA256SUMS
```

上一版的目录可以留着——它只有几百字节，比在 CHANGELOG 里翻哈希方便。
