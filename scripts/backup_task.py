#!/usr/bin/env python3
"""把「无法从上游重建」的东西同步进仓库，让整个任务可以随仓库一起搬走。

备份的不是源码（源码本来就在 git 里），而是这三样：

  1. 项目记忆   —— 版本谱系、踩过的坑、判断规则，全在 WorkBuddy 的记忆目录里
  2. 本机脚本   —— 授权、CI 查询、AVB 校验、刷机这些散在工作目录里的小工具
  3. 交付物清单 —— 每个历史镜像的 sha256，镜像本体不在仓库里
     （Releases 上唯一那份已于 2026-11-21 删除）

用法：

    python scripts/backup_task.py                # 只同步到工作区，先看 diff
    python scripts/backup_task.py --push         # 同步 + 提交 + 推送
    python scripts/backup_task.py --images-dir D:/某个目录/output

换机器后不用跑它，直接 `git clone` 就带着全部备份；它只是用来**让备份保持最新**。
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# WorkBuddy 的任务目录（记忆 + output 都在里面）。
# 换机器后这个目录不存在，所以：环境变量 SKYWARD_TASK_DIR 优先，
# 其次是本机原路径；都没有时就跳过对应部分，绝不覆盖已有备份。
_TASK_DIR = Path(os.environ.get("SKYWARD_TASK_DIR", "").strip()
                 or r"D:\WorkBuddyAI\2026-11-13-20-45-53")

# 项目记忆：WorkBuddy 的任务目录。换机器后路径会变，所以允许用参数覆盖。
MEMORY_DIR = Path(os.environ.get("SKYWARD_MEMORY_DIR", "").strip()
                  or _TASK_DIR / ".workbuddy" / "memory")

# 交付镜像目录。
IMAGES_DIR = Path(os.environ.get("SKYWARD_OUTPUT_DIR", "").strip()
                  or _TASK_DIR / "output")

# 本机脚本的来源目录（按顺序找，第一个存在的生效）。
SCRIPT_DIRS = [Path(os.environ.get("SKYWARD_SCRIPT_DIR", "").strip()
                    or r"D:\WorkBuddyAI\scripts"),
               _TASK_DIR / "scripts"]

# 只备确实会用到的：能从仓库里已有的脚本重建的，不重复搬。
SCRIPTS: dict[str, str] = {
    # GitHub 凭据与查询
    "gh_device_flow.py": "设备码授权，换机器后拿推送令牌用",
    "ci_watch.py": "查 CI 运行状态",
    # 镜像校验
    "avb_verify.py": "校验 AVB footer / vbmeta 摘要",
    "boot_check.py": "检查 boot 镜像结构",
    "boot_analyze.py": "拆解脚本分析分区布局",
    "hash_verify.py": "按清单校验镜像哈希",
    "inspect_rec.py": "查看 recovery 镜像信息",
    # 上机
    "flash_rec.py": "刷 recovery 分区",
    "extract_parts.py": "从线刷包/OTA 包里抽分区",
    # 早期手工出包（v6–v12 用的，CI 化之前的实现，留作参考）
    "unpack_ramdisk.py": "解 ramdisk（lz4 legacy + cpio newc）",
    "parse_boot.py": "解析 boot image v4 头部",
    "repack_image_v12.py": "v12 的手工重打包流程",
    # 通用小工具
    "runjava.py": "跑 Java 工具并捕获输出到日志",
    "pyerr.py": "跑 Python 脚本并捕获 traceback",
}

BACKUP = REPO / "backup"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def copy_memory(memory_dir: Path) -> int:
    """记忆目录整个搬过来，包括历史日志——判断依据往往在旧日志里。"""
    dest = BACKUP / "memory"
    dest.mkdir(parents=True, exist_ok=True)
    n = 0
    if not memory_dir.is_dir():
        print(f"  ! 记忆目录不存在：{memory_dir}")
        return 0
    for src in sorted(memory_dir.glob("*.md")):
        shutil.copy2(src, dest / src.name)
        n += 1
    print(f"  记忆 {n} 个文件 ← {memory_dir}")
    return n


def copy_scripts() -> int:
    dest = BACKUP / "scripts"
    dest.mkdir(parents=True, exist_ok=True)
    n = 0
    missing = []
    for name, why in SCRIPTS.items():
        src = next((d / name for d in SCRIPT_DIRS if (d / name).is_file()), None)
        if src is None:
            missing.append(name)
            continue
        shutil.copy2(src, dest / name)
        n += 1
    print(f"  脚本 {n}/{len(SCRIPTS)} 个")
    if missing:
        print(f"  ! 没找到（跳过）：{', '.join(missing)}")
    # 说明每个脚本干什么，免得半年后不知道某个文件能不能删。
    lines = ["# 脚本快照\n", "从本机工作目录复制来的，原件的权威版本在 WorkBuddy 任务目录里。\n",
             "| 文件 | 用途 |", "|---|---|"]
    lines += [f"| `{n}` | {w} |" for n, w in SCRIPTS.items() if (dest / n).is_file()]
    (dest / "README.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return n


def write_manifest(images_dir: Path) -> int:
    """生成 MANIFEST.md。

    交付目录不存在时（换机器后）**不覆盖**已有清单——否则 13 个镜像的哈希
    会被一行「未找到 .img」抹掉，而镜像本体在 Releases 上，哈希是唯一的身份凭据。
    """
    if not images_dir.is_dir():
        existing = BACKUP / "MANIFEST.md"
        if existing.is_file():
            print(f"  ! 交付目录不存在：{images_dir}")
            print("    保留现有 MANIFEST.md 不动（镜像哈希不会丢）。")
            print("    要用新的交付目录：--images-dir <路径> 或设 SKYWARD_OUTPUT_DIR")
            return -1

    rows = []
    if images_dir.is_dir():
        for img in sorted(images_dir.glob("*.img")):
            rows.append((img.name, img.stat().st_size, sha256(img)))
            print(f"  {img.name}  {img.stat().st_size:,} 字节  {rows[-1][2][:16]}…")

    out = [
        "# 交付物清单",
        "",
        "由 `scripts/backup_task.py` 生成，勿手改。",
        f"生成时间：{datetime.now().strftime('%Y-%m-%d %H:%M')}",
        "",
        "## 历史镜像",
        "",
        "镜像本体不进 git（每个 100 MiB），也不在 Releases 上——2026-11-21 删掉了",
        "唯一的 `backup-2026-11-14`。要就由 CI 用对应提交重新构建。这里是核对身份的哈希。",
        "",
    ]
    if rows:
        out += ["| 文件 | 大小 | sha256 |", "|---|---|---|"]
        out += [f"| `{n}` | {s:,} | `{h}` |" for n, s, h in rows]
    else:
        out.append(f"（未在 `{images_dir}` 找到 .img）")
    out += [
        "",
        "## 恢复包",
        "",
        "`skyward-task-backup-*.zip` 是可以整包拷走的版本：完整 git 历史（bundle）",
        "+ 项目记忆 + 文档 + 脚本，解压后按里面的 `00-先读这个.txt` 操作。",
        "它默认放在 `D:\\WorkBuddyAI\\backup\\task\\`，**不与 recovery 包混在一起**",
        "（`make_backup_zip.py --out-dir` 可以改）。",
        "",
        "**这份清单不记录那个 zip 的名字、大小和哈希**——清单本身就在 zip 里，",
        "记什么都会过时（重打一次就变）。要校验就比对备份 zip 同目录的 `.sha256` 文件，",
        "它由 `make_backup_zip.py` 在打包后生成，是唯一权威的摘要。",
        "",
    ]
    out += [
        "",
        "## 成品镜像在哪",
        "",
        "已经没有了：2026-11-21 删掉了唯一的 `backup-2026-11-14`（15 个资产 / 1.3 GB）。",
        "旧版只能按上表的提交让 CI 重出，或回出包那台机器的 `output/`。",
        "",
    ]
    out += [
        "## 版本谱系",
        "",
        "每一版改了什么、基于哪个上游，见 [docs/CHANGELOG.md](../docs/CHANGELOG.md)。",
        "",
        "## 备份来源",
        "",
        "| 内容 | 来源 |",
        "|---|---|",
        f"| 项目记忆 | `{MEMORY_DIR}` |",
        "| 本机脚本 | " + "、".join(f"`{d}`" for d in SCRIPT_DIRS) + " |",
        f"| 交付镜像 | `{images_dir}` |",
        "",
    ]
    (BACKUP / "MANIFEST.md").write_text("\n".join(out), encoding="utf-8")
    print(f"  清单 {len(rows)} 个镜像 → backup/MANIFEST.md")
    return len(rows)


def push(message: str) -> int:
    """交给 scripts/push.py：它处理 PortableGit 路径、代理和令牌。"""
    cmd = [sys.executable, str(REPO / "scripts" / "push.py"), "-m", message]
    print("  " + " ".join(cmd))
    return subprocess.call(cmd, cwd=REPO)


def main() -> int:
    ap = argparse.ArgumentParser(description="备份这个任务：记忆 + 脚本 + 交付物清单")
    ap.add_argument("--memory-dir", type=Path, default=MEMORY_DIR)
    ap.add_argument("--images-dir", type=Path, default=IMAGES_DIR)
    ap.add_argument("--push", action="store_true", help="同步后提交并推送")
    ap.add_argument("-m", "--message", default=None, help="提交信息")
    args = ap.parse_args()

    print(f"仓库：{REPO}\n")
    copy_memory(args.memory_dir)
    copy_scripts()
    write_manifest(args.images_dir)

    if args.push:
        msg = args.message or f"backup: 同步任务备份 {datetime.now():%Y-%m-%d %H:%M}"
        print("\n推送：")
        return push(msg)

    print("\n完成（未推送）。看改动：git status --short backup/")
    print("确认无误后：python scripts/backup_task.py --push")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
