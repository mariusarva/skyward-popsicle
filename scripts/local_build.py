#!/usr/bin/env python3
"""不用 GitHub Actions，在本机把 popsicle 的 recovery 镜像打出来。

    python scripts/local_build.py                 # 出裸 .img 到 out/（旧包自动归档到 out/archive/）
    python scripts/local_build.py --name v17      # 自己起名字
    python scripts/local_build.py --deliver       # 出完放到交付目录，名带提交号（v14 那种）
    python scripts/local_build.py --skip-build    # 二进制没变，只重打包
    python scripts/local_build.py --zip           # 想压缩包时明说，默认不压

它做的是 .github/workflows/build.yml 里那几件事，一步不少：

  1. cargo 交叉编译 recovery 服务（aarch64-linux-android）
  2. cargo 交叉编译静态的 system-init
  3. cargo 编译跑在电脑上的打包器 skyward-pack
  4. 从官方完整包取出原厂 recovery 分区（有缓存就不重复下）
  5. scripts/augment_stock.py 把 HAL 补丁层注入原厂 ramdisk
  6. skyward-pack recovery 打包并重算 AVB
  7. scripts/check_build.py 四项硬门槛
  8. 算 SHA-256（默认交裸 .img，跟 v14 那个一样；要给压缩包就加 --zip）

Windows 上要有的东西（脚本会自己找，找不到会说缺什么）：
  Rust 1.100.0（rust-toolchain.toml 定的版本）、Android NDK r29、
  mingw 的 dlltool.exe、lz4.exe、带 lz4 模块的 Python。

Windows 跑得起来是因为 src 里几处 Unix 专用的代码加了条件编译，
它们只在手机上生效，编出来的 Android 二进制与 CI 完全一样：

  skyward-bootimg/ramdisk.rs   解包到文件系统（设备节点、符号链接、属主）
  skyward-block/erase.rs       页大小（sysconf）
  skyward-programs/Cargo.toml  omapi 只在有 Unix 时引入
  skyward-programs/secure.rs   安全元件服务名，没有 omapi 时写出来
  skyward-config/lib.rs        「绝对路径」按手机的判定，不是 Windows 的
"""
import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import time
import tomllib
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
DEVICE = "popsicle"
API = "30"
TARGET = "aarch64-linux-android"

# --deliver 用的交付目录（WorkBuddy 任务目录下的 output）
DELIVER = Path(os.environ.get("SKYWARD_OUTPUT_DIR", "").strip()
               or r"D:\WorkBuddyAI\2026-11-13-20-45-53\output")

# 构建目录：默认仓库里的 target。target 下的 .d 偶尔会被安全软件锁住，
# 写进去报 os error 5（2026-11-15 连试两次都断在不同的 crate 上），
# 这时把 CARGO_TARGET_DIR 指到别处重跑即可；脚本里的三条 cargo 命令
# 和取产物的路径都跟这个值走。
BUILD = Path(os.environ.get("CARGO_TARGET_DIR", "").strip() or (REPO / "target"))


def say(step, text):
    print(f"[{step}] {text}", flush=True)


def die(text):
    print(f"\n✗ {text}\n", file=sys.stderr)
    sys.exit(1)


def win(path):
    """cc crate 只认 Windows 形式的路径；别的系统原样返回。"""
    if os.name == "nt":
        return str(Path(path))
    return str(path)


# ---------------------------------------------------------------- 找工具

