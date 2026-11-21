#!/usr/bin/env python3
r"""把出包基线升到新的官方系统版本。

官方发版后 recovery 分区跟着变，`devices/popsicle.toml` 的 `[stock]` 必须
跟着换。这个脚本把整条链走完：

    1. 查新版本全量包的文件名（xmfirmwareupdater），拼出官方下载地址；
    2. 改 `[stock]`，`recovery` 摘要先留空；
    3. 提交推送，等 CI 跑完；
    4. 从 CI 日志里抓出新包的 recovery SHA-256，回填；
    5. 再提交推送一次，从此每次构建都校验摘要。

用法：
    python scripts/bump_stock.py --version OS4.0.7.0.XPBCNXM
    python scripts/bump_stock.py --version OS4.0.7.0.XPBCNXM --sha256 <64 位>
    python scripts/bump_stock.py --version OS4.0.7.0.XPBCNXM --dry-run

只改本机文件、不提交也不等待：--dry-run。

路径、令牌、代理都是自动的，换机器不用改代码：
- 仓库位置 = 本脚本所在目录的上一级
- 令牌：--token-file → GITHUB_TOKEN/GH_TOKEN → ~/.config/skyward/token → 原机器路径
- 代理：本机 127.0.0.1:7890 通才走，不通直连（--no-proxy 强制直连，--proxy 指定别的）
"""
import argparse
import base64
import io
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path
from urllib.parse import quote, unquote

WORK = Path(__file__).resolve().parents[1]
REPO = 'Ff428/skyward-popsicle'
BRANCH = 'main'
PROXY = 'http://127.0.0.1:7890'
INDEX = 'https://xmfirmwareupdater.com/hyperos/popsicle/'
MIRROR = 'https://bkt-sgp-miui-ota-update-alisgp.oss-ap-southeast-1.aliyuncs.com'
UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131 Safari/537.36'

GIT_CANDIDATES = [
    Path("D:/软件/应用/PortableGit/mingw64/bin/git.exe"),
    Path("D:/软件/应用/PortableGit/cmd/git.exe"),
    Path("/usr/bin/git"),
    Path("/usr/local/bin/git"),
]

# 由 main() 的 --no-proxy / --proxy 决定；None 表示还没探测。
_PROXY_URL = None


def find_git():
    for p in GIT_CANDIDATES:
        if p.exists():
            return str(p)
    return shutil.which('git') or 'git'


def _port_open(host, port, timeout=0.6):
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def proxy_url():
    """探测一次，缓存。返回 None 表示直连。"""
    global _PROXY_URL
    if _PROXY_URL is None:
        _PROXY_URL = PROXY if _port_open('127.0.0.1', 7890) else ''
    return _PROXY_URL or None


def log(*a):
    print(time.strftime('%H:%M:%S'), *a, flush=True)


_TOKEN_ARGS = {'path': None}


def token():
    t = ''
    if _TOKEN_ARGS['path']:
        p = Path(_TOKEN_ARGS['path'])
        if p.is_file():
            t = p.read_text(encoding='utf-8').strip()
        if not t:
            sys.exit('--token-file 指向的文件不存在或是空的：' + str(p))
    if not t:
        t = (os.environ.get('GITHUB_TOKEN') or os.environ.get('GH_TOKEN') or '').strip()
    if not t:
        for p in (Path.home() / '.config' / 'skyward' / 'token',
                  Path('D:/WorkBuddyAI/github/.token')):
            if p.is_file():
                t = p.read_text(encoding='utf-8').strip()
                if t:
                    break
    if not t:
        sys.exit('没有令牌。先跑 python scripts/gh_device_flow.py，'
                 '或设 GITHUB_TOKEN，或用 --token-file 指定。')
    return t


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def _opener(follow=True):
    p = proxy_url()
    handlers = [urllib.request.ProxyHandler({'http': p, 'https': p})] if p else []
    if not follow:
        handlers.insert(0, _NoRedirect())
    return urllib.request.build_opener(*handlers)


