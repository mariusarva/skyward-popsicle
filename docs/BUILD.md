# 构建（交叉编译到 aarch64-linux-android）

> 只想出一个能刷的包，看 [LOCAL-BUILD.md](LOCAL-BUILD.md)：`python scripts/local_build.py`
> 一条命令跑完编译、取原厂分区、补 HAL 层、打包、自检、压缩。本文是手工编译那一层的细节。

## 需要什么

| 组件 | 版本 / 说明 |
| --- | --- |
| Rust | 1.100.0（任意近期 stable 均可），并装 `aarch64-linux-android` 标准库 |
| Android NDK | r26 以上，实测 r29；只要 `toolchains/llvm/prebuilt/<host>/bin/clang` |
| Python 3 | 出包脚本用，需要 `lz4` 模块 |
| MinGW gcc（仅 Windows 主机） | 部分 crate 的 `build.rs` 要在主机上编 C 代码，MSVC 工具链下会找不到 gcc |

装标准库：

```sh
rustup target add aarch64-linux-android
```

如果 `rustup target add` 卡在下载，可换镜像（国内）：

```sh
export RUSTUP_DIST_SERVER=https://rsproxy.cn
export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
rustup target add aarch64-linux-android
```

## 编译

```sh
export ANDROID_NDK_HOME=/path/to/android-ndk-r29
./scripts/build-android.sh          # 等价于下面这一串环境变量 + cargo build
```

等价的手工命令：

```sh
TC=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin   # Windows: windows-x86_64
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$TC/clang
export CC_aarch64_linux_android=$TC/clang
export AR_aarch64_linux_android=$TC/llvm-ar
export CXX_aarch64_linux_android=$TC/clang++
export CFLAGS_aarch64_linux_android="--target=aarch64-linux-android30"
export CXXFLAGS_aarch64_linux_android="--target=aarch64-linux-android30"

cargo build --release -p skyward --target aarch64-linux-android
```

产物：`target/aarch64-linux-android/release/skyward`（约 8.6 MiB，未 strip）。

`.cargo/config.toml` 里预先写好了 `link-arg=--target=aarch64-linux-android30`（API 30）与国内 crates 镜像；换网络环境时把那几行 `[source.crates-io]` 删掉即可。

## 只编 UI / 只查错

```sh
cargo check -p skyward-ui                      # 主机上做类型检查，最快
cargo build --release -p skyward --target aarch64-linux-android   # 完整出包用
```

## 常见报错

| 报错 | 原因与处理 |
| --- | --- |
| `can't find crate for 'std'` / `E0463` | 没装 Android 标准库：`rustup target add aarch64-linux-android`。注意别让 `rust-toolchain.toml` 把版本切到没有该 target 的 toolchain 上（`rustup toolchain list` 里挑带 `aarch64-linux-android` 的那个，用绝对路径调它的 `cargo`） |
| `error: linker 'clang' not found` | `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` 没设或路径不对 |
| `gcc not found`（Windows 主机） | 主机缺 gcc：把 MinGW 的 `bin` 加进 `PATH`，或改用 `-gnu` 版 toolchain |
| 编译成功但文件被锁 / `拒绝访问 (os error 5)` | 杀毒软件（腾讯/阿里组件）在扫新生成的二进制，重跑一次通常就过；顽固的话把 `target/` 加白名单 |
| `rustup` 开始自动下载 toolchain | `rust-toolchain.toml` 指定的通道与当前 toolchain 不符。直接用目标 toolchain 的绝对路径运行 `cargo.exe`，或在环境里显式设置 `RUSTUP_TOOLCHAIN` |

## 版本号

`crates/skyward-info/build.rs` 会把 git 描述写进二进制，关于页的版本行即来自这里；本分支没改它，仍显示上游的 `0.3.0`。想让它显示自己的版本，改 `crates/skyward-info/src/lib.rs` 的 `name()` 或对应 build 脚本即可。