def find_cargo():
    """按 rust-toolchain.toml 的版本找 toolchain 里的 cargo，绕开 rustup 代理。"""
    channel = "1.100.0"
    toml = REPO / "rust-toolchain.toml"
    if toml.exists():
        cfg = tomllib.loads(toml.read_text(encoding="utf-8"))
        channel = cfg.get("toolchain", {}).get("channel", channel)
    host = "x86_64-pc-windows-gnu" if os.name == "nt" else "x86_64-unknown-linux-gnu"
    roots = [os.environ.get("RUSTUP_HOME"), Path.home() / ".rustup",
             Path("D:/d/WorkBuddyAI/rustup"), Path("D:/WorkBuddyAI/rustup")]
    for root in roots:
        if not root:
            continue
        bindir = Path(root) / "toolchains" / f"{channel}-{host}" / "bin"
        for exe in ("cargo.exe", "cargo"):
            if (bindir / exe).exists():
                return str(bindir / exe), str(bindir)
    found = shutil.which("cargo")
    if found:
        return found, str(Path(found).parent)
    die(f"没找到 cargo {channel}。装一下：rustup toolchain install {channel} --target {TARGET}")


def find_ndk():
    """Android NDK 的 clang 目录。"""
    given = os.environ.get("ANDROID_NDK_HOME")
    hosts = ["windows-x86_64"] if os.name == "nt" else ["linux-x86_64"]
    roots = []
    if given:
        roots.append(Path(given))
    downloads = Path("D:/WorkBuddyAI/downloads")
    if downloads.exists():
        roots += sorted(downloads.glob("ndk*/android-ndk-*"))
    for root in roots:
        for host in hosts:
            bindir = root / "toolchains" / "llvm" / "prebuilt" / host / "bin"
            for exe in ("clang.exe", "clang"):
                if (bindir / exe).exists():
                    return bindir
    die("没找到 Android NDK。装 r29 后设 ANDROID_NDK_HOME 指过去，"
        "或放到 D:/WorkBuddyAI/downloads/ndk_r29/android-ndk-r29")


def find_dlltool():
    """过程宏要 dlltool，Windows 上必须能从 PATH 里找到。"""
    if os.name != "nt":
        return None
    cands = [Path("D:/软件/应用/mingw64/mingw64/bin"), Path("C:/msys64/mingw64/bin"),
             Path("C:/mingw64/bin")]
    for d in cands:
        if (d / "dlltool.exe").exists():
            return str(d)
    if shutil.which("dlltool"):
        return str(Path(shutil.which("dlltool")).parent)
    die("没找到 dlltool.exe，过程宏编译不过。装 MSYS2/mingw-w64 后重试。")


def find_lz4():
    """打包器压缩 ramdisk 时调的是 lz4 命令行。"""
    found = shutil.which("lz4")
    if found:
        return str(Path(found).parent)
    for cand in (Path("D:/WorkBuddyAI/downloads/lz4"), Path("C:/Program Files/lz4")):
        for exe in ("lz4.exe", "lz4"):
            if (cand / exe).exists():
                return str(cand)
    die("没找到 lz4 命令。从 github.com/lz4/lz4/releases 下 lz4_win64_*.zip，"
        "解压到 D:/WorkBuddyAI/downloads/lz4")


def find_python():
    """要一个能 import lz4.block 的 Python，augment/check 都要用。"""
    try:
        import lz4.block  # noqa: F401
        return sys.executable
    except ImportError:
        pass
    cands = [Path.home() / ".workbuddy/binaries/python/envs/default/Scripts/python.exe",
             Path.home() / ".workbuddy/binaries/python/envs/default/bin/python"]
    for p in cands:
        if p.exists():
            probe = subprocess.run([str(p), "-c", "import lz4.block"], capture_output=True)
            if probe.returncode == 0:
                return str(p)
    die("当前 Python 没有 lz4 模块。装一下："
        f"{sys.executable} -m pip install lz4")


# ---------------------------------------------------------------- 跑命令

def run(cmd, env=None, echo=False):
    started = time.time()
    proc = subprocess.run(cmd, cwd=REPO, env=env, capture_output=True, text=True,
                          encoding="utf-8", errors="replace")
    if proc.returncode != 0:
        tail = (proc.stdout or "")[-1500:] + (proc.stderr or "")[-2500:]
        die(f"失败（{time.time() - started:.0f}s）：{' '.join(map(str, cmd))}\n\n{tail}")
    if echo:
        for line in (proc.stdout or "").splitlines()[-3:]:
            print(f"      {line}")
    return proc.stdout or ""


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


