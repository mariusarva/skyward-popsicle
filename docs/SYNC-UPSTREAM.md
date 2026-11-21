# 同步上游：只跟布丁仓库

**2026-11-21 起，本仓库只跟布丁。** 主仓库（`kirklin/skyward`）不再作为同步对象：
布丁的补丁就是以它的 `v0.3.1` 为底做的整树 `diff -ruN`，主仓库后面的改动布丁里都有；
两头一起跟只会让同一处改动以两种形态进来，合起来还要人工去重。

| 角色 | 仓库 | 现在的用法 |
| --- | --- | --- |
| 布丁仓库 | https://github.com/mariusarva/skyward-pudding | **唯一的同步来源**。不存源码树，只存 **累积式补丁** `patches/skyward-pudding-vX.patch`，每个补丁针对上游 `v0.3.1` 生成；`docs/CHANGES.md` 与 `docs/update.json` 说明每一版做了什么 |
| 主仓库 | https://github.com/kirklin/skyward | 不再跟。只当历史参考：布丁补丁的底座就是它的 `v0.3.1`（`c5e09f6`） |
| 本仓库 | `Ff428/skyward-popsicle` | 合成结果 + 本机修复 + 品牌 |

布丁补丁的用法是先 checkout 到它的底座 tag（`v0.3.1`），再 `git apply`。跨 tag 直接套会冲突——这正是升级的主要工作量。

### 补丁到哪一版为止

`patches/` 里的最后一个补丁是 **v0.3.15**。`v0.3.16` 之后的版本都只动
`docs/update.json` 与 `docs/CHANGES.md`，**没有源码补丁**：`v0.3.16/17` 说的圆磁贴、设备状态卡、
任务页、分区管理、镜像备份、关机页、vbmeta 备份与写回、拼音输入，要靠 `docs/CHANGES.md`
的说明自己实现（本仓库 v80 做了前一半，v82 补上 vbmeta 与拼音）。

**`v0.3.23`（2026-11-21）同样是零源码改动**：tag 打在 `c05106c`（= `v0.3.17` 那个提交）上，
分支头 `aa4b52a` 只多两个提交，`compare/c05106c...aa4b52a` 里全是
`docs/update.json`（tag 与资产名/大小/digest）与 `docs/CHANGES.md`（加一行）。
这一版的录屏本来就是从本仓库的 `Ff428/skyward-popsicle-1` 分支拿过去的（发布说明末尾有致谢），
所以「同步」= 版本号报到 `0.3.23` + 基线改到 `v0.3.23` / `aa4b52a`，没有代码要搬。

它自己的 tag 比提交慢一版：`v0.3.16` 打在「发布 v0.3.15」那个提交上，`v0.3.17` 打在
「发布 v0.3.16」上，`v0.3.23` 打在 `v0.3.17` 的提交上。所以 `scripts/sync_upstream.py`
默认跟 `pudding/main` 的分支头，基线钉提交（`commit`）而不是 tag。

## 零、日常怎么用这个仓库同步

### 网页上没有「Sync fork」按钮

本仓库是独立建的私有仓库，不是 GitHub 的 fork，所以那个按钮不存在。同步只能在开发机上做——但已经包成四条命令。

### 三个 remote

| remote | 指向 | 用途 |
| --- | --- | --- |
| `origin` | `Ff428/skyward-popsicle` | 自己的仓库，推这里触发 CI |
| `pudding` | `mariusarva/skyward-pudding` | **同步源**，取补丁与 `docs/CHANGES.md` |
| `upstream` | `kirklin/skyward` | 不再跟，只留作查历史（`v0.3.1` = `c5e09f6`） |

### 第一步：知道上游发没发新版

两种方式任选：

- **等仓库自己报**：`.github/workflows/watch-upstream.yml` 每天北京时间 10:00 查一次**布丁仓库**，只要比 `.github/upstream-baseline.json` 里的基线新，就在 Issues 里开一条，带完整提交分类和下一步命令。查过的不重复开。
- **自己主动查**：`python scripts/sync_upstream.py --fetch`

### 第二步：看清这次改了什么、能不能合

```bash
python scripts/sync_upstream.py --fetch
```

输出六段：

