#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""把当前分支推到 origin，认证用本地保存的令牌。

Windows 上 git push 需要两样容易被忽略的东西：带 git-remote-https 的
git.exe（PortableGit 的 mingw64/bin 里那个），以及 HTTPS 代理。这个脚本
都处理好了，省得每次手拼 base64。

用法:
    python scripts/push.py                       # 推送当前分支到 origin 同名分支
    python scripts/push.py -m "fix: ..."         # 先 git add -A + commit 再推
    python scripts/push.py --branch main         # 指定远程分支名
    python scripts/push.py --no-proxy            # 直连（本机不通，CI 环境用）

令牌查找顺序（先找到先用）：
    1. --token-file 指定的路径
    2. 环境变量 GITHUB_TOKEN / GH_TOKEN
    3. D:\\WorkBuddyAI\\github\\.token（原机器位置）
    4. ~/.config/skyward/token（换机器后的位置）
"""

from __future__ import annotations

import argparse
import base64
import os
import socket
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PROXY_HOST, PROXY_PORT = "127.0.0.1", 7890

GIT_CANDIDATES = [
    Path("D:/软件/应用/PortableGit/mingw64/bin/git.exe"),
    Path("D:/软件/应用/PortableGit/cmd/git.exe"),
    Path("/usr/bin/git"),
    Path("/usr/local/bin/git"),
]


def find_git() -> str:
    for p in GIT_CANDIDATES:
        if p.exists():
            return str(p)
    return "git"


def resolve_token(explicit: str | None) -> tuple[str, str] | None:
    """返回 (令牌, 来源说明)。找不到返回 None。"""
    if explicit:
        p = Path(explicit)
        if p.is_file():
            t = p.read_text(encoding="utf-8").strip()
            if t:
                return t, str(p)
        return None
    for env in ("GITHUB_TOKEN", "GH_TOKEN"):
        if os.environ.get(env, "").strip():
            return os.environ[env].strip(), f"环境变量 {env}"
    for p in (Path("D:/WorkBuddyAI/github/.token"),
              Path.home() / ".config" / "skyward" / "token"):
        if p.is_file():
            t = p.read_text(encoding="utf-8").strip()
            if t:
                return t, str(p)
    return None


def proxy_alive() -> bool:
    try:
        with socket.create_connection((PROXY_HOST, PROXY_PORT), timeout=0.6):
            return True
    except OSError:
        return False


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    ap = argparse.ArgumentParser(description="推送到 origin")
    ap.add_argument("-m", "--message", help="提交信息；给了就先 add -A + commit")
    ap.add_argument("--branch", help="远程分支名（默认当前分支）")
    ap.add_argument("--token-file", default=None)
    ap.add_argument("--user", default="Ff428", help="GitHub 用户名")
    ap.add_argument("--no-proxy", action="store_true", help="强制直连")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    got = resolve_token(args.token_file)
    if not got:
        print("找不到令牌。先跑：python scripts/gh_device_flow.py", file=sys.stderr)
        print("（或设环境变量 GITHUB_TOKEN，或用 --token-file 指定）", file=sys.stderr)
        return 1
    token, token_src = got

    git = find_git()
    env = os.environ.copy()
    env["PATH"] = str(Path(git).parent) + os.pathsep + env.get("PATH", "")
    use_proxy = False
    if not args.no_proxy and proxy_alive():
        use_proxy = True
        url = f"http://{PROXY_HOST}:{PROXY_PORT}"
        env["http_proxy"] = env["https_proxy"] = url

    def run(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
        r = subprocess.run(cmd, cwd=REPO, env=env, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
        if check and r.returncode != 0:
            print(r.stdout.strip(), file=sys.stderr)
            print(r.stderr.strip(), file=sys.stderr)
            raise SystemExit(r.returncode)
        return r

    if args.message:
        run([git, "add", "-A"])
        dirty = run([git, "status", "--porcelain"]).stdout.strip()
        if dirty:
            run([git, "-c", "user.name=F", "-c", "user.email=f@local",
                 "commit", "-q", "-m", args.message])
            print(f"已提交：{args.message.splitlines()[0]}")
        else:
            print("工作区干净，跳过提交。")

    branch = args.branch or run([git, "rev-parse", "--abbrev-ref", "HEAD"]).stdout.strip()
    if branch == "HEAD":
        print("当前处于分离头指针状态，用 --branch 指定目标分支。", file=sys.stderr)
        return 1

    auth = base64.b64encode(f"{args.user}:{token}".encode()).decode()
    cmd = [git, "-c", f"http.extraheader=Authorization: Basic {auth}",
           "push", "origin", f"HEAD:refs/heads/{branch}"]
    if args.dry_run:
        print(f"将推送 HEAD -> origin/{branch}")
        print(f"  认证：{token_src}（{len(token)} 字符）")
        print(f"  网络：{'代理 127.0.0.1:7890' if use_proxy else '直连'}")
        return 0

    r = subprocess.run(cmd, cwd=REPO, env=env, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    out = (r.stdout + r.stderr).strip()
    for line in out.splitlines():
        if auth not in line and token not in line:
            print(line)
    return r.returncode


if __name__ == "__main__":
    sys.exit(main())