# ---------------------------------------------------------------- 各步骤

def build(env):
    say("1/8", "交叉编译 recovery 服务")
    run([env["CARGO"], "build", "--locked", "--release", "-p", "skyward",
         "--target", TARGET, "--target-dir", str(BUILD)], env=env, echo=True)

    say("2/8", "交叉编译静态的 system-init")
    # RUSTFLAGS 一旦设置就会顶掉 .cargo/config.toml 里那组 rustflags，
    # 而 --target=aarch64-linux-android30 丢了的话 clang 会按主机平台链接，
    # lld 于是不认 --eh-frame-hdr 和 -z。所以这里把它补回来。
    static = dict(env)
    static["RUSTFLAGS"] = f"-C target-feature=+crt-static -C link-arg=--target={TARGET}{API}"
    run([env["CARGO"], "build", "--locked", "--release", "-p", "skyward-system-init",
         "--target", TARGET, "--target-dir", str(BUILD / "system-init")], env=static, echo=True)

    say("3/8", "编译电脑上跑的打包器")
    run([env["CARGO"], "build", "--locked", "--release", "-p", "skyward-pack", "--target-dir", str(BUILD)],
        env=env, echo=True)

    exe = "skyward-pack.exe" if os.name == "nt" else "skyward-pack"
    paths = {
        "pack": BUILD / "release" / exe,
        "skyward": BUILD / TARGET / "release" / "skyward",
        "system-init": BUILD / "system-init" / TARGET / "release" / "system-init",
    }
    for name, p in paths.items():
        if not p.exists():
            die(f"编译完了却没看到 {name}：{p}")
    return paths


def take_stock(env, config):
    """原厂 recovery 分区：有缓存且校验过就不重下。"""
    stock = REPO / "stock" / "recovery.img"
    want = tomllib.loads(config.read_text(encoding="utf-8"))["stock"]["recovery"]
    if stock.exists() and sha256_of(stock) == want:
        say("4/8", f"原厂 recovery 已在缓存里（{stock.stat().st_size} 字节，摘要对得上）")
        return stock
    say("4/8", "从官方完整包取出原厂 recovery 分区（只读这个分区的字节）")
    stock.parent.mkdir(parents=True, exist_ok=True)
    out = run([str(env["PACKER_BIN"]), "stock", "--config", str(config),
               "--out", str(stock)], env=env)
    for line in out.splitlines():
        print(f"      {line}")
    if not stock.exists():
        die("没写出 stock/recovery.img")
    got = sha256_of(stock)
    if got != want:
        die(f"取出来的分区摘要不对：\n  期望 {want}\n  实际 {got}")
    return stock


def augment(py, stock):
    say("5/8", "把 HAL 补丁层注入原厂 ramdisk")
    base = REPO / "stock" / "base.img"
    out = run([py, str(REPO / "scripts" / "augment_stock.py"),
               "--stock", str(stock),
               "--hal", str(REPO / "resources" / DEVICE / "recovery-hal.zip"),
               "--out", str(base)])
    for line in out.strip().splitlines():
        print(f"      {line}")
    return base


def pack(env, paths, base, config, out_dir, name):
    say("6/8", "打包 recovery 并重算 AVB")
    image = out_dir / f"{name}.img"
    out = run([str(paths["pack"]), "recovery", "--stock", str(base),
               "--binary", str(paths["skyward"]),
               "--system-init", str(paths["system-init"]),
               "--config", str(config),
               "--font", str(REPO / "resources" / "fonts" / "MiSansVF.ttf"),
               "--mono", str(REPO / "resources" / "fonts" / "KirkMono-Regular.ttf"),
               "--mono-license", str(REPO / "resources" / "fonts" / "KirkMono-OFL.txt"),
               "--out", str(image)], env=env)
    for line in out.strip().splitlines():
        print(f"      {line}")
    return image


