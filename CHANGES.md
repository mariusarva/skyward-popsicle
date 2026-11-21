# 17 Pro Max builds

同一份代码出两个品牌：偶数版首页与关于页写 **卑鄙坑Baby**，奇数版写
**Skyward** + 署名 **© 2026 Kirk Lin**。功能改动只在偶数版里做，奇数版是它的
换牌子集，二进制除品牌串外一致。

| version | what |
|---|---|
| v40 | 首个留档：外部 fork 在 OS4.0.6.0.XPBCNXM 上构建，weaver 优先（NXP weaver + se_omapi/eSE1），gatekeeper/secdis 兜底；触感改用 Cirrus CS40L26 + FF_SINE（2026-11-19） |
| v42 | 解密页的进度条按**真实打点**加回（千分位 `AtomicU16`：映射 80 / 探测 250 / 挂载 360 / 各用户键 470 / 凭据 690 / 第一段 700，第二段 700→900→990）；条只增不减，满了才切页 |
| v44 | 解密页三条硬边界：记住的密码那次尝试 15 s 不回来就让出页面；分区已开时最多 0.7 s 必定离开；说「正在打开」却没有任务超过 3 s 就重开或报错。另修 `remember::writable()` 与 `forget()` |
| v46 | 找回头两次整树同步丢掉的定制：`Haptics::open` 非致命、`scan()` 双面板打分、`FF_SINE=0x5a`、默认强度 0.7；`panic=unwind`，工作线程 panic 变成页面上的失败原因 |
| v48 | 条走完页面却走不掉（`draw.rs:707` panic）：决定离开时先留帧，离开统一交给 `tick`；每一下点按都回一次马达 |
| v50 | 解密页底下那条英文（`last-failure` 的 panic 原文弹成 toast）只进 kmsg；震动强度每跨一档震一次，跨度 0.15–1.0，时长随强度走 |
| v52 | 切 USB 端口把 `sys.usb.ffs.mtp.ready` 清掉 → 端口停在 `none`、连 adb 一起丢；规则删掉，失败退回 `adb`。下拉面板底角与卡片圆角对齐 |
| v54 | 解密条三段卡顿：改成一条按时间走的扫掠（上限 99.4%），真实进度只当下限，收尾 0.55 s 补满；系统应用行可点 |
| v56 | 刷不进官方完整包（`unmount /vendor: EBUSY`）：`pin` 改成扫**所有进程**的 maps 与 fd，`MNT_DETACH` 兜底 |
| v58 | 写回 `recovery_b` 全是 `EPERM`：写前 `BLKROGET`/`BLKROSET`，仍失败就按 `start×512` 从父盘偏移写，写前逐字节比对 |
| v60 | 进度条不动 / 刷得慢 / 没有 REC：FUSE 块 64 KiB→1 MiB，`misc` 里空的 `boot-recovery` 请求在页面起来时清掉 |
| v62 | 更新覆盖启动镜像后，用手机自己的 root 工具重打再写回：Magisk `boot_patch.sh`（`KEEPVERITY`）、KernelSU `ksud boot-patch`；`getprop` 写替身顶在 `PATH` 前 |
| v64 | `done` 取 `min(served, engine + 64 MiB)`，条不再瞬间满格；条下四行滚动行（`PartitionInfo` / `Completed N/M` / `Hashing` / `Step 2`） |
| v66 | 合并从来没真正开始：recovery 重启后一个 snapshot 都没映射 → 新增 `map_snapshots`，`merge::run` 在 `start_merge` 前调用；boot control 改成 `Option`，拿不到就写 misc offset 4060 |
| v68 | 主进度改用 `update_engine` 自己的 `overall progress N%`，按速率线性外推；本地包直读 `file://` 不过 FUSE |
| v70 | 写入提速四招：`--max_threads`（认才传）、`performance` governor、`read_ahead_kb=4096`、`setpriority(-10)`；安装期间帧间隔 100 ms |
| v72 | **root 一直失败的两个真因**：`ksud boot-patch -o` 要文件夹不要文件名；块设备 `st_size` 是 0，分区大小必须 `BLKGETSIZE64`。另：先打更新写的那张镜像、再搬运行槽 |
| v74 | 保 root 进同一条进度条（四段权重）；不再往 `/sdcard/Skyward/` 留 `init_boot` |
| v76 | **不再让 `update_engine` 写包**：`payload.rs` 读 `CrAU` 头 + protobuf manifest（58 分区 / 5820 op / 4096 块），`applier.rs` 按目标槽 liblp 铺开、多线程解 xz 直写，不建 COW |
| v78 | 补 `REPLACE_BZ`（`bzip2 0.6.1`）；门槛先扫全 manifest、汇总成一句再退出，不是碰到第一处就停 |
| v80 | 控制中心改 **12 个圆磁贴（3 行 4 列）+ 设备卡**；新增任务 / 分区管理 / 关机三页；逐文件核对布丁 v0.3.15（69 文件，39 个逐字节相同） |
| v82 | 同步源只跟布丁 `v0.3.17`（`pudding@c05106c`），主仓库标 retired；补 vbmeta 备份与写回（`avbctl::keep/put_back`） |
| v84 | 版本号报到 `0.3.17`；拼音条真正出现（收汉字的页面一直带着）；**录屏落地**（AVI / MJPEG）；安装：七个解压线程 + 读回计时 |
| v86 | 高级工具页 / 分区页 / 服务页 `fit` 到内容（原来多两屏黑页）；**录屏重写**（只留最新帧、落后跳格、缩放搬进编码线程）；录屏页三档设置 + 计时胶囊；校验改按字节 |
| v88 | 校验按核并行（每分区一条 SHA-256，最多 8 条）；WLAN 首次 `IFF_UP` 等 3 s（驱动 1.9 s 才收下） |
| v90 | 计时胶囊可拖动、点红点暂停/继续；录屏页底部不再被切；**搜索框能打字**（`Screen::Search` 不收汉字） |
| v92 | 胶囊右边加**结束**方块：暂停 / 继续 / 结束三件事都在胶囊上；录完清空 `record_pill` |
| v94 | 录屏容器 **AVI → MP4**：`ftyp` + `mdat`（一帧一个 JPEG sample）+ 收尾 `moov`，`stsd` 是 `mp4v` + `esds` **0x6C（MJPEG）**，90 kHz 时间刻度、一秒一个 chunk。**v96 已改回 AVI**：相册认不认 `mp4v` 的 MJPEG 在主机上验不了 |
| v95 | 与 v94 同一份代码，只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` |
| v96 | 录屏容器 **MP4 → AVI**（照用户要求）：回到 `RIFF` / `hdrl`(`avih`+`strh`+`strf MJPG`) / `movi`(一串 `00dc`) / `idx1`，文件名 `.avi`，规格行 `… · MJPEG`；三档设置、胶囊的拖动/暂停/结束、暂停不写帧都不动，帧数按 `index.len()` 写 |
| v97 | 与 v96 同一份代码，只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` |

- v41 / v43 / v45 / … / v93 这些奇数版都是「上一版只换品牌」，不另列。
- 每一版的详细原因、日志原文与代码位置见 [docs/变更历史-v41-v97.md](docs/变更历史-v41-v97.md)；
  录屏与安装两条线的**当前完整形态**见 [docs/录屏.md](docs/录屏.md) 与
  [docs/安装.md](docs/安装.md)。
