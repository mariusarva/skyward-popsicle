#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""侦察上游更新：分类提交、预演 cherry-pick 冲突、给出建议路线。

用法:
    python scripts/sync_upstream.py                # 用本地已有的 upstream tag 出报告
    python scripts/sync_upstream.py --fetch        # 先联网抓上游 tag 再出报告
    python scripts/sync_upstream.py --base v0.3.0  # 指定基线（默认读 .github/upstream-baseline.json）
    python scripts/sync_upstream.py --apply        # 预演全部干净时，真正 cherry-pick 修复类提交
    python scripts/sync_upstream.py --out report.md

设计原则：只读。除 --apply 外不改动工作区和分支。冲突检测用
`git merge-tree --write-tree`，纯计算，不碰工作树。
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BASELINE = REPO / ".github" / "upstream-baseline.json"

FIX_KINDS = {"fix", "perf"}
HOST_SCOPES = {"mtp", "preview"}
KIND_NAMES = {
    "fix": "修复",
    "perf": "性能",
    "refactor": "重构",
    "test": "测试",
    "build": "构建",
    "ci": "CI",
    "docs": "文档",
    "chore": "杂务",
    "feat": "新功能",
    "style": "格式",
    "revert": "回退",
}


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    exe = shutil.which("git") or "git"
    return subprocess.run(
        [exe, *args], cwd=REPO, capture_output=True, text=True,
        encoding="utf-8", errors="replace", check=check,
    )


def git_raw(*args: str) -> bytes:
    exe = shutil.which("git") or "git"
    return subprocess.run([exe, *args], cwd=REPO, capture_output=True).stdout


def patch_id(sha: str) -> str | None:
    """提交的 patch-id：与提交哈希无关，只看改动内容，用来判断是否已应用过。"""
    exe = shutil.which("git") or "git"
    show = git_raw("show", "--patch", "--format=", sha)
    if not show.strip():
        return None
    r = subprocess.run([exe, "patch-id", "--stable"], cwd=REPO, input=show, capture_output=True)
    txt = r.stdout.decode("utf-8", "replace").strip()
    return txt.split()[0] if txt else None


def applied_ids(limit: int = 60) -> tuple[set[str], set[str]]:
    """HEAD 最近若干提交的 (patch-id 集合, 标题集合)。

    cherry-pick 会保留原提交标题，但周边代码不同可能让 patch-id 变掉，
    所以两个都用：任一命中即认为已经应用过。
    """
    shas = out("log", f"-{limit}", "--format=%H%x1f%s").splitlines()
    ids, subjects = set(), set()
    for line in shas:
        if not line.strip():
            continue
        sha, subject = line.split("\x1f", 1)
        subjects.add(subject.strip())
        pid = patch_id(sha)
        if pid:
            ids.add(pid)
    return ids, subjects


def out(*args: str) -> str:
    return git(*args).stdout.strip()


def rev(ref: str) -> str:
    return out("rev-parse", ref)


def resolve_tag(tag: str) -> str:
    """把 tag 解成真实提交（处理 annotated tag）。"""
    try:
        return rev(f"{tag}^{{commit}}")
    except subprocess.CalledProcessError:
        return rev(tag)


def latest_tag() -> str:
    tags = out("tag", "-l", "v[0-9]*", "--sort=-v:refname").splitlines()
    if not tags:
        raise SystemExit("没有上游 tag，先跑 --fetch")
    return tags[0]


def latest_target(remote: str) -> str:
    """上游该跟到哪儿：分支头，而不是最后一个 tag。

    上游常常先合提交、过几天才打 tag（0.3.1 之后 main 又进了两个 fix）。
    只看 tag 会把「main 已经修好了、还没发版」当成「没有更新」。
    """
    for ref in (f"{remote}/main", f"{remote}/master"):
        if git("rev-parse", "--verify", "--quiet", ref, check=False).returncode == 0:
            return ref
    tags = out("tag", "-l", "v[0-9]*", "--sort=-v:refname").splitlines()
    if not tags:
        raise SystemExit("没有上游分支或 tag，先跑 --fetch")
    return tags[0]


