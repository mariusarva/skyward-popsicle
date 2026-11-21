# 换机 / 重装 / 任务丢了怎么恢复

**这份文件是整个项目的入口。** 换了电脑、重装了系统、或者这个 WorkBuddy 任务被删掉了，
只要还能拿到这个仓库，让 AI 先读这一份就能接着干活，不用重新摸索。

---

## 0. 三句话说明这是什么

1. **做的是什么**：小米 17 Pro Max（代号 `popsicle`）的第三方 recovery（卡刷包），Rust 写的，
   上游是 [kirklin/skyward](https://github.com/kirklin/skyward)，功能补丁来自
   [mariusarva/skyward-pudding](https://github.com/mariusarva/skyward-pudding)。
2. **本仓库是什么**：`Ff428/skyward-popsicle`（私有）。**不是 fork**，所以 GitHub 网页上
   没有 "Sync fork" 按钮，同步上游只能在本机用 git 做（已封装成一条命令，见第 5 节）。
3. **当前跟到哪**：上游 `v0.3.1` 的 4 个修复 + 布丁 `v0.2.52` + 品牌定制，
   基线官方系统 `OS4.0.6.0.XPBCNXM`。具体版本谱系见
   [docs/CHANGELOG.md](docs/CHANGELOG.md)，跟上游的方法见
   [docs/SYNC-UPSTREAM.md](docs/SYNC-UPSTREAM.md)。

> 给 AI 的一句话：**这是重建过的 git 历史，`v0.3.x` tag 不是 HEAD 的祖先**，
> `git cherry-pick <上游提交>` 可以用，`git merge upstream/vX` 会报 unrelated histories。
> 别试图 merge，走 cherry-pick。

---

## 0.5 四种情况，这份备份够不够

结论：**都够，区别只在「还要补什么」。**

| 情况 | 能不能接着干 | 要补的东西 |
|---|---|---|
| 电脑重装 / 换一台电脑 | ✅ 能 | 装 WorkBuddy + Python + git；令牌重新授权（2 分钟）。走 CI 出包不用装 Rust/NDK；要本机出包就按 `docs/LOCAL-BUILD.md` 把 Rust 1.100.0、NDK r29、`lz4.exe` 补上 |
| 这个 WorkBuddy 任务被删了 | ✅ 能 | 什么都补不了——把仓库或那个 zip 给 AI，让它先读本文件 |
| 手机系统升级（如 `OS4.0.7.0`） | ✅ 能，但要真机验证 | 新版本全量包地址 + 一次出包确认原厂 ramdisk 结构没变（见下） |
| 上游发新版（如 `0.3.2`） | ✅ 能 | 一条 `git fetch upstream --tags`，然后跑侦察脚本；按规则决定 cherry-pick 还是三方合并 |

**唯一补不了的是令牌**（不能进版本库）。其余都在仓库或 zip 里。

两条容易忽略的前提：

- **上游 tag 不在 zip 的 bundle 里**。历史是重建的，tag 指向被截断的真实上游提交，
  所以必须 `git fetch upstream --tags --depth=200` 才有。第 2 节第 3 步就是干这个。
- **手机系统升级不是改个版本号就完事**。`bump_stock.py` 会改基线、跑 CI、回填摘要，
  但新原厂 recovery 的 ramdisk 结构可能变了（多了/少了 HAL、库版本不同）。
  出包后必须确认 `check_build.py` 通过（ramdisk ≥1200 条、15 个关键文件齐全），
  再上真机验证解密和震动——这两项最依赖 HAL 层。

---

## 1. 环境清单

编译本身跑在 GitHub Actions（Ubuntu）上，所以**新电脑只要能 clone、能推、能跑 python 就能出包**。
下面这张表只有「想在本机做编译验证」时才需要配齐。

| 组件 | 版本 | 本机（Windows）实测路径 | 说明 |
|---|---|---|---|
| Python | 3.13+ | `C:\Users\F\.workbuddy\binaries\python\versions\3.13.12\python.exe` | 跑 `scripts/*.py` |
| git | 2.55+ | `D:\软件\应用\PortableGit\mingw64\bin\git.exe` | 需要 `merge-tree --write-tree`（2.38+） |
| Rust | **1.100.0** | `D:\d\WorkBuddyAI\rustup\toolchains\1.100.0-x86_64-pc-windows-gnu\bin` | 版本由 `rust-toolchain.toml` 钉死，别用 stable |
| Rust target | `aarch64-linux-android` | 同上 `lib/rustlib/` | 只有这个 toolchain 带 |
| mingw64 | 任意 | `D:\软件\应用\mingw64\mingw64\bin` | 只为 `dlltool.exe`，过程宏编译必需 |
| Android NDK | r29 | `D:\WorkBuddyAI\downloads\ndk_r29\android-ndk-r29\toolchains\llvm\prebuilt\windows-x86_64\bin` | 编译 sqlite3 等 C 代码 |
| lz4（Python 包） | 任意 | `pip install --target D:\WorkBuddyAI\cache\pylibs lz4` | `check_build.py` / `augment_stock.py` 要用 |

**Linux / macOS** 更简单：`rustup target add aarch64-linux-android`，装个 NDK，
然后直接 `scripts/build-android.sh`（自动挑 host 目录）。

---

## 2. 恢复步骤

```bash
# 1) 拿代码
git clone https://github.com/Ff428/skyward-popsicle.git
cd skyward-popsicle

# 2) 配好三个 remote（clone 只会带 origin）
git remote add upstream https://github.com/kirklin/skyward.git
git remote add pudding  https://github.com/mariusarva/skyward-pudding.git

# 3) 拿上游 tag（历史是重建的，必须显式 fetch 才有）
git fetch upstream --tags --depth=200

# 4) 拿推送凭据（见第 3 节），然后验证能推
python scripts/push.py --dry-run
```

Windows 上想在本机编译验证，先 source 一份环境（会自动找 toolchain / NDK / mingw）：

```bash
source scripts/dev-env.sh
cargo check --workspace --target aarch64-linux-android
```

**注意**：`cargo test` 在 Windows 上跑不了（`skyward-bootimg`、`skyward-preview` 用了
`std::os::unix`），单元测试只在 CI 上跑。`skyward-preview` 同理，只能 Linux。

---

## 3. 凭据：唯一不能备份的东西

令牌**故意没进仓库**（进了仓库就等于泄露）。换机器后重新走一次设备码授权即可，约 2 分钟：

```bash
python scripts/gh_device_flow.py          # 生成八位码，浏览器打开 github.com/login/device 输入
```

- 授权成功后令牌写入：老机器 `D:\WorkBuddyAI\github\.token`；那个目录不存在时
  自动落到 `~/.config/skyward/token`（Linux/macOS 上也一样）
- 需要的 scope：**`repo` + `workflow`**（没有 `workflow` 就推不了 `.github/workflows/`）
- `scripts/push.py` 不用把令牌写进任何命令，它按这个顺序找：
  1. `--token-file`
  2. 环境变量 `GITHUB_TOKEN` / `GH_TOKEN`（CI、临时机器最省事）
  3. `D:\WorkBuddyAI\github\.token`
  4. `~/.config/skyward/token`
- 代理也是自动判断的：本机 `127.0.0.1:7890` 通就走代理，不通就直连，
  换网络不用改脚本。要强制直连加 `--no-proxy`（两个脚本都支持）

---

## 4. 出包

两条路，跑的是同一串命令。

**一、推 CI**（发布用，产物可复现）：

```bash
python scripts/push.py -m "改了什么"      # 推 main → 自动触发 Build popsicle
```

产物在 Actions 页面 → 对应 run → Artifacts → 下载 zip → 解开是 100 MiB 的 `.img`。

**二、本机出包**（断网、额度用完、只想快点看成品）：

```bash
python scripts/local_build.py            # → out/skyward-*.zip + SHA256SUMS
```

八步全在公司里跑完：交叉编译两个 ARM 二进制、编译打包器、取原厂 recovery 分区
（有缓存不重下）、补 HAL 层、打包重算 AVB、自检、压缩。需要 Rust 1.100.0、NDK r29、
`lz4.exe`、带 `lz4` 模块的 Python —— 详见 [docs/LOCAL-BUILD.md](docs/LOCAL-BUILD.md)，
那里也写了为什么 Windows 编得过（5 处 `#[cfg(unix)]`，手机端二进制不变）。

打包链路（`.github/workflows/build.yml` 与 `local_build.py` 一一对应）：

```
build tools → 缓存/还原原厂 recovery.img → 校验 sha256
            → augment_stock.py 补 HAL 层 → skyward-pack 打包
            → check_build.py 自检 → zip
```

**卡关标准**（`scripts/check_build.py` 会强制）：ramdisk ≥ 1200 条目、15 个关键文件齐全、
AVB 自洽。任何一项不过就红，不允许交出「能开机但解不开 /data」的镜像。

两条路的 ramdisk 都是 **1279 条**；压缩后体积略有差别（本机 lz4 v1.10 比 CI 的 v1.9.4
压得松一点），是分区留白，不影响刷机。

打 `v*` 标签会额外发 Release。

---

## 5. 跟上游更新

```bash
python scripts/sync_upstream.py --fetch    # 看报告：规模、已应用、分类、冲突预演、建议
python scripts/sync_upstream.py --apply    # 预演干净才动手，有冲突会拒绝
python scripts/push.py -m "v15: ..."
```

另外，仓库里有个每天跑的 `watch-upstream.yml`：上游发新版就在 **Issues 里开一条**，
附完整提交分类和下一步命令，同一个 tag 不重复开。所以**平时不用主动去查**。

> 升级完记得手工改 `.github/upstream-baseline.json` 里的 tag。不改的话，监听每天
> 都会把已经跟过的版本再报一次。

判断原则（血泪教训，详见 `docs/SYNC-UPSTREAM.md` 第六节）：

- 纯重构、又正好压在 `app.rs` / `ui.rs` 上 → **只 cherry-pick fix，别整体合并**
- 布丁也发了适配新上游的补丁 → 走三方合并
- 布丁没跟上 → 只合上游，再按 `docs/CUSTOMIZATIONS.md` 把四件套补回来
- 改完必须 `cargo check`，不能只看 diff 有没有冲突标记

---

## 6. 这份备份里有什么、没有什么

**在仓库里**（`git clone` 就有）：

| 位置 | 内容 |
|---|---|
| `crates/` `devices/` `resources/` | 源码、设备配置、HAL 补丁层（`resources/popsicle/recovery-hal.zip`） |
| `docs/` | 升级、出包、定制、变更记录四份文档 |
| `backup/memory/` | 项目记忆（版本谱系、踩过的坑、判断规则）——**这是最值钱的部分** |
| `backup/scripts/` | 本机常用脚本快照（授权、CI 查询、AVB 校验、刷机） |
| `backup/MANIFEST.md` | 历史交付镜像的 sha256 清单 |
| `scripts/` | 出包、同步、推送、备份的工具 |

**不在仓库里**（要单独拿）：

| 东西 | 在哪 | 为什么 |
|---|---|---|
| 成品 `.img` | 本机 `D:\WorkBuddyAI\backup\recovery\`（v14–v17 的 zip）与原机器的 `output/`；GitHub 上那份 2026-11-21 已删 | 单个 100 MiB，git 里放不下 |
| 推送令牌 | 本机 `D:\WorkBuddyAI\github\.token` | 不能进版本库，换机重新授权 |
| 编译缓存 `target/`（1.9 GB） | 本机 | 全新编译 7 分钟，没必要备 |
| 原厂 recovery.img（1 GB） | CI 缓存 / 现下 | 已固化 sha256，随时可重新拉 |

**重建成本参考**：全新机器，装环境 20 分钟 + 首次 `cargo check` 8 分钟 + CI 全量编译 7 分钟。

**重新打包**（记忆 / 文档 / 脚本有更新后，重打一份带新历史的 zip）：

```bash
python scripts/make_backup_zip.py            # 版本号自动读 docs/CHANGELOG.md 的「当前版本」
python scripts/backup_task.py --push         # 顺带把备份目录和 MANIFEST 刷新并提交
```

`make_backup_zip.py` 会重打 bundle（只打分支，不打 tag——理由见第 7 节）、按固定清单
收文件、写同名 `.sha256`。以前这一步是手工敲的，现在一条命令，换机器也不会丢。

**换机器后路径会变，脚本按下面的顺序找，不用改代码**：

| 要找什么 | 环境变量 | 默认位置 |
|---|---|---|
| WorkBuddy 任务目录 | `SKYWARD_TASK_DIR` | `D:\WorkBuddyAI\2026-11-13-20-45-53` |
| 记忆目录 | `SKYWARD_MEMORY_DIR` | `<任务目录>\.workbuddy\memory` |
| 交付镜像目录 | `SKYWARD_OUTPUT_DIR` | `<任务目录>\output` |
| 本机脚本目录 | `SKYWARD_SCRIPT_DIR` | `D:\WorkBuddyAI\scripts` |
| 推送令牌 | `SKYWARD_TOKEN_FILE` 或 `GITHUB_TOKEN` | `~/.config/skyward/token` → 老路径 |
| 网络代理 | `--proxy` / `--no-proxy` | 探测 `127.0.0.1:7890`，通才走 |

**交付目录不存在时 `backup_task.py` 不会覆盖 `backup/MANIFEST.md`**。13 个镜像的
哈希是它们唯一的身份凭据（镜像本体已不在 Releases 上），被一行「未找到 .img」抹掉就
没法核对了。脚本只提示原因并跳过；要指向新的交付目录用 `--images-dir` 或设环境变量。

---

## 7. 已知的坑（都踩过，别再踩）

| 现象 | 原因 | 处理 |
|---|---|---|
| `error calling dlltool 'dlltool.exe'` | mingw 不在 PATH | `source scripts/dev-env.sh` 会自动找 |
| `os error 3 系统找不到指定的路径` | 给 cc 传了 `/d/...` 形式 | `CC_*` 必须写 **Windows 形式** `D:\\...`，脚本里已用 `cygpath -w` 转 |
| `cargo check` 报找不到 rustc | 走了 rustup 代理 | 直接用工具链里的绝对路径 `cargo.exe`，别用 `~/.cargo/bin/cargo` |
| 推到 GitHub 失败 / 超时 | 沙箱内连不上，或没设代理 | 用 `scripts/push.py`（自带代理与认证） |
| 推 `.github/workflows/` 被拒 | 令牌没有 `workflow` scope | 重新授权，scope 写 `repo,workflow` |
| 镜像 42 MB，怀疑是错的 | 那是 zip，解开才 100 MiB | 别按 zip 体积判断 |
| `git merge upstream/vX` 报 unrelated histories | 历史是重建的快照 | 用 cherry-pick，别 merge |
| 镜像能开机但解不开 /data | 原厂 ramdisk 缺 HAL 层 | 确认 `augment_stock.py` 跑过了，ramdisk 应 1279 条 |
| `git clone` bundle 报 `remote did not send all necessary objects` | 用 `bundle --all` 打了 tag，本仓库浅克隆、tag 指向被截断的上游提交 | 只打分支：`git bundle create x.bundle HEAD main master pudding up030`；或直接用 `make_backup_zip.py` |
| 克隆出来只有 1 个提交、停在 `master` | bundle 里没显式写 `HEAD` | 同上，`HEAD` 要放第一个 |
| `gh_device_flow.py` 找不到文件 | 它以前只在本机 `D:\WorkBuddyAI\scripts\` | 现在仓库 `scripts/` 里也有一份，路径就是 `scripts/gh_device_flow.py` |
| 脚本里 `GIT_DIR` 指向 `repo.git` | 那是早期的快照仓库，已废弃，HEAD 停在旧提交 | 已全部移除；git 操作一律在仓库工作区里直接跑 |
| 所有网络请求都超时 | 老脚本无条件 pop 掉代理变量再强制设 7890，没 Clash 的机器上必挂 | 已改成探测；`--no-proxy` 强制直连，`--proxy` 指定别的 |
