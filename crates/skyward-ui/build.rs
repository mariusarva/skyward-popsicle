//! 把 openh264 的 shim 编出来，跟仓库里那份 aarch64 的静态库合成一个包。
//!
//! openh264 自己没法在构建时现编（要 NDK 的 clang、八十多个源文件、几分钟），
//! 所以 `native/aarch64-linux-android/libopenh264.a` 是提交进仓库的二进制，
//! 来源、版本和重编命令写在 `native/README.md`。这个脚本只做三件事：
//!
//! 1. 用 NDK 的 clang++ 编 `h264_shim.cpp`（C 包装，见那边的注释）；
//! 2. 把两个静态库解包再打成一个 —— `ar` 不能把一个 `.a` 塞进另一个 `.a`，
//!    而链接时库的顺序又不好控（shim 引 openh264 的符号），合成一个最省心；
//! 3. 告诉 rustc 去链这一个。
//!
//! 主机上（`cargo test`、`skyward-pack` 的依赖检查）什么都不做：那些目标不会
//! 引用 `sky_h264_*`，也就不会去找这个库。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run(cmd: &mut Command) {
    let out = cmd.output().expect("spawn");
    if !out.status.success() {
        panic!(
            "{:?} 失败：\n{}\n{}",
            cmd,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn env_any(names: &[&str], fallback: &str) -> String {
    for name in names {
        if let Ok(v) = env::var(name)
            && !v.is_empty()
        {
            return v;
        }
    }
    fallback.to_string()
}

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.starts_with("aarch64-linux-android") {
        return;
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let native = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("native");
    let shim = native.join("h264_shim.cpp");
    println!("cargo:rerun-if-changed={}", shim.display());
    println!("cargo:rerun-if-changed={}", native.join("aarch64-linux-android/libopenh264.a").display());

    let cxx = env_any(&["CXX_aarch64_linux_android", "CXX_aarch64-linux-android", "CXX"], "clang++");
    let ar = env_any(&["AR_aarch64_linux_android", "AR_aarch64-linux-android", "AR"], "llvm-ar");

    let obj = out.join("h264_shim.o");
    let mut cc = Command::new(&cxx);
    cc.arg(format!("--target={target}"))
        .arg("-std=c++11")
        .arg("-O2")
        .arg("-fPIC")
        .arg("-fno-rtti")
        .arg("-fno-exceptions")
        .arg(format!("-I{}", native.join("include").display()))
        .arg("-c")
        .arg(&shim)
        .arg("-o")
        .arg(&obj);
    run(&mut cc);

    let unpack = out.join("openh264");
    let _ = fs::remove_dir_all(&unpack);
    fs::create_dir_all(&unpack).expect("mkdir openh264");
    let mut x = Command::new(&ar);
    x.current_dir(&unpack)
        .arg("x")
        .arg(native.join("aarch64-linux-android/libopenh264.a"));
    run(&mut x);

    let mut objs: Vec<PathBuf> = fs::read_dir(&unpack)
        .expect("read openh264")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "o").unwrap_or(false))
        .collect();
    objs.sort();
    let count = objs.len();
    objs.push(obj);

    let merged = out.join("libskyward_h264.a");
    let _ = fs::remove_file(&merged);
    let mut pack = Command::new(&ar);
    pack.current_dir(&out).arg("rcs").arg(&merged).args(&objs);
    run(&mut pack);

    println!("cargo:rustc-link-search=native={}", out.display());
    // 整个归档都要进去：openh264 内部靠函数指针表互相引用（`WelsInitMeFunc`
    // 之类），纯按需拉取时 ld 只解出引用方那一个成员，被指向的 `.S.o` 就漏了。
    // 链接时 `--gc-sections` 还会把真正用不到的丢掉。
    println!("cargo:rustc-link-lib=static:+whole-archive=skyward_h264");
    println!("cargo:warning=openh264: {count} objects + the shim → libskyward_h264.a");
}