def nearest_tag(ref: str) -> str | None:
    """`ref` 上能到达的最近 tag。

    不能拿全局最新的 tag：布丁仓库的 tag 也在同一个 git 里（v0.3.9），
    按版本号排会排到 skyward 前面，把「上游只有 v0.3.1」误报成落后 300 多个提交。
    """
    r = git("describe", "--tags", "--abbrev=0", ref, check=False)
    return r.stdout.strip() if r.returncode == 0 else None


def behind_tag(head: str) -> list[dict]:
    """`head` 里有、它最近的那个 tag 里没有的提交。"""
    tag = nearest_tag(head)
    if not tag:
        return []
    return commits(resolve_tag(tag), head)


def pudding_report(baseline: dict) -> None:
    """布丁仓库的差距，单独报。

    它是另一个机型（`pudding`，小米 17）的补丁层，整份 diff 都压在上游
    某个 tag 上，和本仓库的定制（`popsicle`，17 Pro Max）不能直接合。
    这里只回答两件事：它到哪一版了、比我们跟的基线新在哪。
    """
    tag = baseline.get("pudding", {}).get("tag")
    if not tag:
        return
    print(f"\n{'='*68}\n布丁仓库（mariusarva/skyward-pudding）\n{'='*68}")
    if git("rev-parse", "--verify", "--quiet", "pudding/main", check=False).returncode != 0:
        print("  没有 pudding/main，先跑 --fetch。")
        return
    tip = nearest_tag("pudding/main")
    if not tip:
        print("  pudding/main 上没有任何 tag。")
        return
    if resolve_tag(tag) == resolve_tag(tip):
        print(f"  已是最新：基线 {tag} = 它最新 {tip}。")
        return
    rows = commits(resolve_tag(tag), resolve_tag(tip))
    print(f"  基线 {tag} → 它最新 {tip}：{len(rows)} 个提交")
    for r in rows:
        print(f"    {r['short']}  {r['subject'][:64]}")
    changed = len(out("diff", "--name-only", resolve_tag(tag), resolve_tag(tip)).splitlines())
    print(f"  改动文件数：{changed}")
    print("  注意：它整份 diff 压在上游某个 tag 上，且面向 pudding 机型（小米 17），")
    print("        本仓库是 popsicle（17 Pro Max）、结构停在 0.3.0，不能直接套补丁。")
    print("        要取它的具体改动，按 docs/SYNC-UPSTREAM.md 第四节逐条比。")


def kind_of(subject: str) -> str:
    m = re.match(r"^([a-z]+)(\([^)]*\))?!?:", subject)
    return m.group(1) if m else "other"


def label(kind: str) -> str:
    return KIND_NAMES.get(kind, kind)


def commits(base: str, head: str) -> list[dict]:
    raw = out("log", "--reverse", "--format=%H%x1f%s", f"{base}..{head}")
    rows = []
    for line in raw.splitlines():
        if not line.strip():
            continue
        sha, subject = line.split("\x1f", 1)
        rows.append({"sha": sha, "short": sha[:7], "subject": subject, "kind": kind_of(subject)})
    return rows


def scope_of(subject: str) -> str:
    m = re.match(r"^[a-z]+\(([^)]*)\)!?:", subject)
    return m.group(1) if m else ""


