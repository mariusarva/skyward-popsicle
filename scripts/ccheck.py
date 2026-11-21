#!/usr/bin/env python3
"""只做类型检查，不出镜像：

    python scripts/ccheck.py -p skyward -p skyward-ui

用的环境跟 local_build.py 一模一样（同一套 cargo、NDK、共享 target 目录），
所以检查过的依赖，出包时不用再编一遍。默认 --target-dir 是
D:/WorkBuddyAI/tmp/target-v44（CARGO_TARGET_DIR 可覆盖）。
"""
import argparse
import importlib.util
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TARGET = "aarch64-linux-android"
BUILD = Path(os.environ.get("CARGO_TARGET_DIR", "").strip() or r"D:\WorkBuddyAI\tmp\target-v44")


def load_build():
    spec = importlib.util.spec_from_file_location("local_build", REPO / "scripts" / "local_build.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-p", "--package", action="append", required=True)
    ap.add_argument("--target-dir", default=str(BUILD))
    args = ap.parse_args()

    lb = load_build()
    cargo, cargo_bin = lb.find_cargo()
    ndk = lb.find_ndk()
    dlltool = lb.find_dlltool()
    lz4 = lb.find_lz4()

    env = dict(os.environ)
    env["PATH"] = os.pathsep.join([cargo_bin, str(ndk), lz4] + ([dlltool] if dlltool else []) + [env.get("PATH", "")])
    env["CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"] = lb.win(ndk / "clang.exe")
    env["CC_aarch64_linux_android"] = lb.win(ndk / "clang.exe")
    env["CXX_aarch64_linux_android"] = lb.win(ndk / "clang++.exe")
    env["AR_aarch64_linux_android"] = lb.win(ndk / "llvm-ar.exe")
    proxy = os.environ.get("SKYWARD_PROXY")
    if proxy:
        env["http_proxy"] = env["https_proxy"] = proxy

    cmd = [cargo, "check", "--locked", "--release", "--target", TARGET, "--target-dir", args.target_dir]
    for p in args.package:
        cmd += ["-p", p]
    print(" ".join(cmd), flush=True)
    sys.exit(subprocess.call(cmd, env=env, cwd=REPO))


if __name__ == "__main__":
    main()