def http_get(url, timeout=120, follow=True):
    req = urllib.request.Request(url, headers={'User-Agent': UA})
    with _opener(follow).open(req, timeout=timeout) as r:
        return r.read()


def http_text(url, timeout=120):
    return http_get(url, timeout).decode('utf-8', 'replace')


def head_ok(url):
    req = urllib.request.Request(url, method='HEAD', headers={'User-Agent': UA})
    try:
        with _opener(True).open(req, timeout=60) as r:
            return r.status, int(r.headers.get('Content-Length') or 0)
    except urllib.error.HTTPError as e:
        return e.code, 0
    except Exception as e:
        return type(e).__name__, 0


def api(method, path, body=None):
    req = urllib.request.Request('https://api.github.com' + path, method=method)
    req.add_header('Authorization', 'Bearer ' + token())
    req.add_header('Accept', 'application/vnd.github+json')
    req.add_header('X-GitHub-Api-Version', '2022-11-28')
    req.add_header('User-Agent', 'skyward-bump')
    data = None
    if body is not None:
        data = json.dumps(body).encode()
        req.add_header('Content-Type', 'application/json')
    try:
        with urllib.request.urlopen(req, data, timeout=60) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw.strip() else {})
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode('utf-8', 'replace')[:400]


def git(*args, check=True):
    """在仓库工作区里直接跑 git。

    过去这里用 GIT_DIR 指向另一套快照仓库，那套结构已经废弃——用它会基于
    旧树的索引提交，把后来的文件全丢掉，推上去也会因为不是快进被拒。
    """
    exe = find_git()
    env = dict(os.environ, GIT_TERMINAL_PROMPT='0', GCM_INTERACTIVE='never')
    env['PATH'] = str(Path(exe).parent) + os.pathsep + env.get('PATH', '')
    env.pop('GIT_DIR', None)
    env.pop('GIT_WORK_TREE', None)
    p = proxy_url()
    if p:
        env['http_proxy'] = env['https_proxy'] = p
    p = subprocess.run((exe,) + args, cwd=str(WORK), env=env,
                       capture_output=True, text=True, encoding='utf-8', errors='replace')
    shown = ' '.join(a if 'AUTHORIZATION' not in a.upper() else '<redacted>' for a in args)
    log('$ git', shown[:110], '->', p.returncode)
    if p.stdout.strip():
        print(p.stdout.strip()[:600])
    if p.stderr.strip():
        print(p.stderr.strip()[:600])
    if check and p.returncode != 0:
        sys.exit('git 失败')
    return p


def find_package(version):
    """在 xmfirmwareupdater 上找这个版本的全量包文件名，返回官方下载地址。"""
    log('查', INDEX, '上的', version, '…')
    index = http_text(INDEX)
    m = re.search(r'/hyperos/popsicle/([^/"\']+)/' + re.escape(version) + r'/', index)
    if not m:
        return None, None
    branch = m.group(1)
    # 页面里的分支名可能带未编码的空格（stable beta），拼 URL 前先规整。
    branch = quote(unquote(branch))
    page_url = f'https://xmfirmwareupdater.com/hyperos/popsicle/{branch}/{version}/'
    log('页面：', page_url)
    page = http_text(page_url)
    name = re.search(rf'(popsicle-ota_full-{re.escape(version)}-user-\d+(?:\.\d+)?-[0-9a-f]{{8,}}\.zip)', page)
    if not name:
        return branch, None
    return branch, f'{MIRROR}/{version}/{name.group(1)}'