def conflicts(base_tree_ref: str, ours: str, theirs: str) -> tuple[bool, list[str]]:
    """把 theirs 的改动合到 ours 上会不会冲突。base_tree_ref 是三方合并的基点。"""
    r = git("merge-tree", "--write-tree", "--name-only", "--messages",
            f"--merge-base={base_tree_ref}", ours, theirs, check=False)
    if r.returncode == 0:
        return False, []
    files = []
    for line in r.stdout.splitlines():
        if line.startswith("CONFLICT"):
            m = re.search(r"in (\S+)$", line)
            if m:
                files.append(m.group(1))
    if not files:
        files = [l.strip() for l in r.stdout.splitlines()
                 if l.strip() and not l.startswith(("Auto-merging", "CONFLICT", "changed in both"))]
    return True, sorted(set(files))


def shortstat(a: str, b: str) -> str:
    s = out("diff", "--shortstat", a, b)
    return s or "无差异"


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    ap = argparse.ArgumentParser(description="侦察上游更新")
    ap.add_argument("--fetch", action="store_true", help="先联网抓上游分支与 tag")
    ap.add_argument("--remote", default="pudding", help="上游 remote 名（默认 pudding）")
    ap.add_argument("--base", help="基线 tag（默认读 .github/upstream-baseline.json）")
    ap.add_argument("--head", help="目标 tag（默认本地最新的上游 tag）")
    ap.add_argument("--apply", action="store_true", help="预演全干净时真正 cherry-pick 修复类提交")
    ap.add_argument("--out", help="把报告写到文件")
    args = ap.parse_args()

    if args.fetch:
        # 分支头也要抓：上游常先合提交、过几天才打 tag，只抓 --tags 会看不到。
        for remote in dict.fromkeys([args.remote, "pudding"]):
            if git("remote", "get-url", remote, check=False).returncode != 0:
                continue
            print(f"抓取 {remote} ...", file=sys.stderr)
            git("fetch", remote, "--tags", "--depth=400", check=False)
            git("fetch", remote, "--depth=400", f"+refs/heads/*:refs/remotes/{remote}/*", check=False)

    baseline = json.loads(BASELINE.read_text(encoding="utf-8")) if BASELINE.exists() else {}
    # 只跟布丁。`commit` 优先于 `tag`：它的 tag 比提交慢一版（v0.3.17 打在
    # 「发布 v0.3.16」那个提交上），钉提交才看得见 tag 之后的改动。
    pudding = baseline.get("pudding", {})
    base_tag = args.base or pudding.get("commit") or pudding.get("tag")
    if not base_tag:
        raise SystemExit("没有基线，用 --base 指定一个上游 tag 或提交")

    # 默认跟分支头，不跟最近的 tag：布丁的 tag 有一版滞后。
    head_ref = args.head or f"{args.remote}/main"
    if resolve_tag(base_tag) == resolve_tag(head_ref):
        print(f"已是最新：基线 {base_tag} = 上游 {head_ref}，无需同步。")
        pudding_report(baseline)
        return 0

    base_sha, head_sha = resolve_tag(base_tag), resolve_tag(head_ref)
    rows = commits(base_sha, head_sha)

    done_ids, done_subjects = applied_ids()
    for r in rows:
        r["applied"] = r["subject"].strip() in done_subjects or patch_id(r["sha"]) in done_ids

    fixes = [r for r in rows if r["kind"] in FIX_KINDS and not r["applied"]]
    already = [r for r in rows if r["kind"] in FIX_KINDS and r["applied"]]
    churn = [r for r in rows if r["kind"] not in FIX_KINDS]

    ours_files = set(out("diff", "--name-only", head_sha, "HEAD").splitlines())

    tag_note = ""
    newest = nearest_tag(head_ref)
    if newest and head_ref != newest:
        ahead = behind_tag(head_ref)
        if ahead:
            tag_note = f"（领先 tag {newest} {len(ahead)} 个提交，还没发版）"
    print(f"\n基线 {base_tag} → 上游 {head_ref} {tag_note}")
    print(f"规模：{len(rows)} 个提交 · {shortstat(base_sha, head_sha)}")
    print(f"工作区 HEAD：{out('log', '-1', '--format=%h %s')}")
    if already:
        print(f"已包含：{len(already)} 个修复在 HEAD 里已经存在，跳过（{', '.join(r['short'] for r in already)}）")

    print(f"\n{'='*68}\n提交分类\n{'='*68}")
    for r in rows:
        if r["kind"] in FIX_KINDS:
            mark = "已应用" if r["applied"] else "修复  "
        else:
            mark = "      "
        print(f"  [{mark}] {r['short']}  {r['subject']}")

    print(f"\n{'='*68}\n修复类预演（cherry-pick 到当前 HEAD）\n{'='*68}")
    results = []
    if not fixes:
        print("  这次上游没有任何修复类提交。")
    for r in fixes:
        try:
            parent = rev(f"{r['sha']}^")
        except subprocess.CalledProcessError:
            print(f"  {r['short']}  跳过（根提交）")
            continue
        bad, files = conflicts(parent, "HEAD", r["sha"])
        touched = set(out("show", "--name-only", "--format=", r["sha"]).splitlines())
        overlap = sorted(touched & ours_files)
        results.append((r, bad, files, overlap))
        state = "冲突" if bad else "干净"
        host = "  ← 电脑端编译相关，不影响手机" if scope_of(r["subject"]) in HOST_SCOPES else ""
        note = f"  冲突文件: {', '.join(files)}" if bad else ""
        extra = f"  （碰了 {len(overlap)} 个我们改过的文件: {', '.join(overlap[:4])}）" if overlap and not bad else ""
        print(f"  {r['short']}  [{state}] {r['subject'][:52]}{note}{extra}{host}")

    blocked = [x for x in results if x[1]]
    ours_changed = sorted({f for x in results for f in x[3]})

    print(f"\n{'='*68}\n建议\n{'='*68}")
    if not rows:
        print("  上游没有新提交。")
    elif not fixes:
        print("  全是重构/工程改动，没有功能修复。")
        print("  → 建议：不动。等上游出功能版本或布丁适配。")
    elif blocked:
        print(f"  {len(blocked)} 个修复有冲突，不能盲目 cherry-pick。")
        print(f"  → 建议：逐个手工处理 {', '.join(x[0]['short'] for x in blocked)}，")
        print("    或改用 docs/SYNC-UPSTREAM.md 第二节的三方合并。")
    elif churn and len(churn) > len(fixes) * 0.6:
        print(f"  修复 {len(fixes)} 个、非功能改动 {len(churn)} 个，且修复全部零冲突。")
        print("  → 建议：只 cherry-pick 修复，跳过重构（本仓库当前路线）。")
    else:
        print("  修复全部零冲突，非功能改动占比不高。")
        print("  → 可以顺手把上游全部提交 cherry-pick 过来。")

    if ours_changed:
        print(f"\n  注意：有修复碰到了我们改过的文件 —— {', '.join(ours_changed)}")
        print("  预演干净不代表语义正确，合并后必须编译验证。")

    print(f"\n{'='*68}\n下一步\n{'='*68}")
    if fixes and not blocked:
        print("  python scripts/sync_upstream.py --base %s --head %s --apply" % (base_tag, head_ref))
        print("  然后：cargo check --workspace --target aarch64-linux-android")
        print("        跑 scripts/local_build.py 出包 → 比对 ramdisk 条目数")
    else:
        print("  按 docs/SYNC-UPSTREAM.md 的对应路线手工处理。")

    pudding_report(baseline)

    if args.apply:
        if blocked:
            print("\n拒绝 --apply：有冲突的修复，先人工处理。", file=sys.stderr)
            return 2
        print(f"\n开始 cherry-pick {len(fixes)} 个修复 ...")
        for r in fixes:
            git("cherry-pick", r["sha"])
            print(f"  已应用 {r['short']}  {r['subject'][:56]}")
        print("完成。记得编译验证后再推。")

    return 0


if __name__ == "__main__":
    sys.exit(main())