def check(py, image):
    say("7/8", "四项硬门槛自检")
    out = run([py, str(REPO / "scripts" / "check_build.py"), "--img", str(image)])
    for line in out.strip().splitlines():
        print(f"      {line}")


def finalize(image, want_zip):
    """默认就交裸 .img（v14 那种），--zip 才压成压缩包。"""
    if not want_zip:
        say("8/8", "算 SHA-256（不压缩）")
        digest = sha256_of(image)
        sums = image.parent / "SHA256SUMS"
        # 只记当前这一版：旧包已经归档，清单跟着包走，不留历史行。
        with open(sums, "w", encoding="utf-8", newline="\n") as f:
            f.write(f"{digest}  {image.name}\n")
        print(f"      {image.name}  {image.stat().st_size} 字节")
        print(f"      sha256 {digest}")
        return image, digest

    say("8/8", "压缩并算 SHA-256")
    zipped = image.with_suffix(".zip")
    with zipfile.ZipFile(zipped, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        z.write(image, image.name)
    image.unlink()
    digest = sha256_of(zipped)
    sums = zipped.parent / "SHA256SUMS"
    with open(sums, "w", encoding="utf-8", newline="\n") as f:
        f.write(f"{digest}  {zipped.name}\n")
    print(f"      {zipped.name}  {zipped.stat().st_size} 字节")
    print(f"      sha256 {digest}")
    return zipped, digest


# ---------------------------------------------------------------- 入口

def archive_older(out_dir):
    """out/ 只放当前这一版：旧的 img / zip 挪进 out/archive/，不删。"""
    archive = out_dir / "archive"
    olds = sorted(p for p in out_dir.iterdir()
                  if p.is_file() and p.suffix in (".img", ".zip"))
    if not olds:
        return
    archive.mkdir(exist_ok=True)
    for p in olds:
        shutil.move(str(p), str(archive / p.name))
        print(f"   旧包归档 {p.name} → archive/")


def short_commit():
    """交付名要带提交号（v14-skyward-...-ea931aa.img 那个 ea931aa）。"""
    for exe in ("git", r"C:/Users/F/.workbuddy/binaries/PortableGit/versions/1.2.0/cmd/git.exe"):
        try:
            r = subprocess.run([exe, "rev-parse", "--short=7", "HEAD"], cwd=REPO,
                               capture_output=True, text=True)
            if r.returncode == 0:
                return r.stdout.strip()
        except OSError:
            continue
    return "local"


def deliver(image, tag, stock_version):
    """拷到交付目录，名字按 v14 那个规范：<版本>-skyward-<设备>-<系统>-<提交>.img。"""
    target = DELIVER / f"{tag}-skyward-{DEVICE}-{stock_version}-{short_commit()}{image.suffix}"
    say("交付", f"拷到 {target.name}")
    if not DELIVER.is_dir():
        say("交付", f"没有 {DELIVER}，跳过")
        return image
    # 交付目录只留当前这一版：旧的 recovery 镜像和压缩包都挪进 archive/，
    # 不留任何别的包跟它并排（v18 之前这里只扫 .zip，换成裸 img 后漏了）。
    archive = DELIVER / "archive"
    for p in sorted(DELIVER.iterdir()):
        if not p.is_file() or p.suffix.lower() not in (".img", ".zip") or p.name == target.name:
            continue
        archive.mkdir(exist_ok=True)
        shutil.move(str(p), str(archive / p.name))
        print(f"   旧包归档 {p.name} → archive/")
    shutil.copy2(image, target)
    with open(DELIVER / "SHA256SUMS", "w", encoding="utf-8", newline="\n") as f:
        f.write(f"{sha256_of(target)}  {target.name}\n")
    print(f"   {target.name}  {target.stat().st_size} 字节")
    return target


def main():
    ap = argparse.ArgumentParser(description="在本机把 popsicle 的 recovery 镜像打出来")
    ap.add_argument("--name", help="产物名字，默认 v<版本>-local-<日期>")
    ap.add_argument("--out-dir", default="out", help="产物目录，默认 out")
    ap.add_argument("--config", default=f"devices/{DEVICE}.toml")
    ap.add_argument("--skip-build", action="store_true", help="不重编，直接用现有二进制")
    ap.add_argument("--keep-old", action="store_true",
                    help="不在出包前把 out/ 里的旧 img/zip 归档（默认归档到 out/archive/）")
    ap.add_argument("--zip", action="store_true",
                    help="压成 .zip（默认直接交裸 .img）")
    ap.add_argument("--deliver", action="store_true",
                    help="出包后拷到交付目录，命名带提交号")
    ap.add_argument("--deliver-tag", help="交付文件名前缀，默认取 --name 的第一段（如 v16）")
    args = ap.parse_args()

    config = REPO / args.config
    if not config.exists():
        die(f"没有配置文件 {config}")

    cargo, cargo_bin = find_cargo()
    ndk = find_ndk()
    dlltool = find_dlltool()
    lz4 = find_lz4()
    py = find_python()

    project = tomllib.loads((REPO / "Cargo.toml").read_text(encoding="utf-8"))
    version = project["workspace"]["package"]["version"]
    stock_version = tomllib.loads(config.read_text(encoding="utf-8"))["stock"]["version"]
    name = args.name or f"v{version}-local-{time.strftime('%Y%m%d')}"
    full = f"skyward-{name}-{DEVICE}-{stock_version}"

    env = dict(os.environ)
    env["PATH"] = os.pathsep.join([cargo_bin, str(ndk), lz4] + ([dlltool] if dlltool else [])
                                  + [env.get("PATH", "")])
    env["CARGO"] = cargo
    env["CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"] = win(ndk / ("clang.exe" if os.name == "nt" else "clang"))
    env["CC_aarch64_linux_android"] = win(ndk / ("clang.exe" if os.name == "nt" else "clang"))
    env["CXX_aarch64_linux_android"] = win(ndk / ("clang++.exe" if os.name == "nt" else "clang++"))
    env["AR_aarch64_linux_android"] = win(ndk / ("llvm-ar.exe" if os.name == "nt" else "llvm-ar"))
    proxy = os.environ.get("SKYWARD_PROXY")
    if proxy:
        env["http_proxy"] = env["https_proxy"] = proxy

    print(f"\n== 出包 {full} ==")
    print(f"   cargo      {cargo}")
    print(f"   NDK        {ndk}")
    print(f"   dlltool    {dlltool or '本机不需要'}")
    print(f"   lz4        {lz4}")
    print(f"   python     {py}\n")

    out_dir = Path(args.out_dir)
    if not out_dir.is_absolute():
        out_dir = REPO / out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    if not args.keep_old:
        archive_older(out_dir)

    exe = "skyward-pack.exe" if os.name == "nt" else "skyward-pack"
    env["PACKER_BIN"] = str(BUILD / "release" / exe)

    if args.skip_build:
        paths = {
            "pack": BUILD / "release" / exe,
            "skyward": BUILD / TARGET / "release" / "skyward",
            "system-init": BUILD / "system-init" / TARGET / "release" / "system-init",
        }
        for label, p in paths.items():
            if not p.exists():
                die(f"--skip-build 需要已有的 {label}，但没有 {p}")
        say("1-3/8", "按 --skip-build 跳过编译")
    else:
        paths = build(env)

    stock = take_stock(env, config)
    base = augment(py, stock)
    image = pack(env, paths, base, config, out_dir, full)
    check(py, image)
    final, digest = finalize(image, args.zip)

    print(f"\n✓ 出去了：{final}")
    print(f"  {final.stat().st_size} 字节，sha256 {digest}")
    print(f"  刷这个包和刷 CI 出来的一样：条目数、关键文件、AVB 都过同一道检查。")
    if args.deliver:
        tag = args.deliver_tag or (args.name.split("-")[0] if args.name else f"v{version}")
        deliver(final, tag, stock_version)


if __name__ == "__main__":
    main()
