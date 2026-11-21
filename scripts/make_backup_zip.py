#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""打一个可以整包拷走的备份：完整 git 历史 + 记忆 + 文档 + 脚本。

换机器、重装、或者这个 WorkBuddy 任务被删了，把这个 zip 交给 AI 就能接着干。
zip 里最关键的是 `skyward-popsicle.bundle`——带着全部提交和分支，不依赖网络
也能把仓库还原出来。

用法:
    python scripts/make_backup_zip.py                  # 版本号从 docs/CHANGELOG.md 读
    python scripts/make_backup_zip.py --tag v15
    python scripts/make_backup_zip.py --out-dir D:/tmp

产物：`<out-dir>/skyward-task-backup-<tag>-<日期>.zip` 和同名 `.sha256`。

注意：bundle **只打分支，不打 tag**。本仓库是浅克隆，`v0.3.x` tag 指向被截断的
真实上游提交、不在本地历史里，打 `--all` 会得到克隆时报
`remote did not send all necessary objects` 的残缺包。上游 tag 换机后用
`git fetch upstream --tags --depth=200` 单独补。
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import time
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
GIT_CANDIDATES = [
    Path("D:/软件/应用/PortableGit/mingw64/bin/git.exe"),
    Path("D:/软件/应用/PortableGit/cmd/git.exe"),
    Path("/usr/bin/git"),
    Path("/usr/local/bin/git"),
]
# 备份 zip 的落脚处：跟交付目录分开，免得 recovery 包旁边堆一堆备份。
# 换机器后原路径不存在，会自动落到仓库的 dist/（见 main）。
DEFAULT_OUT = Path(os.environ.get("SKYWARD_BACKUP_DIR", "").strip()
                   or r"D:\WorkBuddyAI\backup\task")

HOWTO = """这是「skyward-popsicle」这个任务的完整备份（{tag}，{date}）。
仓库状态：{commits} 个提交 · {nrefs} 个分支 · HEAD {head}

## 四种情况都能用

| 情况 | 怎么做 |
|---|---|
| 电脑重装 / 换电脑 / 这个任务被删了 | 按下面第 2 步还原仓库，然后读 RESTORE.md |
| 手机系统要升级（比如 4.0.7.0） | 还原后跑 `python scripts/bump_stock.py --version OS4.0.7.0.XPBCNXM` |
| 上游发新版（比如 0.3.2） | 还原后跑 `python scripts/sync_upstream.py --fetch` |
| 只想拿某个旧版本的镜像 | 本机 `D:/WorkBuddyAI/backup/recovery/`（v14–v17 的 zip）或原机器 `output/`；Releases 上那份「任务备份」已于 2026-11-21 删除 |

详细步骤、环境清单、踩过的坑全在 RESTORE.md 里——**让 AI 先读它**。

## 怎么还原

1. 解压。
2. 用 bundle 还原仓库（不依赖网络）：
       git clone skyward-popsicle.bundle skyward-popsicle
       cd skyward-popsicle
       git remote set-url origin https://github.com/Ff428/skyward-popsicle.git
   能上网的话，直接从 GitHub clone 也一样，内容相同。
3. 补上游 tag（bundle 里没有，见下面「缺什么」）：
       git remote add upstream https://github.com/kirklin/skyward.git
       git fetch upstream --tags --depth=200
4. 重新授权推送：`python scripts/gh_device_flow.py`（浏览器输 8 位码，2 分钟）。

## 缺什么（都是故意的）

- **上游 tag**（v0.1.0 … v0.3.1）：指向真实上游提交，不在本仓库历史里，上面的命令补。
- **推送令牌**：进版本库就等于泄露。重新授权，scope 要 `repo,workflow`。
- **成品镜像**：每个 100 MiB，**不在仓库里，也不在 Releases 上**（2026-11-21 删掉了
  `backup-2026-11-14`）。本机 `D:/WorkBuddyAI/backup/recovery/` 留了 v14–v17 的 zip，
  其余按 `backup/MANIFEST.md` 里的提交让 CI 重出。
- **编译工具链**：Rust 1.100.0 / NDK r29 / mingw，清单在 RESTORE.md 第 1 节。
  只出包的话不需要——CI 上编译。

## 包里有什么

- RESTORE.md              恢复入口（最重要，先读这个）
- AGENTS.md               给 AI 的硬规则（别 merge 上游、出包只走 CI）
- docs/                   升级、出包、定制、变更记录
- backup/memory/          项目记忆：版本谱系、判断规则、踩过的坑
- backup/scripts/         本机脚本快照
- backup/MANIFEST.md      历史镜像 sha256
- scripts/                出包 / 同步 / 推送 / 备份 / 授权工具
- skyward-popsicle.bundle 完整 git 历史（{commits} 提交 / {nrefs} 分支）

> 包内 MANIFEST.md 里「恢复包」那一行的 sha256 是**上一版**的（自引用没法自洽）。
> 本包的真实校验值看解压目录同级的 `.sha256` 文件，或生成时终端的输出。

## 重新打包

仓库内容变了想刷新这个包，在仓库根目录跑：

    python scripts/make_backup_zip.py

它会重打 bundle、重打 zip、写校验文件，不需要手工拼文件清单。
"""


