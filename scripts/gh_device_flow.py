#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""GitHub OAuth 设备码授权：不用密码，也不用在网页上创建 PAT。

流程：本脚本向 GitHub 申请一个 8 位设备码 → 你在浏览器里输入并授权
     → 脚本轮询拿到 token → 写入本机令牌文件（push.py 直接读它）。

用法:
    python scripts/gh_device_flow.py                     # 默认位置
    python scripts/gh_device_flow.py --token-file ./t    # 指定落盘位置
    python scripts/gh_device_flow.py --no-proxy          # 不走代理

令牌默认写到（按顺序取第一个目录存在的）：
    D:\\WorkBuddyAI\\github\\.token      ← 原机器的位置
    ~/.config/skyward/token              ← 换机器后的默认位置
也可以直接给环境变量 SKYWARD_TOKEN_FILE / GITHUB_TOKEN_FILE。

不写文件、只要令牌：加 --print（令牌会出现在终端里，注意终端历史）。
"""

from __future__ import annotations

import argparse
import json
import os
import socket
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

CLIENT_ID = "178c6fc778ccc68e1d6a"  # GitHub CLI 的公开 OAuth client id
SCOPE = "repo workflow"            # workflow：推 .github/workflows/*.yml 必需
PROXY_HOST, PROXY_PORT = "127.0.0.1", 7890


def default_token_file() -> Path:
    for env in ("SKYWARD_TOKEN_FILE", "GITHUB_TOKEN_FILE"):
        if os.environ.get(env):
            return Path(os.environ[env])
    legacy = Path("D:/WorkBuddyAI/github")
    if legacy.is_dir():
        return legacy / ".token"
    return Path.home() / ".config" / "skyward" / "token"


def proxy_alive() -> bool:
    try:
        with socket.create_connection((PROXY_HOST, PROXY_PORT), timeout=0.6):
            return True
    except OSError:
        return False


def make_post(use_proxy: bool):
    handlers = []
    if use_proxy:
        url = f"http://{PROXY_HOST}:{PROXY_PORT}"
        handlers.append(urllib.request.ProxyHandler({"http": url, "https": url}))
    else:
        handlers.append(urllib.request.ProxyHandler({}))  # 显式禁用，避开环境变量
    opener = urllib.request.build_opener(*handlers)

    def post(url: str, fields: dict) -> dict:
        data = urllib.parse.urlencode(fields).encode()
        req = urllib.request.Request(url, data=data, method="POST")
        req.add_header("Accept", "application/json")
        req.add_header("User-Agent", "skyward-popsicle-devtool")
        with opener.open(req, timeout=30) as r:
            return json.loads(r.read())

    return post


def log(*a) -> None:
    print(*a, flush=True)


def main() -> int:
    ap = argparse.ArgumentParser(description="GitHub 设备码授权")
    ap.add_argument("--token-file", type=Path, default=None)
    ap.add_argument("--no-proxy", action="store_true", help="强制直连")
    ap.add_argument("--proxy", action="store_true", help="强制走 127.0.0.1:7890")
    ap.add_argument("--print", dest="print_only", action="store_true",
                    help="只打印令牌，不落盘")
    args = ap.parse_args()

    use_proxy = False
    if args.proxy:
        use_proxy = True
    elif not args.no_proxy and proxy_alive():
        use_proxy = True
    log(f"网络：{'经代理 127.0.0.1:7890' if use_proxy else '直连'}")

    post = make_post(use_proxy)
    try:
        start = post("https://github.com/login/device/code",
                     {"client_id": CLIENT_ID, "scope": SCOPE})
    except Exception as e:
        log("无法连到 GitHub：", type(e).__name__, str(e)[:160])
        log("换网络，或加 --proxy / --no-proxy 再试。")
        return 4

    code = start["user_code"]
    uri = start.get("verification_uri", "https://github.com/login/device")
    interval = max(int(start.get("interval", 5)), 5)
    deadline = time.time() + int(start.get("expires_in", 900))

    log("=" * 46)
    log("  在浏览器打开：" + uri)
    log("  输入代码：" + code)
    log("=" * 46)

    while time.time() < deadline:
        time.sleep(interval)
        try:
            res = post("https://github.com/login/oauth/access_token", {
                "client_id": CLIENT_ID,
                "device_code": start["device_code"],
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            })
        except Exception as e:
            log("轮询出错（继续等）：", type(e).__name__, str(e)[:120])
            continue
        err = res.get("error")
        if err == "authorization_pending":
            log("等待授权…")
            continue
        if err == "slow_down":
            interval += 5
            log("放慢轮询，间隔", interval)
            continue
        if err:
            log("AUTH_FAILED:", err, res.get("error_description", ""))
            return 2
        token = (res.get("access_token") or "").strip()
        if not token:
            continue
        if args.print_only:
            log("TOKEN:", token)
            return 0
        target = args.token_file or default_token_file()
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(token, encoding="ascii")
        try:
            target.chmod(0o600)
        except OSError:
            pass
        log("TOKEN_SAVED:", target)
        log("TOKEN_PREFIX:", token[:7] + "…" + token[-4:])
        log("SCOPE:", res.get("scope"))
        log("")
        log("下一步：python scripts/push.py --dry-run 验证能用。")
        return 0
    log("AUTH_FAILED: 设备码超时，重跑一次拿新码。")
    return 3


if __name__ == "__main__":
    sys.exit(main())
