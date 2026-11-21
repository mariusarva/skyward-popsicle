#!/usr/bin/env sh
# 交叉编译 skyward 到 aarch64-linux-android。
#   ANDROID_NDK_HOME=/path/to/android-ndk-r29 ./scripts/build-android.sh
# 省略参数时构建整个 workspace 里被依赖的 crate；产物在
#   target/aarch64-linux-android/release/skyward
set -eu

: "${ANDROID_NDK_HOME:?请先 export ANDROID_NDK_HOME 指向 Android NDK（r26+）}"

case "$(uname -s)" in
  Linux)  HOST=linux-x86_64 ;;
  Darwin) HOST=darwin-x86_64 ;;
  MINGW*|MSYS*|CYGWIN*) HOST=windows-x86_64 ;;
  *) echo "未知主机 $(uname -s)，请手动指定 HOST_TAG"; exit 1 ;;
esac

TC="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$HOST/bin"
[ -x "$TC/clang" ] || [ -x "$TC/clang.exe" ] || {
  echo "在 $TC 找不到 clang，检查 ANDROID_NDK_HOME"; exit 1; }

API=30
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TC/clang"
export CC_aarch64_linux_android="$TC/clang"
export AR_aarch64_linux_android="$TC/llvm-ar"
export CXX_aarch64_linux_android="$TC/clang++"
export CFLAGS_aarch64_linux_android="--target=aarch64-linux-android$API"
export CXXFLAGS_aarch64_linux_android="--target=aarch64-linux-android$API"

cd "$(dirname "$0")/.."
cargo build --release -p skyward --target aarch64-linux-android "$@"

BIN=target/aarch64-linux-android/release/skyward
ls -l "$BIN"
echo "下一步：cp $BIN work/ramdisk/system/bin/recovery"