def find_git() -> str:
    for p in GIT_CANDIDATES:
        if p.exists():
            return str(p)
    return "git"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def detect_tag() -> str:
    cl = REPO / "docs" / "CHANGELOG.md"
    if cl.is_file():
        m = re.search(r"^\|\s*(v\d+)\s*\|.*当前版本", cl.read_text(encoding="utf-8"), re.M)
        if m:
            return m.group(1)
    return "v0"


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    ap = argparse.ArgumentParser(description="打包整包备份")
    ap.add_argument("--tag", default=None, help="版本标签，默认读 CHANGELOG 的「当前版本」")
    ap.add_argument("--out-dir", type=Path, default=None)
    ap.add_argument("--keep-bundle", action="store_true", help="把 bundle 留在 out-dir 里")
    args = ap.parse_args()

    git = find_git()

    def run(*a: str, check: bool = True) -> subprocess.CompletedProcess:
        r = subprocess.run([git, *a], cwd=REPO, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
        if check and r.returncode != 0:
            print((r.stdout + r.stderr).strip(), file=sys.stderr)
            raise SystemExit(f"git {' '.join(a)} 失败")
        return r

    tag = args.tag or detect_tag()
    out_dir = args.out_dir or (DEFAULT_OUT if DEFAULT_OUT.is_dir() else REPO / "dist")
    out_dir.mkdir(parents=True, exist_ok=True)
    date = time.strftime("%Y-%m-%d")
    head = run("rev-parse", "--short", "HEAD").stdout.strip()
    commits = run("rev-list", "--count", "HEAD").stdout.strip()
    branches = [b for b in run("for-each-ref", "--format=%(refname:short)",
                               "refs/heads").stdout.split() if b]

    # 1) bundle：HEAD 必须显式写进去，否则克隆默认落到别的分支
    tmp = REPO / ".git" / "backup-tmp"
    tmp.mkdir(exist_ok=True)
    bundle = tmp / "skyward-popsicle.bundle"
    refs = ["HEAD"] + branches
    print(f"打 bundle：{len(refs)} 个引用（HEAD + {len(branches)} 分支）", flush=True)
    run("bundle", "create", str(bundle), *refs)
    v = run("bundle", "verify", str(bundle), check=False)
    if v.returncode != 0:
        print((v.stdout + v.stderr).strip(), file=sys.stderr)
        return 1
    print(f"bundle {bundle.stat().st_size / 1024 / 1024:.1f} MB，校验通过")

    # 2) 文件清单
    files = [p for p in sorted((REPO / "docs").glob("*.md"))]
    for name in ("README.md", "RESTORE.md", "AGENTS.md", "UPGRADE-PROMPT.md"):
        p = REPO / name
        if p.is_file():
            files.append(p)
    files += [p for p in sorted((REPO / "backup").rglob("*")) if p.is_file()]
    files += [p for p in sorted((REPO / "scripts").iterdir())
              if p.is_file() and p.suffix in {".py", ".sh"}]

    out = out_dir / f"skyward-task-backup-{tag}-{date}.zip"
    howto = HOWTO.format(tag=tag, date=date, commits=commits,
                         nrefs=len(branches), head=head)
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        z.writestr("00-先读这个.txt", howto)
        for f in files:
            z.write(f, f.relative_to(REPO).as_posix())
        z.write(bundle, "skyward-popsicle.bundle")

    digest = sha256(out)
    (out_dir / f"{out.name}.sha256").write_text(f"{digest}  {out.name}\n", encoding="utf-8")
    if args.keep_bundle:
        shutil.copy2(bundle, out_dir / bundle.name)
    shutil.rmtree(tmp, ignore_errors=True)

    print("")
    print(f"{out}")
    print(f"  {out.stat().st_size / 1024 / 1024:.1f} MB · {len(files) + 2} 个条目")
    print(f"  sha256 {digest}")
    print(f"  校验文件 {out.name}.sha256")
    return 0


if __name__ == "__main__":
    sys.exit(main())
