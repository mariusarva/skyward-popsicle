# 不用 GitHub，在本机出包

Actions 只是把一串命令放在 ubuntu 上跑。同一串命令在本机也能跑，一条：

```bash
python scripts/local_build.py
```

跑完 `out/` 里有 **一个裸 `.img`**（**不压缩**，跟 `v14-skyward-popsicle-...-ea931aa.img` 那种一样）
和一份对应该镜像的 SHA256SUMS，`stock/recovery.img` 留着下次复用。
再出一版时，`out/` 里旧的 img / zip 会自动挪进 `out/archive/`——目录里永远只有当前这一版。

```bash
python scripts/local_build.py --name v17        # 自己起名字
python scripts/local_build.py --deliver         # 出完放到交付目录，名字带提交号（v14 那种）
python scripts/local_build.py --skip-build      # 二进制没变，只重打包
python scripts/local_build.py --out-dir D:/out  # 产物放别处
python scripts/local_build.py --zip             # 只有明说才压成 .zip
python scripts/local_build.py --keep-old        # 不归档旧包（想留多版时用）
```

## 它做的八步

| 步 | 干什么 | 对应 CI 的哪一步 |
| --- | --- | --- |
| 1 | `cargo build -p skyward --target aarch64-linux-android` | Build the recovery service |
| 2 | `cargo build -p skyward-system-init`（`+crt-static`） | Build the second system's first program |
| 3 | `cargo build -p skyward-pack`（跑在电脑上） | Build the packer |
| 4 | `skyward-pack stock --out stock/recovery.img` | Take the recovery partition out |
| 5 | `python scripts/augment_stock.py`（注入 HAL 层） | Add the HAL layer |
| 6 | `skyward-pack recovery`（打包 + 重算 AVB） | Pack the recovery |
| 7 | `python scripts/check_build.py`（四项硬门槛） | Check the packed image |
| 8 | SHA-256（不压，直接给 img；`--zip` 才压） | Zip the image |

第 4 步有缓存：`stock/recovery.img` 已存在且摘要和 `devices/popsicle.toml` 里的
`[stock] recovery` 对得上就跳过，不重复下 100 MiB。想强制重下就删掉那个文件。

## 怎么出得快

一次全量交叉编译在这台机器上要十几分钟，增量出包约 **2 分钟**。差别全在
`target/` 有没有留住，以及触发了多少 crate 重编：

| 做法 | 为什么快 |
|---|---|
| 固定一个 `CARGO_TARGET_DIR`，**不要清** | 只重编改过的 crate + 链接；清一次等于全量 |
| 只换品牌时 `touch crates/skyward-ui/src/pages.rs` | 品牌串是 `option_env!`，只有 `skyward-ui` 与最终链接要重来，另外 22 个成员一行不碰 |
| 二进制没变（只改文档 / `device.toml` / 只换包名）用 `--skip-build` | 跳过第 1–3 步，只跑打包 + AVB + 校验 |
| openh264 是提交进仓库的静态库 | `build.rs` 只在 aarch64 target 下跑，且只在 `native/h264_shim.cpp` 或 `native/aarch64-linux-android/libopenh264.a` 的 mtime 变了才重跑；87 个对象不会每次重编 |
| 两个品牌连着出 | 第一版出完，第二版只多一次 `skyward-ui` 重编 + 打包 |

会让增量白攒的动作：换 `CARGO_TARGET_DIR`、清 `target/`、改 `Cargo.toml` 的
profile 或 `RUSTFLAGS`、改 `[workspace.package] version`（`Cargo.lock` 里 23 个
成员的 `version` 都要刷，每个 crate 的元数据哈希都变）。

## 需要什么

| 组件 | 本机实测位置 | 缺了会怎样 |
| --- | --- | --- |
| Rust 1.100.0 + `aarch64-linux-android` | `D:\d\WorkBuddyAI\rustup\toolchains\1.100.0-x86_64-pc-windows-gnu` | 脚本自己找；找不到会说要装什么 |
| Android NDK r29 | `D:\WorkBuddyAI\downloads\ndk_r29\android-ndk-r29` | 也可用 `ANDROID_NDK_HOME` 指 |
| mingw 的 `dlltool.exe`（仅 Windows） | `D:\软件\应用\mingw64\mingw64\bin` | 过程宏编不过 |
| `lz4.exe` | `D:\WorkBuddyAI\downloads\lz4` | 打包器压缩 ramdisk 时报错 |
| 带 `lz4` 模块的 Python | `~\.workbuddy\binaries\python\envs\default` | `augment_stock.py` 立刻退出 |

