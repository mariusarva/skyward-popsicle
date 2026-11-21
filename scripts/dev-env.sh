#!/usr/bin/env bash
# 在本机准备「交叉编译到 aarch64-linux-android」的环境。
#
#   source scripts/dev-env.sh          # 自动寻找工具链、NDK、mingw
#   cargo check --workspace --target aarch64-linux-android
#
# 只影响当前 shell，不改任何系统设置。找不到的组件会明确说出缺什么、装到哪。
# 需要联网装依赖时先 export SKYWARD_PROXY=http://127.0.0.1:7890 再 source。

set -u

REPO="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
WIN=0
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) WIN=1 ;; esac

# cc crate 只认 Windows 形式的路径；Linux 上原样返回。
winpath() {
  if [ "$WIN" = "1" ] && command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s' "$1"
  fi
}

fail=0
note() { printf '  %-14s %s\n' "$1" "$2"; }

CHANNEL="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$REPO/rust-toolchain.toml" 2>/dev/null)"
[ -n "$CHANNEL" ] || CHANNEL="1.100.0"

echo "== 定位工具链（Rust $CHANNEL）=="

# rustup 的代理会重装工具链，所以优先直接用 toolchain 里的绝对路径。
HOST=""
case "$(uname -m)" in
  x86_64|amd64)
    [ "$WIN" = "1" ] && HOST="x86_64-pc-windows-gnu" || HOST="x86_64-unknown-linux-gnu" ;;
  aarch64|arm64)
    [ "$WIN" = "1" ] && HOST="aarch64-pc-windows-gnu" || HOST="aarch64-unknown-linux-gnu" ;;
esac

CARGO_BIN=""
for root in "${RUSTUP_HOME:-}" "$HOME/.rustup" "/d/d/WorkBuddyAI/rustup" "D:/d/WorkBuddyAI/rustup"; do
  [ -n "$root" ] || continue
  cand="$root/toolchains/$CHANNEL-$HOST/bin"
  if [ -x "$cand/cargo" ] || [ -x "$cand/cargo.exe" ]; then
    CARGO_BIN="$cand"
    break
  fi
done

if [ -n "$CARGO_BIN" ]; then
  export PATH="$CARGO_BIN:$PATH"
  note "cargo" "$CARGO_BIN"
else
  if command -v cargo >/dev/null 2>&1; then
    note "cargo" "$(command -v cargo)（PATH 里已有的，版本可能不是 $CHANNEL）"
  else
    note "cargo" "✗ 没找到。装 rustup 后执行："
    echo "                 rustup toolchain install $CHANNEL --target aarch64-linux-android"
    fail=1
  fi
fi

# 编译器要能找到 aarch64-linux-android 的 std，没有就提示。
if command -v rustc >/dev/null 2>&1; then
  if ! rustc --print target-list 2>/dev/null | grep -qx aarch64-linux-android; then
    note "target" "✗ 这个 rustc 不认识 aarch64-linux-android"
    fail=1
  fi
fi

echo "== 定位 mingw（过程宏要 dlltool）=="
if [ "$WIN" = "1" ]; then
  MINGW=""
  for d in "/d/软件/应用/mingw64/mingw64/bin" "/c/msys64/mingw64/bin" "/c/mingw64/bin" \
           "/d/软件/应用/PortableGit/mingw64/bin"; do
    [ -x "$d/dlltool.exe" ] && { MINGW="$d"; break; }
  done
  if [ -n "$MINGW" ]; then
    export PATH="$MINGW:$PATH"
    note "dlltool" "$MINGW"
  else
    note "dlltool" "✗ 没找到 dlltool.exe，过程宏会编译失败。装 MSYS2/mingw-w64 后重试。"
    fail=1
  fi
else
  note "mingw" "本机不需要"
fi

echo "== 定位 Android NDK =="
NDK_BIN=""
if [ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "$ANDROID_NDK_HOME" ]; then
  NDK_BIN="$(echo "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/bin)"
fi
if [ -z "$NDK_BIN" ] || [ ! -d "$NDK_BIN" ]; then
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) PRE="windows-x86_64" ;;
    Darwin)               PRE="darwin-x86_64" ;;
    *)                    PRE="linux-x86_64" ;;
  esac
  for d in /d/WorkBuddyAI/downloads/ndk*/android-ndk-*/toolchains/llvm/prebuilt/$PRE/bin \
           "$HOME/AppData/Local/Android/Sdk/ndk"/*/toolchains/llvm/prebuilt/$PRE/bin \
           "$HOME/Library/Android/sdk/ndk"/*/toolchains/llvm/prebuilt/$PRE/bin \
           "$HOME/Android/Sdk/ndk"/*/toolchains/llvm/prebuilt/$PRE/bin \
           /opt/android-ndk*/toolchains/llvm/prebuilt/$PRE/bin; do
    [ -x "$d/clang" ] || [ -x "$d/clang.exe" ] && { NDK_BIN="$d"; break; }
  done
fi

if [ -n "$NDK_BIN" ] && [ -d "$NDK_BIN" ]; then
  CLANG="$(winpath "$NDK_BIN/clang.exe")"
  [ -x "$NDK_BIN/clang.exe" ] || CLANG="$(winpath "$NDK_BIN/clang")"
  export CC_aarch64_linux_android="$CLANG"
  export CXX_aarch64_linux_android="$(winpath "$NDK_BIN/clang++.exe")"
  [ -x "$NDK_BIN/clang++.exe" ] || export CXX_aarch64_linux_android="$(winpath "$NDK_BIN/clang++")"
  export AR_aarch64_linux_android="$(winpath "$NDK_BIN/llvm-ar.exe")"
  [ -x "$NDK_BIN/llvm-ar.exe" ] || export AR_aarch64_linux_android="$(winpath "$NDK_BIN/llvm-ar")"
  export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CLANG"
  note "NDK" "$NDK_BIN"
  note "CC" "$CC_aarch64_linux_android"
else
  note "NDK" "✗ 没找到。装 NDK r29 后设 ANDROID_NDK_HOME 指过去。"
  fail=1
fi

if [ -n "${SKYWARD_PROXY:-}" ]; then
  export http_proxy="$SKYWARD_PROXY" https_proxy="$SKYWARD_PROXY"
  note "代理" "$SKYWARD_PROXY"
fi

echo
if [ "$fail" = "0" ]; then
  echo "就绪。可以跑："
  echo "  cargo check --workspace --target aarch64-linux-android"
else
  echo "有组件没找到（见上面的 ✗）。只跑 CI 出包的话不装这些也行，"
  echo "推 scripts/push.py 就够了；本机编译验证才需要全部配齐。"
fi
