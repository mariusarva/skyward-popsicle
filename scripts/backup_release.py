#!/usr/bin/env python3
"""把成品镜像传到 GitHub Releases，做异地备份。

镜像每个 100 MiB，进不了 git，所以放在 Release 的资产里。脚本是幂等的：
release 已存在就复用，同名资产已存在就跳过，中断后重跑不会重复上传。

    python scripts/backup_release.py --tag backup-2026-11-14 --dir D:/.../output
    python scripts/backup_release.py --tag backup-2026-11-14 --dir D:/.../output --only "v1*"

凭据：环境变量 GITHUB_TOKEN / GH_TOKEN，或 ~/.config/skyward/token，
或原机器路径 D:\\WorkBuddyAI\\github\\.token（跑 scripts/gh_device_flow.py 生成）。
代理：本机 127.0.0.1:7890 通才走，不通直连；也可由 http_proxy 环境变量覆盖。
"""

from __future__ import annotations

import argparse
import json
import os
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

REPO = "Ff428/skyward-popsicle"
API = "https://api.github.com"
PROXY_HOST, PROXY_PORT = "127.0.0.1", 7890


def _token_paths() -> list[Path]:
    """令牌可能在哪。换机器后落在 ~/.config，原机器在 D 盘。"""
    out = []
    env = os.environ.get("SKYWARD_TOKEN_FILE", "").strip()
    if env:
        out.append(Path(env))
    out += [Path.home() / ".config" / "skyward" / "token",
            Path("D:/WorkBuddyAI/github/.token")]
    return out


def token() -> str:
    t = (os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or "").strip()
    if t:
        return t
    for p in _token_paths():
        if p.is_file():
            t = p.read_text(encoding="utf-8").strip()
            if t:
                return t
    sys.exit("没有令牌：设 GITHUB_TOKEN，或跑 scripts/gh_device_flow.py 生成令牌文件")


_PROXY_URL: str | None = None


def proxy_url() -> str | None:
    """本机 7890 通才走代理，否则直连。不设环境变量时也是对的。"""
    global _PROXY_URL
    if _PROXY_URL is None:
        try:
            with socket.create_connection((PROXY_HOST, PROXY_PORT), timeout=0.6):
                _PROXY_URL = f"http://{PROXY_HOST}:{PROXY_PORT}"
        except OSError:
            _PROXY_URL = ""
    return _PROXY_URL or None


def _opener() -> urllib.request.OpenerDirector:
    p = proxy_url()
    handlers = [urllib.request.ProxyHandler({"http": p, "https": p})] if p else []
    return urllib.request.build_opener(*handlers)


def api(method: str, path: str, body: dict | None = None) -> dict:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(API + path, data=data, method=method)
    req.add_header("Authorization", f"Bearer {token()}")
    req.add_header("Accept", "application/vnd.github+json")
    if data:
        req.add_header("Content-Type", "application/json")
    try:
        with _opener().open(req, timeout=120) as r:
            return json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:400]
        sys.exit(f"{method} {path} -> HTTP {e.code}\n{detail}")


def ensure_release(tag: str, name: str, body: str) -> dict:
    try:
        rel = api("GET", f"/repos/{REPO}/releases/tags/{tag}")
        print(f"复用已有 release：{rel['tag_name']}（{len(rel.get('assets', []))} 个资产）")
        return rel
    except SystemExit:
        pass
    rel = api("POST", f"/repos/{REPO}/releases",
              {"tag_name": tag, "name": name, "body": body, "draft": False, "prerelease": False})
    print(f"已创建 release：{rel['tag_name']}")
    return rel


def upload(rel: dict, path: Path) -> None:
    """用 curl 传：它会正确处理 307 跳转并带着认证头。"""
    # upload_url 形如 https://uploads.github.com/repos/.../assets{?name,label}
    base = rel["upload_url"].split("{")[0]
    url = f"{base}?name={urllib.parse.quote(path.name)}"
    cmd = [
        "curl", "-sS", "--fail-with-body", "-X", "POST",
        "-H", f"Authorization: Bearer {token()}",
        "-H", "Content-Type: application/octet-stream",
        "-H", "Accept: application/vnd.github+json",
        "--data-binary", f"@{path}",
        url,
    ]
    t0 = time.time()
    # curl 认 http_proxy/https_proxy；探测到代理就补上，没探测到就用调用者的环境。
    env = os.environ.copy()
    p = proxy_url()
    if p:
        env["http_proxy"] = env["https_proxy"] = p
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    dt = time.time() - t0
    if r.returncode != 0:
        print(f"  ✗ {path.name}  {r.stdout.strip()[:300]}{r.stderr.strip()[:200]}")
        return
    try:
        d = json.loads(r.stdout)
        size = d.get("size", path.stat().st_size)
    except json.JSONDecodeError:
        size = path.stat().st_size
    mb = size / 1024 / 1024
    print(f"  ✓ {path.name}  {mb:.0f} MB  {dt:.0f}s  {mb/dt:.1f} MB/s")


def main() -> int:
    ap = argparse.ArgumentParser(description="把镜像传到 GitHub Releases 做异地备份")
    ap.add_argument("--tag", required=True)
    ap.add_argument("--name", default=None)
    ap.add_argument("--body", default="成品镜像异地备份。各版本说明见 docs/CHANGELOG.md，"
                                     "哈希见 backup/MANIFEST.md。")
    ap.add_argument("--dir", type=Path,
                    default=Path(os.environ.get("SKYWARD_OUTPUT_DIR", "").strip()
                                 or r"D:\WorkBuddyAI\2026-11-13-20-45-53\output"),
                    help="镜像目录；换机器后用这个指定，或设 SKYWARD_OUTPUT_DIR")
    ap.add_argument("--only", default="*.img", help="文件名通配，默认 *.img")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    files = sorted(p for p in args.dir.glob(args.only) if p.is_file())
    if not files:
        sys.exit(f"{args.dir} 里没有匹配 {args.only} 的文件")
    total = sum(p.stat().st_size for p in files) / 1024 / 1024
    print(f"共 {len(files)} 个文件 / {total:.0f} MB\n")

    if args.dry_run:
        for p in files:
            print(f"  {p.name}  {p.stat().st_size / 1024 / 1024:.0f} MB")
        return 0

    rel = ensure_release(args.tag, args.name or args.tag, args.body)
    have = {a["name"] for a in rel.get("assets", [])}

    for i, p in enumerate(files, 1):
        if p.name in have:
            print(f"  - {p.name}  已存在，跳过")
            continue
        print(f"[{i}/{len(files)}] {p.name}", flush=True)
        upload(rel, p)

    final = api("GET", f"/repos/{REPO}/releases/tags/{args.tag}")
    print(f"\n完成：{len(final.get('assets', []))} 个资产")
    for a in final.get("assets", []):
        print(f"  {a['name']:58s} {a['size'] / 1024 / 1024:>5.0f} MB  {a['state']}")
    print(f"\n下载页：{final['html_url']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