def set_stock(text, version, package, recovery):
    """只换 [stock] 段（连同它上面的注释），别处一字不动。"""
    lines = text.splitlines()
    out, i = [], 0
    while i < len(lines):
        if lines[i].strip() == '[stock]':
            while out and out[-1].lstrip().startswith('#'):
                out.pop()
            out += [
                f"# The stock build the recovery is made on: the Chinese system's {version},",
                '# whose full update package on the phone maker\'s own download server holds',
                '# the recovery partition the recovery is built on top of.',
                '[stock]',
                f'version = "{version}"',
                f'package = "{package}"',
                f'recovery = "{recovery}"',
            ]
            i += 1
            while i < len(lines) and not lines[i].lstrip().startswith('['):
                i += 1
            continue
        out.append(lines[i])
        i += 1
    return '\n'.join(out) + '\n'


def toml_path(device):
    return str(WORK / 'devices' / f'{device}.toml')


def read_stock(path):
    t = open(path, encoding='utf-8').read()
    sec = t.split('[stock]', 1)
    if len(sec) < 2:
        sys.exit(path + ' 里没有 [stock] 段')
    body = sec[1].split('\n[', 1)[0]
    ver = re.search(r'version\s*=\s*"([^"]*)"', body)
    pkg = re.search(r'package\s*=\s*"([^"]*)"', body)
    rec = re.search(r'recovery\s*=\s*"([^"]*)"', body)
    return (ver.group(1) if ver else ''), (pkg.group(1) if pkg else ''), (rec.group(1) if rec else '')


def commit_push(message):
    git('add', 'devices/popsicle.toml')
    p = git('status', '--short', check=False)
    if not p.stdout.strip():
        log('没有改动，跳过提交')
        return git('rev-parse', 'HEAD').stdout.strip()
    git('commit', '-m', message)
    head = git('rev-parse', 'HEAD').stdout.strip()
    owner = api('GET', '/user')[1].get('login', 'Ff428')
    import base64
    basic = base64.b64encode(f'{owner}:{token()}'.encode()).decode()
    auth = f'http.https://github.com/.extraheader=Authorization: Basic {basic}'
    git('-c', auth, '-c', 'http.postBuffer=524288000', 'push', 'origin', BRANCH)
    log('已推送', head[:7])
    return head


def wait_run(head_sha, timeout=3000):
    """等这次推送触发的 workflow 跑完。"""
    start = time.time()
    seen = None
    while time.time() - start < timeout:
        _, runs = api('GET', f'/repos/{REPO}/actions/runs?per_page=20')
        run = next((r for r in runs.get('workflow_runs', []) if r['head_sha'] == head_sha), None)
        if run is None:
            log('等 CI 启动…')
        else:
            line = f"run #{run['run_number']} {run['status']}/{run['conclusion']}"
            if line != seen:
                log(line, '|', run['html_url'])
                seen = line
            if run['status'] == 'completed':
                return run
        time.sleep(15)
    return None


def failing_steps(run_id):
    _, jobs = api('GET', f'/repos/{REPO}/actions/runs/{run_id}/jobs?per_page=20')
    bad = []
    for j in jobs.get('jobs', []):
        for s in j.get('steps', []):
            if s.get('conclusion') == 'failure':
                bad.append(f"{j['name']} / {s['name']}")
    return bad