| 段 | 内容 |
| --- | --- |
| 规模 | 提交数、文件数、增删行数 |
| 已应用 | 哪些 fix 已经在 HEAD 里（patch-id + 提交标题双重判断，cherry-pick 会改 patch-id 所以不能只看一种） |
| 分类 | 按 commit 前缀分成修复 / 重构 / 构建 / 测试 / CI / 杂务 |
| 预演 | 对每个未应用的 fix 用 `git merge-tree --write-tree` 算一遍 cherry-pick 冲突，**纯计算，不碰工作区** |
| 建议 | 结合冲突情况和重构占比给出路线 |
| 布丁 | 另一个仓库跟到哪一版、比我们基线多了哪些提交（它面向 pudding 机型，只作参考） |

上游常常**先合提交、过几天才打 tag**，所以脚本跟的是 `upstream/main` 的分支头，而不是最后一个 tag；开头那行会写「领先 tag vX N 个提交，还没发版」。只盯 tag 会把「已经修好、还没发版」当成「没有更新」——0.3.1 之后那两个 fix 就是这么漏掉的。

脚本还会标出这次修复有没有碰到我们改过的文件——预演干净不代表语义正确，这种情况必须编译。

### 第三步：应用

预演全干净时：

```bash
python scripts/sync_upstream.py --fetch --apply
```

这一步才真正执行 `git cherry-pick`。有冲突时脚本会拒绝 `--apply`，转第二或第三节手工处理。

