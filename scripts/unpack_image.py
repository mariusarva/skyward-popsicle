#!/usr/bin/env python3
"""解包 Skyward recovery 的 boot 镜像（boot image v4 + LZ4 legacy 压缩的 ramdisk）。

    python scripts/unpack_image.py --ref stock.img --out work

产出：
    work/ramdisk.cpio     原始 cpio（未改动，重新打包时以它为骨架）
    work/ramdisk/         展开出来的 ramdisk 目录树
    work/head.bin         前 4096 字节的镜像头（kernel_size=0，ramdisk_size 在 12..16）

只依赖标准库 + lz4（pip install lz4）。
"""
import argparse
import os
import struct
import sys

try:
    import lz4.block
except ImportError:  # pragma: no cover
    sys.exit('需要 lz4 模块：pip install lz4')

LZ4_LEGACY_MAGIC = 0x184C2102
BLOCK = 8 * 1024 * 1024


def decompress(f, size):
    """读完 LZ4 legacy 帧：magic + 每个块前 4 字节长度。"""
    out = bytearray()
    magic, first = struct.unpack('<II', f.read(8))
    if magic != LZ4_LEGACY_MAGIC:
        sys.exit(f'ramdisk 不是 LZ4 legacy 帧：{magic:#x}')
    out += lz4.block.decompress(f.read(first), uncompressed_size=BLOCK)
    consumed = 8 + first
    while consumed < size:
        (sz,) = struct.unpack('<I', f.read(4))
        consumed += 4
        if sz == 0x80000000:  # 未压缩块
            (raw,) = struct.unpack('<I', f.read(4))
            consumed += 4
            out += f.read(raw)
            consumed += raw
        elif sz == 0:  # 结束标记
            break
        else:
            out += lz4.block.decompress(f.read(sz), uncompressed_size=BLOCK)
            consumed += sz
    if len(out) != size:
        print(f'  注意：解出 {len(out)} B，头部声明 {size} B')
    return bytes(out)


def read_header(f):
    head = f.read(4096)
    if head[:8] != b'ANDROID!':
        sys.exit('不是 Android boot image（缺少 ANDROID! 魔数）')
    (kernel_size,) = struct.unpack('<I', head[8:12])
    (ramdisk_size,) = struct.unpack('<I', head[12:16])
    return head, kernel_size, ramdisk_size


def parse_cpio(data):
    """newc cpio → [(name, mode, body)]，按原顺序。"""
    entries = []
    pos = 0
    while pos < len(data):
        if data[pos:pos + 6] != b'070701':
            break
        hx = lambda off, n: int(data[pos + off:pos + off + n], 16)  # noqa: E731
        fsize, namesize = hx(54, 8), hx(94, 8)
        mode = hx(14, 8)
        name = data[pos + 110:pos + 110 + namesize - 1]
        fstart = (pos + 110 + namesize + 3) & ~3
        fend = fstart + fsize
        entries.append((name, mode, data[fstart:fend]))
        if name == b'TRAILER!!!':
            break
        pos = (fend + 3) & ~3
    return entries


def safe_join(root, name):
    rel = name.decode('utf-8', 'replace').lstrip('/')
    if rel.startswith('./'):
        rel = rel[2:]
    dst = os.path.join(root, *rel.split('/'))
    if not os.path.abspath(dst).startswith(os.path.abspath(root)):
        sys.exit(f'cpio 里有越界路径，拒绝解出：{name!r}')
    return dst


def extract(entries, root):
    n = 0
    for name, mode, body in entries:
        if name == b'TRAILER!!!':
            continue
        dst = safe_join(root, name)
        kind = mode & 0o170000
        if kind == 0o040000:
            os.makedirs(dst, exist_ok=True)
        elif kind == 0o120000:
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            if os.path.lexists(dst):
                os.remove(dst)
            os.symlink(body.decode(), dst)
        else:
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            with open(dst, 'wb') as w:
                w.write(body)
            os.chmod(dst, mode & 0o777)
            n += 1
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ref', required=True, help='原厂 / 上一版 skyward 镜像')
    ap.add_argument('--out', default='work', help='工作目录')
    a = ap.parse_args()

    os.makedirs(a.out, exist_ok=True)
    with open(a.ref, 'rb') as f:
        head, kernel_size, ramdisk_size = read_header(f)
        cpio = decompress(f, ramdisk_size)
    print(f'{os.path.basename(a.ref)}: kernel {kernel_size} B, ramdisk {ramdisk_size} B -> cpio {len(cpio)} B')

    with open(os.path.join(a.out, 'head.bin'), 'wb') as w:
        w.write(head)
    cpio_path = os.path.join(a.out, 'ramdisk.cpio')
    with open(cpio_path, 'wb') as w:
        w.write(cpio)

    tree = os.path.join(a.out, 'ramdisk')
    os.makedirs(tree, exist_ok=True)
    entries = parse_cpio(cpio)
    n = extract(entries, tree)
    print(f'cpio 条目 {len(entries)}，解出文件 {n} 个 -> {tree}')


if __name__ == '__main__':
    main()