def recovery_sha_from_logs(run_id):
    """下载这次运行的日志 zip，找打包器打印的 recovery 分区摘要。"""
    url = f'https://api.github.com/repos/{REPO}/actions/runs/{run_id}/logs'
    req = urllib.request.Request(url, headers={'Authorization': 'Bearer ' + token(), 'User-Agent': UA})
    try:
        _opener(False).open(req, timeout=60)
        sys.exit('日志没有重定向，GitHub 的行为变了？')
    except urllib.error.HTTPError as e:
        loc = e.headers.get('Location')
    if not loc:
        return None
    blob = http_get(loc, timeout=300)
    log(f'日志 {len(blob)/1048576:.1f} MB，找摘要…')
    zf = zipfile.ZipFile(io.BytesIO(blob))
    text = '\n'.join(zf.read(n).decode('utf-8', 'replace') for n in zf.namelist())
    m = re.search(r'has the SHA-256 ([0-9a-f]{64})', text)
    return m.group(1) if m else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--version', required=True, help='如 OS4.0.7.0.XPBCNXM')
    ap.add_argument('--package', help='官方全量包地址；不给就自动查')
    ap.add_argument('--sha256', help='已知的 recovery 分区摘要；给了就不用等 CI')
    ap.add_argument('--device', default='popsicle')
    ap.add_argument('--dry-run', action='store_true', help='只改文件，不提交不推送')
    ap.add_argument('--token-file', help='令牌文件；不给就自动找')
    ap.add_argument('--no-proxy', action='store_true', help='强制直连')
    ap.add_argument('--proxy', help='指定代理，如 http://127.0.0.1:1080')
    a = ap.parse_args()

    global _PROXY_URL
    if a.no_proxy:
        _PROXY_URL = ''
    elif a.proxy:
        _PROXY_URL = a.proxy
    _TOKEN_ARGS['path'] = a.token_file

    version = a.version.strip()
    if not re.fullmatch(r'OS\d+(?:\.\d+)+\.\w+', version):
        sys.exit(f'版本串看着不对：{version}（形如 OS4.0.7.0.XPBCNXM）')

    path = toml_path(a.device)
    if not os.path.isfile(path):
        sys.exit('找不到 ' + path)
    old_ver, old_pkg, old_rec = read_stock(path)
    log('当前配置：', old_ver, '| 摘要', (old_rec[:12] + '…') if old_rec else '(空)')
    if old_ver == version and old_rec and not a.dry_run:
        log('已经是这个版本且摘要在，什么都不用做')
        return

    package = a.package
    if not package:
        branch, package = find_package(version)
        if not package:
            sys.exit('没查到包名。到 ' + INDEX + ' 找这个版本的 Recovery 包，用 --package 传进来')
        log('分支：', branch)
    code, size = head_ok(package)
    log(f'包地址 HEAD {code}，{size/1073741824:.2f} GB')
    if code != 200:
        sys.exit('包下不下来，先确认地址')
    if size and size < 1 << 30:
        sys.exit('这个包不到 1 GB，八成是增量包，要全量包')

    text = open(path, encoding='utf-8').read()
    sha = (a.sha256 or '').strip().lower()
    if sha and not re.fullmatch(r'[0-9a-f]{64}', sha):
        sys.exit('--sha256 得是 64 位十六进制')
    open(path, 'w', encoding='utf-8', newline='\n').write(
        set_stock(text, version, package, sha))
    log('已改', path, '→', version, '| 摘要', sha or '(留空待回填)')

    if a.dry_run:
        log('--dry-run：不提交')
        return

    head = commit_push(f'feat(device): 出包基线升到 {version}')
    if sha:
        log('摘要已知，完成。')
        return

    run = wait_run(head)
    if not run:
        sys.exit('等 CI 超时，稍后自己看 ' + f'https://github.com/{REPO}/actions')
    if run['conclusion'] != 'success':
        bad = failing_steps(run['id'])
        sys.exit(f"CI 没通过（{run['conclusion']}）：" + '；'.join(bad) + '\n看 ' + run['html_url'])

    found = recovery_sha_from_logs(run['id'])
    if not found:
        sys.exit(f"日志里没找到摘要，去 {run['html_url']} 搜 'SHA-256' 手工回填")

    log('抓到摘要：', found)
    text = open(path, encoding='utf-8').read()
    open(path, 'w', encoding='utf-8', newline='\n').write(
        set_stock(text, version, package, found))
    commit_push(f'fix(device): 回填 {version} 的 recovery 摘要，恢复强校验')
    log('完成：' + f'https://github.com/{REPO}')
    log('下一次构建起，摘要对不上会直接报错。')


if __name__ == '__main__':
    main()