然后照[第四节](#四升级后怎么验收)验收：编译 → 出包 → 比对 ramdisk。

### 第四步：发布

```bash
python scripts/push.py -m "v15: ..."
```

脚本会先 add + commit，再用本地保存的令牌推 `origin main`，推完自动触发 CI 出包。

直接 `git push` 也能用，但 Windows 上要自己保证 PATH 里有 PortableGit 的 `mingw64/bin`（`git-remote-https` 在那）并设好代理，容易卡住，所以包了一层。

### 升级完必须改基线

把 `.github/upstream-baseline.json` 里的 `tag` 改成新版本，否则监听每天报同一个版本。

跟的是「tag 之后 main 上的提交」时，tag 改不动（还没发新版），这时加 `commit` 字段写上那个提交的短哈希，脚本会优先用它：

```json
"skyward": { "tag": "v0.3.1", "commit": "c5e09f6", "note": "0.3.1 后两个 fix 已手工移植" }
```

### 一个已知的历史问题

`v0.3.0`、`v0.3.1` 两个 tag 指向的是上游真实提交，但它们**不是**当前 `HEAD` 的祖先——本仓库的历史早期是从源码快照重建的（`e38180a base: upstream 0.2.0` 是孤立根提交）。后果：

- `git cherry-pick <上游提交>` 正常可用，按文件内容合并，不依赖祖先关系
- `git merge upstream/vX.Y.Z` **不行**，会报 unrelated histories

所以本仓库一直走「挑选 + 重放」而不是「merge」。要恢复完整血统，需要一次性把自己的提交 rebase 到真实上游历史上，属于结构性改造，动之前先备份。

## 一、先判断这次升级属于哪种情况

1. 上游发新 tag，布丁也发了对应新补丁 → 按下面「三方合并」走；
2. 上游发新 tag，布丁还是老补丁 → 先把老补丁 `git apply` 到新上游，冲突按「补丁冲突」处理；
3. 上游新 tag 已经原生包含布丁那批功能（例如本次 `v0.3.0` 一度被误判为已包含，实际**没有**——判断方法是搜 `app.rs`/`pages.rs` 里有没有 `Shade` 状态机与 `/metadata/skyward/credential`）→ 只合上游，然后按 [CUSTOMIZATIONS.md](CUSTOMIZATIONS.md) 重新加回四件套；
4. 上游新 tag 是一次**大重构**（删文件、拆模块、重排上万行），而真正有功能价值的只是其中几个 `fix` → **不要整体合并**，只 `cherry-pick` 那几个 fix。见下面[第六节](#六只挑-fix上游大重构时的做法)。`v0.3.0 → v0.3.1` 走的就是这条。

## 二、三方合并（我们做 v8 时用的流程）

准备：记下布丁补丁针对的上游 tag（`BASE_TAG`，补丁头部 `diff`/`index` 上下文能看出来，v0.2.49 针对 `v0.2.0`）。

```sh
mkdir merge && cd merge && git init
git config core.autocrlf false

# 1) base：布丁补丁所针对的那版上游
#    把 BASE_TAG 的源码解压进来
git add -A && git commit -m "base: upstream $BASE_TAG"

# 2) pudding 分支：base + 布丁补丁
git checkout -b pudding
git apply --whitespace=nowarn /path/to/patches/skyward-pudding-vX.patch
git add -A && git commit -m "pudding vX"
git checkout master

# 3) up 分支：新上游 + 本分支的定制（把本仓库最新那版整棵树覆盖进来）
git checkout -b up
#    用新上游 tag 的源码覆盖，然后按 CUSTOMIZATIONS.md 把四件套重新打上
git add -A && git commit -m "upstream $NEW_TAG + popsicle customizations"

# 4) 合并
git merge pudding
```

v0.2.0 → v0.3.0 这次合并**只有 9 处冲突**：

| 文件 | 处数 | 冲突点 | 取法 |
| --- | --- | --- | --- |
| `crates/skyward/src/main.rs` | 4 | 启动流程：布丁的「页面先出、模块后台加载」vs 上游的 `prepare/bring_up` 重构 | 保留上游新接口，补回布丁的进度阶段标记 `progress::reached(...)` |
| `crates/skyward/src/services.rs` | 2 | `Started` / `readied()` 与布丁的「单服务失败不拖垮整体」 | 以上游结构为准，失败只记日志 |
| `crates/skyward/src/ui.rs` | 3 | `ui::run` 的 `programs` 参数、`detach()`、`read_mounts()` | 保留布丁的 `EXTRA_PROGRAMS` 回填，签名按上游 |

`app.rs`、`pages.rs` 这两个最大的文件**没有冲突**，自动合并成功。

### 合并后必须手工检查的几处（跨版本容易漏）

新上游把一些老 API 换掉了，布丁的代码里仍用旧名：

| 旧写法 | 新写法 | 位置 |
| --- | --- | --- |
| `app.panel_lit()` | `!blanked`（`ui.rs` 事件循环里的熄屏变量） | `crates/skyward/src/ui.rs` |
| 没有 `Event::Wake` 分支 | 新增 `Incoming::Input(Event::Wake) => if !blanked { app.key(Key::Power, now); app.key_up(Key::Power, now) }` | `crates/skyward/src/ui.rs` |
| `use skyward_ui::app::{...}` 缺 `Key` | 补进 import | `crates/skyward/src/ui.rs` |
| 启动页写死 `Screen::Home` | 改 `Screen::Decrypt`，否则记住密码/过渡动画不生效 | `crates/skyward/src/main.rs` |

## 三、只合上游、不合并布丁分支的做法

如果只是想先把上游新版本跑起来：

```sh
git checkout -b up-new <当前分支>
# 用新上游整棵树覆盖（保留 README/docs/scripts/patches）
git add -A && git commit -m "upstream $NEW_TAG"
# 按 CUSTOMIZATIONS.md 重放四件套 → 编译 → 出包
```

## 四、升级后怎么验收

1. `cargo check -p skyward-ui` 过；
2. `scripts/build-android.sh` 出二进制；
3. `scripts/verify_image.py` 打印 `ALL CHECKS PASSED`；
4. 真机：开机先进解密页 → 触控正常 → 下拉控制中心 → 点一下有震动 → 关于本机品牌字样正确。

## 五、把升级交给 AI 时

直接把 [UPGRADE-PROMPT.md](../UPGRADE-PROMPT.md) 里的模板粘过去，再把两个上游的 tag 告诉它即可。

## 六、只挑 fix：上游大重构时的做法

`v0.3.0 → v0.3.1` 是一次典型情况，记下来省得下次重新判断。

### 先量规模

```sh
git fetch upstream --tags --depth=200
git diff --stat v0.3.0 v0.3.1 | tail -1      # 107 files, +15618 / -14393
git log --oneline v0.3.0..v0.3.1             # 14 个提交
git show --stat <sha>                        # 逐个看动了哪些文件
```

`v0.3.1` 的 14 个提交分成两类：

| 类别 | 提交 | 规模 | 处理 |
| --- | --- | --- | --- |
| 真修复 | `34dab67` DNS 多服务器回退 | `skyward-net/src/dns.rs` +103/−13 | ✅ 取 |
| 真修复 | `cca526b` 长日志拆成多条内核记录 | `skyward-sys/src/lib.rs` +93/−8 | ✅ 取 |
| 真修复 | `2ea0065` 电池/系统属性读不到时页面不崩 | 3 文件 +116/−20 | ✅ 取 |
| 真修复 | `990abf5` 写 boot 分区前先比对 | 10 文件 +277/−17 | ✅ 取 |
| 纯重构 | `0b4ee3e` `ui.rs` 状态机重排 | +1716/−1403 | ❌ 跳 |
| 纯重构 | `684a2f4` `app.rs` 11694 行拆成 `app/` 下 27 个模块 | 删 1 增 27 | ❌ 跳 |
| 纯重构 | `d0118df` edition 2024 | 全 workspace | ❌ 跳 |
| 纯重构 | `e5f3cf0` 共享 worker / hex 编码 | 15 个 crate | ❌ 跳 |
| 纯重构 | `31f3978` binder/crypto 拆文件 + lint | 10+ 文件 | ❌ 跳 |
| 平台 | `b696542` 让 MTP 在 Linux 主机上能编译 | `Cargo.toml` 4 行 | ❌ 跳（我们不在 Linux 出包） |
| 构建 | `3586240` 把工具链钉到 1.100.0 | `rust-toolchain.toml` | ✅ 取（只取这一处） |
| 构建 | `69a77ca` 手机端依赖清单 | 4 个 `Cargo.toml` | ❌ 跳 |

判断依据：**重构不带来任何功能差异，却会和布丁的改动正面撞车**（`app.rs`、`ui.rs` 正是布丁改得最多的两个文件）。跳过它们，补丁层和 CI 一行都不用动。

### 怎么做

```sh
git checkout -b v14 <上一版提交>
git cherry-pick 34dab67 cca526b 2ea0065 990abf5
```

这次四个全部**零冲突**自动合并——布丁改的位置与这几个 fix 不重叠。`990abf5` 给 `Overview` 加了 `held` 字段，需要顺带确认所有构造点都补了（`grep -rn "Overview {" crates/`），编译能过就没问题。

### 必须编译验证，别只看 diff

```sh
export PATH="/d/d/WorkBuddyAI/rustup/toolchains/1.100.0-x86_64-pc-windows-gnu/bin:/d/软件/应用/mingw64/mingw64/bin:$PATH"
export CARGO_HOME='D:\d\WorkBuddyAI\cargo'
export CC_aarch64_linux_android='D:\WorkBuddyAI\downloads\ndk_r29\android-ndk-r29\toolchains\llvm\prebuilt\windows-x86_64\bin\clang.exe'
export CXX_aarch64_linux_android='D:\WorkBuddyAI\downloads\ndk_r29\android-ndk-r29\toolchains\llvm\prebuilt\windows-x86_64\bin\clang++.exe'
export AR_aarch64_linux_android='D:\WorkBuddyAI\downloads\ndk_r29\android-ndk-r29\toolchains\llvm\prebuilt\windows-x86_64\bin\llvm-ar.exe'
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"
cargo check --workspace --target aarch64-linux-android
```

Windows 主机上要注意三点，缺一个都编不过：

1. `dlltool.exe` 在 `D:\软件\应用\mingw64\mingw64\bin`（PortableGit 里没有），过程宏要它；
2. NDK 路径必须写成 **Windows 形式**（`D:\...\clang.exe`），写成 `/d/...` 会报 `os error 3 系统找不到指定的路径`；
3. `cargo test` 在 Windows 上跑不了——`skyward-bootimg` 用了 `std::os::unix`，单元测试只能在 Linux 上跑。

### 验收

`cargo check --workspace --target aarch64-linux-android` 退出码 0 → 推 CI 出包 → `check_build.py` 自检通过 → 真机验解密/震动/WiFi/备份。

## 七、布丁仓库怎么跟

`mariusarva/skyward-pudding` 和上游**不是同一条线**：它是**小米 17（`pudding`）**的补丁层，整份 diff 压在上游某个 tag 上，本仓库是 **小米 17 Pro Max（`popsicle`）**、结构停在 0.3.0。两侧的 `devices/*.toml`、机型相关代码、UI 版本都不同，**补丁不能直接套**。

`scripts/sync_upstream.py` 会把它单独报一段（基线 `pudding` 字段对比 `pudding/main`），只回答「它到哪一版了、比我们多了哪些提交」。

### 它现在是什么状态

| 版本 | 是什么 |
| --- | --- |
| v0.2.49 | 本仓库跟的最后一版（v6 起的底座） |
| v0.2.50~52 | 明文 data 能开分区、首页重排、更新源提速 |
| **v0.2.55** | **从上游 v0.3.1 重做**：全功能移植 + OTG 修复（`patch -p1` 零失败） |
| v0.2.56 | 记住密码 / 文件管理排序 / 选项设置 的打磨 |
| v0.3.9 | 只改 `docs/update.json`（版本号带 v 导致解析失败，撤回） |

也就是说：**它已经整体搬到 0.3.1 上了，我们还在 0.3.0**。差距不是几个 fix，是那一整套 UI 重构。

### 要取它的改动，按这个流程

它的补丁只在仓库里的 `patches/skyward-pudding-v0.2.55.patch`（v0.2.56 的树里），是 `diff -ruN a/ b/` 格式。先还原出它那棵树，再逐文件比：

```bash
# 1) 上游那个 tag 的树
mkdir -p /d/WorkBuddyAI/tmp/upsync/up031 && git archive v0.3.1 | tar -x -C /d/WorkBuddyAI/tmp/upsync/up031

# 2) 打上它的补丁（实测零冲突）
cp -r /d/WorkBuddyAI/tmp/upsync/up031/* /d/WorkBuddyAI/tmp/upsync/pud055/
git show v0.2.56:patches/skyward-pudding-v0.2.55.patch > /d/WorkBuddyAI/tmp/upsync/pud055.patch
cd /d/WorkBuddyAI/tmp/upsync/pud055 && patch -p1 --batch -i ../pud055.patch

# 3) 把我们那棵树也按同一个索引导进来，直接 diff
cd /d/WorkBuddyAI/merge030
export GIT_INDEX_FILE="D:/WorkBuddyAI/tmp/upsync/idx055"
git --git-dir="D:/WorkBuddyAI/merge030/.git" --work-tree="D:/WorkBuddyAI/tmp/upsync/pud055" add -A
TREE=$(git --git-dir="D:/WorkBuddyAI/merge030/.git" write-tree)
git diff --stat HEAD $TREE
```

`git diff` 出来的每一处，判三件事：

1. **是不是它自己的真修复**（不是 0.3.1 重构带出来的差异）→ 看它 patch 里同一文件的 hunk，注释风格会写明「为什么」；
2. **我们是不是已经有了等价物**（`grep` 关键常量/函数名，很多功能 v6 就并进来了）；
3. **能不能脱离 0.3.1 结构单独移植** → 平台层的（`skyward-crypto`、`skyward-sys`、`skyward-net`）基本都能，UI 层的（`skyward-ui/src/app/*`）要重写。

### v15 这一轮实际取了什么

| 来源 | 改动 | 为什么 |
| --- | --- | --- |
| skyward `c5e09f6` | `wait_file()` + `hand_over` 里等文件出现（5 秒） | 驱动建节点晚于 insmod 返回，原来直接 chown 会失败 |
| skyward `cc68936` | `/vendor/lib64/hw` 进 vendor / odm 的库搜索路径 | Android 上 HAL 实现库在这个目录，原来找不到就起不来 |
| 布丁 v0.2.55 | 明文分区：映射前先探测文件系统，能读就直接用原设备 | 未加密的 data 原来也要走 dm-crypt，直接报错 |
| 布丁 v0.2.55 | `unmap()` 和失败清理前先 `dm::status` | 映射没建成时 `remove` 会再报一个错，把真正的原因盖掉 |
| 布丁 v0.2.55 | 无 `/data/unencrypted/mode` 时跳过用户解锁 | 明文分区没有这个文件，`open_shared` 会失败 |

`999` 分身跳过、`weaver_first` 的备用 blob、`gadget_state`（OTG）本仓库早就有，不用再取。

### v16 这一轮：把布丁「整体」过了一遍

v15 只挑了平台层三处修复。v16 是按用户要求把布丁 **44 个文件的补丁逐个甄别**一遍，结论比预想的干净：

**逐字节相同的（10 个，功能我们 v6 就有）**

`wlanmem.rs`、`sortmem.rs`、`remember.rs`、`portal.rs`、`fileserve.rs`、`fetch.rs`、`settings.rs`、`relabel.rs`、`pty.rs`、`keys.rs`，外加 `vendor/qrcodegen/`（二维码）与 `skyward-ui/src/progress.rs`（进度阶段）。这些从布丁 0.2.49 到 0.2.55 一个字没改。

**我们领先的（不取，取了是退步）**

| 文件 | 我们的做法 | 布丁的做法 |
| --- | --- | --- |
| `skyward-sys/usb.rs` | `switch()` 已在目标配置就跳过、只增功能就直接切，保 adb 会话 | 一律先 `none` 再设，adb 会断 |
| `skyward-net/download.rs` | `free()` 自己读 statvfs，带 `slow` 慢速判定 | 交给 `skyward_block::mount::space()`，无慢速判定 |
| `skyward/src/updates.rs` | `STALL` 8 秒 + `JUDGE_AFTER` 512KB + `MISSES` 2 次 | 只有重试，没有断流判定 |
| `skyward-net/github.rs` | 版本号支持裸计数（`v26` → 26.0.0） | 严格 semver，裸计数直接报错 |
| `skyward/src/wlan.rs` | `iface::set_up` 失败就每 100ms 重试到 `INTERFACE_WAIT` | 固定 `sleep(600ms)` 赌一次 |
| `skyward-programs/secure.rs` | `libraries_of` 带 `/vendor/lib64/hw` + `SHARED_LIBRARIES` | 旧的两段式，没有 `/hw` |

**是 0.3.1 重构、不是布丁功能的（不取）**

`skyward_sys::worker::{Worker, Teller}` 替换自建 pipe+mpsc（`updates`/`wlan`/`systems`/`sleep`/`install` 五处）、`encryption.rs` 从 `data.rs` 拆出、`waveform.rs` 从 `haptics.rs` 拆出（6 段曲线参数一字不差）、`hex::encode` 替代手写 `format!("{b:02x}")`、`let` chains 语法、`find_free_bytes` 换 `mount::space`。这些只是把同一件事写在不同文件里。

**真取的（5 处，其中 4 处是代码）**

| 改动 | 为什么 |
| --- | --- |
| `github.rs`：`MIRRORS` 8 个加速前缀进 `sources()` | 国内直连 github.com 常不通，原来只有「签名地址 + github.com」两条 |
| `http.rs`：新增 `probe_agent()`（连接/读各 3 秒） | 测速要能放弃不响应的镜像，否则一个死的拖住全部 |
| `updates.rs`：`by_speed()` 改**并发** + `FASTEST` 记忆 30 分钟 | 原来串行测 9 个地址最长 27 秒；并发 3 秒测完，且上次最快的先试 |
| `data.rs`：`Encryption::of` 支持只写 `keydirectory` 的表 | 这类分区原来直接 `bail!("not encrypted whole")`，解不开 |
| `main.rs`：`CANNOT_RUN_HERE` 跳过 weaver / secure_element 程序 | 它们在 recovery 里跑不起来，其中一个让设备等待 20 秒才失败 |

`skyward/src/secure.rs` 的 `write_descriptions`/`await_descriptions` 拆分没取：布丁自己也没在 `main.rs` 用（仍调 `publish_descriptions`），是没启用的半成品。

### 甄别方法（下次照这个走）

逐行 diff 会被 0.3.1 的风格重写淹掉。有效的做法是**只看补丁里的 `+` 行**，再按关键词核对本地：

```bash
# 1) 补丁里每个文件的 +行，只看 pub fn / const / struct / enum
python - <<'PY'
import re, pathlib
text = pathlib.Path('pud055.patch').read_text(encoding='utf-8', errors='replace')
for b in re.split(r'(?m)^(?=diff -ruN )', text)[1:]:
    print(b.split('\n',1)[0])
    for line in b.splitlines():
        s = line[1:].strip() if line.startswith('+') else ''
        if s and not s.startswith('//') and re.match(r'(pub )?(async )?fn |pub const |const |pub struct |struct ', s):
            print('   ', s[:100])
PY

# 2) 逐个关键词核对本地有没有
grep -rn "MIRRORS\|probe_agent\|FASTEST\|keydirectory" crates/
```

判据只有一条：**这个差异换到我们 0.3.0 结构上，用户能不能感觉到？** 感觉不到的就是重构。

## 八、拿对方的成品镜像做终检

比源码更硬的对照物是对方发布的 `.img`：解出来就是真机上跑的那份二进制。
本机有能跑的打包器（`target/release/skyward-pack.exe`，host 版），直接：

```bash
./target/release/skyward-pack.exe list  <对方.img>            # 条目清单
./target/release/skyward-pack.exe cat   <对方.img> system/bin/recovery > /tmp/theirs
./target/release/skyward-pack.exe cat   <对方.img> skyward/device-popsicle.toml > /tmp/theirs.toml
```

然后两件事：

1. **文案集合**：把 `crates/**/*.rs` 里所有含中文的字面量抓出来，逐条 `bytes.count()` 到对方二进制。
   全部命中 → 我们没有缺页面/缺标签；反向要小心：Rust 把相邻字面量连着放，直接取"中文连续段"
   会把两句话粘成一条，得逐位置试探最长缺失子串，粘出来的都是假差异。
2. **函数级**：`pages.rs` / `widgets.rs` / `files.rs` 按 `^fn` 切开逐个 `difflib`，
   注释、`let`-chain 写法、常量位置不同不算差异，只看**布丁独有且影响画面**的行。

判据同第七节：**换上用户的手机，他能不能看出来？** 能看出来就补，看不出来就是写法差异。
v19 就是这样抓到三处：解密页的进度条参数一直是 `None`、排序行画在画布外、
无线传输少一句提示。

## 九、布丁只在 release 镜像里改代码的时候（v0.3.7 起）

**tarball 里的源码不是源码。** `codeload` 下载的 `skyward-pudding-0.3.7` 解出来只有
`README.md`、`.github/workflows/build.yml`、`docs/CHANGES.md`、`docs/update.json`、
`docs/发布公告.md` 和若干 `.patch` —— 和 v0.2.55 那份比只多了一个补丁文件。
`git log v0.2.55..v0.3.9` 也只有 `update.json` 和补丁重生成。

也就是说：**作者是在别处改完、直接把 `recovery` 二进制打进镜像发布的**，
仓库里留下的只是壳。判断新版到底改了什么，只有两个来源：

1. `docs/发布公告.md` —— 中文清单，作者自己写的，最接近「改了什么」。
2. release 里的 `.img` —— 真机上跑的那份，唯一可信的事实。

流程：

```bash
# 1. 拿 release 里的 img（直连会断，走 gh-proxy；44 MB 约 130 秒）
curl -L -o pud.zip https://gh-proxy.com/https://github.com/mariusarva/skyward-pudding/releases/download/<tag>/<zip>
# 2. 用本机 host 版打包器解出来（Windows 路径，别给 /d/...）
./target/release/skyward-pack.exe cat <pud.img> system/bin/recovery > /tmp/theirs
# 3. 中文字符串集合比一遍（脚本见第八节）
python D:/WorkBuddyAI/tmp/diff_zh.py
```

对比结果的处理顺序，**先剔「我们已经有、只是写法不同」，再看真差异**：
v0.3.7 那份 63 条清单里，明文 data 兜底、`dm::status` 后 remove、跳过 999、
`weaver_first`、控制中心四磁贴与两个滑块、`fileserve`、`portal`、AVB 两开关、
界面优先启动，本仓库全都有。真正要写代码的只有三块：U 盘互传、文件多选、
记住密码的胶囊与风险窗。

**别照清单一条条当 TODO。** 清单是作者的变更记录，不是与你的差集：
先在自己的代码里 grep 一遍关键词（`999`、`weaver_first`、`umount_detach`、
`Magic`、`MIRRORS`……），命中的直接划掉。

## 十、反向：把这棵树推给 popsicle 存档仓库

上游另一头是 https://github.com/mariusarva/skyward-popsicle，它只有 `master` 一个分支、
两个提交（`2bffba0` 建库 + `afe1481` 校验和）、5 个文件：`device.toml`、
`README.md`、`CHANGES.md`、`SHA256SUMS-skyward-popsicle-v40`、`docs/解密方式.md`。
它不存源码，只存 v40 那一版的说明。

**我们没有这个仓库的写权限**（GitHub 返回的 `permissions` 是 `None`），`git push` 必被拒。
唯一的通路是 fork + PR：

| 角色 | 位置 | 说明 |
| --- | --- | --- |
| 基线 | `mariusarva/skyward-popsicle:master` | 注意是 **master**，不是 `main` |
| 头 | `Ff428/skyward-popsicle-1:v96` | fork 上的分支，PR #1 盯着它 |
| PR | https://github.com/mariusarva/skyward-popsicle/pull/1 | 作者手动合并 |

### 为什么是 force push，不是追加提交

这个 PR 的形态是「整棵树落进只有 5 个文件的仓库」，不是逐行补丁。每次同步把 head 分支
重建成**一个提交**，父是对方 `master`，树等于本仓库当前 HEAD 的树减去不该给的路径：

```
mariusarva:master (afe1481)
        └── sync: the <sha> tree  ← 树 = 本仓库 HEAD，去掉 backup/
```

好处是 PR 的 diff 永远只有两份树的一次比较：`14231 files / +416,243 / −26`，
不会每同步一次就多一层「上一版的树 → 这一版的树」的历史垃圾。代价是 head 分支的
旧提交会被覆盖（PR 页面会留一条 force-pushed 记录），这在这个形态里没有损失。

### 一次同步就两条命令

```bash
cd D:/WorkBuddyAI/merge030
python scripts/sync_to_popsicle.py            # 造树、打印 diff 统计，不动远端
python scripts/sync_to_popsicle.py --push     # 推 fork 的 v96，PR 自动刷新
```

脚本做的事（全在临时 index 里，不碰工作区、不碰当前分支）：

1. `GIT_INDEX_FILE=.git/sync-to-popsicle.index` + `git read-tree <HEAD>` 取本仓库的树
2. `git rm -r --cached backup .token` 去掉本机的东西（21 个文件）
3. `git write-tree` 得到新树，`git commit-tree <tree> -p popsicle/master` 造提交
4. `git push --force <fork> refs/heads/sync/<sha>:refs/heads/v96`

推送走 `20.27.177.113`（本机只有这个 `github.com` A 记录通），认证用
`D:\WorkBuddyAI\github\.token` 里的 PAT。默认参数就能用，要换源/换分支：

```bash
python scripts/sync_to_popsicle.py --source v98 --branch v96 --push
python scripts/sync_to_popsicle.py --dry-run          # 只算树，不造提交
python scripts/sync_to_popsicle.py --exclude backup docs/内部.md
```

### 对方的 master 前进了怎么办

脚本开头会 `git fetch popsicle master`；拿到新提交后，新树的父自动换到新的 `master` 上，
PR 依然是「一份整树 vs 一份整树」。**不要** merge 对方的 `master` 进本仓库：
两边是无共同祖先的两条历史（`git merge-base afe1481 HEAD` 输出为空），
merge 只会把 v40 的 5 个文件以冲突形式带进来。

### 对方合并之后

PR 变 `merged`，fork 的 `v96` 就没用了。下一次同步改成从对方新的 `master` 起：

```bash
git fetch popsicle master
python scripts/sync_to_popsicle.py --push     # 父已经是新的 master
```

如果 fork 的 `master` 落后（它自己也是 fork，默认不会自动同步），先在网页上点
`Sync fork`，或者 `git push --force <fork> popsicle/master:refs/heads/master`。

### 哪天拿到写权限

对方把 `Ff428` 加进 collaborators 之后，PR 这条链可以整个跳过：直接把本仓库的
`main` 推成对方的 `master`（同样是 force，理由同上——两条历史无共同祖先）：

```bash
git -c http.curloptResolve=github.com:443:20.27.177.113 push popsicle HEAD:refs/heads/master --force
```

在那之前，任何「直接把源码推到 mariusarva/skyward-popsicle」的命令都会得到
`remote: Permission to mariusarva/skyward-popsicle.git denied to Ff428`，这不是配置问题。