两样要自己下：

- `lz4.exe`：<https://github.com/lz4/lz4/releases> 的 `lz4_win64_*.zip`，解压到
  `D:\WorkBuddyAI\downloads\lz4\`
- Python 的 lz4：`<venv>\Scripts\python.exe -m pip install --proxy http://127.0.0.1:7890 lz4`
  （国内直连 pypi 常拿不到 `lz4` 的 wheel，走代理或换镜像）

代理：设 `SKYWARD_PROXY=http://127.0.0.1:7890` 会传给第 4 步的下载。

## 为什么 Windows 跑得起来

打包器和 recovery 共用的几个 crate 原本有几处 Unix 专用调用。它们在手机上永远
成立，在 Windows 上编不过，所以按平台分开了 —— **手机端的二进制一个字节都不变**，
`#[cfg(unix)]` 在 `aarch64-linux-android` 上恒真：

| 文件 | 原来 | 现在 |
| --- | --- | --- |
| `skyward-bootimg/src/ramdisk.rs` | 解包时 `mknod`/`symlink`/`lchown`/`utimensat` | 整段 `#[cfg(unix)]`；电脑端只用内存里的 `Ramdisk`，落盘只有手机上的 recovery 才做 |
| `skyward-block/src/erase.rs` | `libc::sysconf(_SC_PAGESIZE)` | 非 Unix 返回 4096；格式化分区本来只在手机上发生 |
| `skyward-programs/Cargo.toml` | 无条件依赖 `skyward-omapi` | 改成 `[target.'cfg(unix)'.dependencies]`，没有 Unix 就不引入那条 binder 链 |
| `skyward-programs/src/secure.rs` | `skyward_omapi::SERVICE` | 非 Unix 写出一份同名字面量；`:unix` 下有测试盯着两者不许漂 |
| `skyward-config/src/lib.rs` | `Path::is_absolute()` 判配置里的设备路径 | `is_from_the_top()`：只认开头是不是 `/`，Windows 不把 `/sbin/sh` 叫绝对路径 |

只有 `skyward-system-init` 的静态链接有个坑：设了 `RUSTFLAGS` 就会顶掉
`.cargo/config.toml` 里那组 `link-arg=--target=aarch64-linux-android30`，clang 于是
按主机平台链接，lld 报 `unknown argument: --eh-frame-hdr`。脚本里把
`-C link-arg=--target=aarch64-linux-android30` 一起写进 `RUSTFLAGS` 就对了。

## 和 CI 出来的包差在哪

同一套检查都过：ramdisk **1279 条**、15 个关键条目带可执行位、AVB 摘要与 footer 自洽、
镜像总长等于 100 MiB 分区。

不完全一样的是镜像内部那段 ramdisk 压缩数据：本机 `lz4.exe` 是 v1.10.0，ubuntu-24.04
自带的是 v1.9.4，`-12 --favor-decSpeed` 两版压出来长度不同（所以本机 img 里留白少一点，
包成 zip 时差 7 MB 左右）。两个 img 都是同样的 100 MiB 分区、同样的条目和 AVB，刷机与校验一致。

要发布、要留一份可复现的构建记录，还是推 main 让 Actions 出；本机这条路的用处是
断网、额度用完、Actions 抽风、或者只是想快点看到成品。

## 常见故障

| 现象 | 原因 |
| --- | --- |
| `lld: error: unknown argument: --eh-frame-hdr` | `RUSTFLAGS` 顶掉了 `--target`，见上一节 |
| `[haptics] resonance: /skyward/resonance is not an absolute path` | 旧版代码用 Windows 的绝对路径判定，`skyward-config` 的 `is_from_the_top` 就是修这个 |
| `需要 lz4 模块：pip install lz4` | 用错了 Python，要用带 lz4 的那个 venv |
| `run lz4 (install it, e.g. brew install lz4)` | `lz4.exe` 不在 PATH，脚本会自己去 `D:\WorkBuddyAI\downloads\lz4` 找 |
| 第 4 步卡住不动 | 官方包在境外 OSS，设 `SKYWARD_PROXY` |
| `取出来的分区摘要不对` | 官方包被换过或下了半截，删 `stock/recovery.img` 重来 |
