#!/usr/bin/env python3
"""Push the current tree to mariusarva/skyward-popsicle through a fork + PR.

That repository is not writable for us: it holds two commits and five v40 files,
and `master` is its only branch.  The only way in is a pull request whose head
lives on a fork.  This script rebuilds that head branch from scratch every time:

    master (afe1481) -> [ one commit whose tree == our tree, minus the paths below ]

Nothing in the working tree or in the local branch is touched: the tree is
assembled in a throw-away index, committed with `commit-tree`, and only the
branch ref used by the pull request is moved (force).

Usage:

    python scripts/sync_to_popsicle.py                 # build, show the diff, stop
    python scripts/sync_to_popsicle.py --push          # build and update the PR
    python scripts/sync_to_popsicle.py --source HEAD --base popsicle/master

Defaults: source = HEAD, base = popsicle/master, branch = v96,
fork = Ff428/skyward-popsicle-1, excluded paths = backup/ .token
"""

from __future__ import annotations

import argparse
import base64
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOKEN = r"D:\WorkBuddyAI\github\.token"
# github.com:443 is only reachable through this address from this machine;
# every other A record either times out or resets the connection.
GOOD_IP = "140.82.112.3"


def git(*args: str, env: dict[str, str] | None = None, check: bool = True) -> str:
    p = subprocess.run(
        ["git", *args],
        cwd=REPO,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
    )
    if check and p.returncode != 0:
        sys.stderr.write(f"git {' '.join(args)} failed ({p.returncode})\n{p.stderr}\n")
        raise SystemExit(1)
    return p.stdout


def resolve(ref: str) -> str:
    return git("rev-parse", "--verify", ref + "^{commit}").strip()


def has_ref(ref: str) -> bool:
    p = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", ref + "^{commit}"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    return p.returncode == 0


def fetch_base(remote: str, branch: str) -> None:
    """Fetch the base branch; tolerate failure when the object is already here."""
    spec = f"refs/heads/{branch}:refs/remotes/{remote}/{branch}"
    args = [
        "git",
        "-c",
        f"http.curloptResolve=github.com:443:{GOOD_IP}",
        "fetch",
        "--no-tags",
        remote,
        spec,
    ]
    p = subprocess.run(args, cwd=REPO, capture_output=True, text=True)
    if p.returncode != 0:
        sys.stderr.write(
            f"fetch {remote}/{branch} failed, using the local copy if there is one\n"
            f"  {p.stderr.strip().splitlines()[-1] if p.stderr.strip() else ''}\n"
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", default="HEAD", help="tree to publish (default HEAD)")
    ap.add_argument("--base", default="popsicle/master", help="commit to sit on top of")
    ap.add_argument("--branch", default="v96", help="branch on the fork that the PR watches")
    ap.add_argument("--fork", default="Ff428/skyward-popsicle-1", help="owner/repo of the fork")
    ap.add_argument("--exclude", nargs="*", default=["backup", ".token"],
                    help="paths kept out of the published tree")
    ap.add_argument("--message", default=None, help="commit message")
    ap.add_argument("--push", action="store_true", help="actually move the branch")
    ap.add_argument("--dry-run", action="store_true", help="build the tree, print, do not commit")
    args = ap.parse_args()

    remote, _, base_branch = args.base.partition("/")
    if base_branch and not has_ref(f"refs/remotes/{remote}/{base_branch}"):
        print(f"-> fetching {remote}/{base_branch}")
        fetch_base(remote, base_branch)

    source_sha = resolve(args.source)
    base_sha = resolve(args.base)
    source_tree = git("rev-parse", source_sha + "^{tree}").strip()
    print(f"source {args.source} = {source_sha[:7]}  tree {source_tree[:7]}")
    print(f"base   {args.base} = {base_sha[:7]}")

    index = os.path.join(REPO, ".git", "sync-to-popsicle.index")
    if os.path.exists(index):
        os.remove(index)
    env = os.environ.copy()
    env["GIT_INDEX_FILE"] = index

    try:
        git("read-tree", source_sha, env=env)
        total = len(git("ls-files", env=env).splitlines())
        for path in args.exclude:
            git("rm", "-r", "--cached", "-q", "--ignore-unmatch", "--", path, env=env)
        kept = len(git("ls-files", env=env).splitlines())
        tree = git("write-tree", env=env).strip()
    finally:
        # The throw-away index runs into megabytes; never leave it behind.
        if os.path.exists(index):
            os.remove(index)
    print(f"tree   {tree[:7]}  {kept} files ({total - kept} excluded)")

    short = git("rev-parse", "--short", source_sha).strip()
    message = args.message or (
        f"sync: the {short} tree, on top of the v40 notes\n\n"
        f"Tree of Ff428/skyward-popsicle @ {source_sha}\n"
        f"Left out: {', '.join(args.exclude)}\n"
    )

    if args.dry_run:
        print("\n--dry-run: nothing committed")
        return 0

    commit = git("commit-tree", tree, "-p", base_sha, "-m", message).strip()
    print(f"commit {commit[:7]}  parents {base_sha[:7]}")

    # What the pull request will show against its base.
    stat = git("diff", "--shortstat", base_sha, commit).strip()
    print(f"vs {args.base}: {stat}")
    renamed = git("diff", "--name-status", "-M", base_sha, commit).splitlines()
    kinds: dict[str, int] = {}
    for line in renamed:
        kinds[line.split("\t")[0][0]] = kinds.get(line.split("\t")[0][0], 0) + 1
    print("  " + "  ".join(f"{k}:{v}" for k, v in sorted(kinds.items())))

    # One fixed local ref: the branch is rebuilt every time anyway, and a ref per
    # sha would pile up one line of `git branch` output per sync.
    head_branch = "refs/heads/sync/popsicle"
    git("update-ref", head_branch, commit)
    print(f"local ref {head_branch} -> {commit[:7]}")

    if not args.push:
        print("\nref moved locally only; re-run with --push to update the pull request")
        print(f"  git push --force https://github.com/{args.fork}.git "
              f"{head_branch}:refs/heads/{args.branch}")
        return 0

    with open(TOKEN, "r", encoding="utf-8") as fh:
        token = fh.read().strip()
    user = args.fork.split("/")[0]
    auth = "Basic " + base64.b64encode(f"{user}:{token}".encode()).decode()
    url = f"https://github.com/{args.fork}.git"
    p = subprocess.run(
        [
            "git",
            "-c", f"http.curloptResolve=github.com:443:{GOOD_IP}",
            "-c", f"http.extraheader=Authorization: {auth}",
            "push", "--force", url,
            f"{head_branch}:refs/heads/{args.branch}",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    sys.stdout.write(p.stdout)
    sys.stderr.write(p.stderr)
    if p.returncode != 0:
        return p.returncode

    print(f"\nupdated https://github.com/{args.fork}/tree/{args.branch}")
    print(f"pull request  https://github.com/mariusarva/skyward-popsicle/pulls/1")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
