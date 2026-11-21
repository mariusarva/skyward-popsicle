#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""检查上游是否发布了比基线更新的版本；有就在本仓库开一个 issue。

只读上游公开 API，不写任何代码。判断依据是本仓库的
.github/upstream-baseline.json，每次升级后手工更新那里的 tag。

用法:
    python .github/scripts/watch_upstream.py --dry-run
    GH_TOKEN=xxx GITHUB_REPOSITORY=Ff428/skyward-popsicle python .github/scripts/watch_upstream.py
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
BASELINE_FILE = REPO_ROOT / ".github" / "upstream-baseline.json"

# 只盯布丁。主仓库（kirklin/skyward）不再跟：pudding 的补丁就是以它的 v0.3.1
# 为底做的整树 diff，跟一个就够。
UPSTREAMS = {
    "pudding": "mariusarva/skyward-pudding",
}
FIX_KINDS = {"fix", "perf"}


def api(path: str, token: str | None) -> dict | list:
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "watch-upstream"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(f"https://api.github.com{path}", headers=headers)
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def version(tag: str) -> tuple[int, int, int] | None:
    m = re.match(r"v?(\d+)\.(\d+)\.(\d+)", tag)
    return tuple(int(x) for x in m.groups()) if m else None


def latest_tag(repo: str, token: str | None) -> tuple[str | None, list[str]]:
    tags = [t["name"] for t in api(f"/repos/{repo}/tags?per_page=40", token)]
    versioned = [t for t in tags if version(t)]
    if not versioned:
        return None, tags
    return max(versioned, key=version), versioned


def commits_between(repo: str, base: str, head: str, token: str | None) -> list[dict]:
    """两个 tag 之间的提交，取不到就返回空（不影响提醒本身）。"""
    try:
        data = api(f"/repos/{repo}/compare/{base}...{head}", token)
    except urllib.error.HTTPError:
        return []
    rows = []
    for c in data.get("commits", []):
        msg = c["commit"]["message"].splitlines()[0]
        rows.append({"sha": c["sha"][:7], "subject": msg, "fix": kind(msg) in FIX_KINDS})
    return rows


def kind(subject: str) -> str:
    m = re.match(r"^([a-z]+)(\([^)]*\))?!?:", subject)
    return m.group(1) if m else "other"


def classify(rows: list[dict]) -> str:
    fixes = [r for r in rows if r["fix"]]
    churn = [r for r in rows if not r["fix"]]
    lines = [f"共 {len(rows)} 个提交：修复 {len(fixes)} 个，其余 {len(churn)} 个。", ""]
    for r in rows:
        mark = "修复" if r["fix"] else "    "
        lines.append(f"- [{mark}] `{r['sha']}` {r['subject']}")
    return "\n".join(lines)


def open_issues(repo: str, token: str) -> list[dict]:
    try:
        return api(f"/repos/{repo}/issues?state=open&per_page=100", token)
    except urllib.error.HTTPError:
        return []


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true", help="只打印，不建 issue")
    args = ap.parse_args()

    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    self_repo = os.environ.get("GITHUB_REPOSITORY", "Ff428/skyward-popsicle")
    baseline = json.loads(BASELINE_FILE.read_text(encoding="utf-8"))

    news = []
    for key, slug in UPSTREAMS.items():
        base_tag = baseline.get(key, {}).get("tag")
        try:
            tag, _ = latest_tag(slug, token)
        except (urllib.error.URLError, TimeoutError) as e:
            print(f"{slug}: 查询失败，跳过（{e}）", file=sys.stderr)
            continue
        if not tag or not base_tag:
            continue
        if version(tag) and version(base_tag) and version(tag) > version(base_tag):
            rows = commits_between(slug, base_tag, tag, token)
            news.append({"key": key, "slug": slug, "base": base_tag, "tag": tag, "rows": rows})
            print(f"{slug}: {base_tag} -> {tag}（有新版本）")
        else:
            print(f"{slug}: {base_tag} = {tag}，无更新")

    if not news:
        print("上游没有新版本。")
        return 0

    parts = [f"{n['key']} {n['tag']}" for n in news]
    title = f"上游更新：{' + '.join(parts)}"

    body = ["上游发布了比本仓库基线更新的版本。", ""]
    body.append("| 项目 | 基线 | 最新 |")
    body.append("|---|---|---|")
    for n in news:
        body.append(f"| {n['slug']} | {n['base']} | {n['tag']} |")
    body.append("")
    for n in news:
        if n["rows"]:
            body.append(f"### {n['slug']} {n['base']} → {n['tag']}")
            body.append("")
            body.append(classify(n["rows"]))
            body.append("")
    body.append("### 怎么处理")
    body.append("")
    body.append("在开发机上：")
    body.append("")
    body.append("```")
    body.append("python scripts/sync_upstream.py --fetch")
    body.append("```")
    body.append("")
    body.append("脚本会分类提交、预演 cherry-pick 冲突并给出建议路线；")
    body.append("判断逻辑见 [docs/SYNC-UPSTREAM.md](../blob/main/docs/SYNC-UPSTREAM.md)。")
    body.append("")
    body.append("升级完成后，记得更新 `.github/upstream-baseline.json` 里的 tag 并关掉本 issue。")
    text = "\n".join(body)

    if args.dry_run:
        print(f"\n[dry-run] 会建 issue：{title}\n")
        print(text)
        return 0

    if not token:
        print("没有 GH_TOKEN，无法建 issue。", file=sys.stderr)
        return 1

    for issue in open_issues(self_repo, token):
        if issue.get("title", "").startswith("上游更新：") and any(n["tag"] in issue["title"] for n in news):
            print(f"已存在同类 issue #{issue['number']}，不重复创建。")
            return 0

    data = json.dumps({"title": title, "body": text}).encode()
    req = urllib.request.Request(
        f"https://api.github.com/repos/{self_repo}/issues",
        data=data, method="POST",
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "User-Agent": "watch-upstream",
        },
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        created = json.load(r)
    print(f"已建 issue #{created['number']}：{created['html_url']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
