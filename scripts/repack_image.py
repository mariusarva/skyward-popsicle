#!/usr/bin/env python3
"""把新的 skyward 二进制与 device.toml 塞回 ramdisk，重新压成 boot 镜像。

    python scripts/repack_image.py --ref stock.img --cpio work/ramdisk.cpio \
        --tree work/ramdisk --fw work/fw --out output/v13.img

做法：以 `work/ramdisk.cpio`（原镜像解出来的那份）为骨架逐条复制，
只替换 MANAGED 列表里的条目，再追加 cs40l26 固件与 resonance。
这样 cpio 里其它条目的顺序、inode、对齐全部保持原样——recovery 的 ramdisk
一旦顺序或对齐变了，个别机型会启动失败。

压缩用 LZ4 legacy（magic 0x184C2102），每块 8 MiB，与官方镜像一致。
镜像尾部按 `--part-size` 全零补齐；vbmeta/footer 交给 patch_avb.py。
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
# 需要从 work/ramdisk 里取新版本覆盖的 cpio 条目
MANAGED = [b'skyward/device.toml', b'system/bin/recovery']
# 每次都要追加的条目：(cpio 内路径, 权限, 内容)
EXTRA = [(b'skyward/resonance', 0o100644, b'1724\n')]


def newc_hdr(ino, mode, uid, gid, nlink, mtime, fsize, namesize):
    return ('070701' + '%08X' * 13 % (ino, mode, uid, gid, nlink, mtime, fsize,
                                      0, 0, 0, 0, namesize, 0)).encode()


def parse_cpio(data):
    entries, pos, trailer_end = [], 0, None
    while pos < len(data):
        if data[pos:pos + 6] != b'070701':
            break
        hx = lambda off, n: int(data[pos + off:pos + off + n], 16)  # noqa: E731
        hdr = data[pos:pos + 110]
        fsize, namesize = hx(54, 8), hx(94, 8)
        name = data[pos + 110:pos + 110 + namesize - 1]
        fstart = (pos + 110 + namesize + 3) & ~3
        fend = fstart + fsize
        entries.append((hdr, name, data[fstart:fend]))
        if name == b'TRAILER!!!':
            trailer_end = (fend + 3) & ~3
            break
        pos = (fend + 3) & ~3
    if trailer_end is None:
        sys.exit('ramdisk.cpio 里没有 TRAILER!!!，文件不完整')
    return entries, data[trailer_end:]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ref', required=True, help='原镜像：只取前 4096 字节的 boot 头')
    ap.add_argument('--cpio', required=True, help='解包得到的原始 cpio')
    ap.add_argument('--tree', required=True, help='已放好新 device.toml / recovery 的 ramdisk 目录')
    ap.add_argument('--out', required=True)
    ap.add_argument('--fw', default=None, help='cs40l26 固件目录，省略则不打固件')
    ap.add_argument('--part-size', type=int, default=104857600, help='分区大小，默认 100 MiB')
    a = ap.parse_args()

    def tree_bytes(name):
        with open(os.path.join(a.tree, *name.decode().split('/')), 'rb') as f:
            return f.read()

    entries, trailing = parse_cpio(open(a.cpio, 'rb').read())
    print(f'骨架条目 {len(entries)}，尾部附加 {len(trailing)} B')

    replace = {name: tree_bytes(name) for name in MANAGED}
    for k, v in replace.items():
        print(f'  替换 {k.decode()}: {len(v)} B')

    added = [(b'lib', 0o040755, b''), (b'lib/firmware', 0o040755, b'')]
    if a.fw:
        for fn in sorted(os.listdir(a.fw)):
            path = os.path.join(a.fw, fn)
            if os.path.isfile(path):
                added.append((('lib/firmware/' + fn).encode(), 0o100644, open(path, 'rb').read()))
        print(f'  追加固件 {len(added) - 2} 个')
    added += EXTRA

    out = bytearray()
    ino, replaced = 0x51EA0001, 0
    for hdr, name, body in entries:
        if name in replace:
            body = replace[name]
            hdr = hdr[:54] + ('%08X' % len(body)).encode() + hdr[62:]
            replaced += 1
        if name == b'TRAILER!!!':
            for n, mode, content in added:
                nm = n + b'\x00'
                out += newc_hdr(ino, mode, 0, 0, 1, 0, len(content), len(nm)) + nm
                out += b'\x00' * ((-len(out)) % 4)
                out += content
                out += b'\x00' * ((-len(out)) % 4)
                ino += 1
        out += hdr + name + b'\x00'
        out += b'\x00' * ((-len(out)) % 4)
        out += body
        out += b'\x00' * ((-len(out)) % 4)
    if replaced != len(replace):
        sys.exit(f'只替换了 {replaced} 条，期望 {len(replace)} 条——ramdisk 结构不对')
    out += trailing

    comp = bytearray()
    comp += struct.pack('<I', LZ4_LEGACY_MAGIC)
    for off in range(0, len(out), BLOCK):
        chunk = lz4.block.compress(bytes(out[off:off + BLOCK]), mode='high_compression',
                                   compression=9, store_size=False)
        comp += struct.pack('<I', len(c)) + c

    with open(a.ref, 'rb') as f:
        head = bytearray(f.read(4096))
    if head[:8] != b'ANDROID!':
        sys.exit('--ref 不是 Android boot image')
    head[12:16] = struct.pack('<I', len(comp))

    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    with open(a.out, 'wb') as w:
        w.write(head)
        w.write(comp)
        pad = a.part_size - 4096 - len(comp)
        if pad < 0:
            sys.exit(f'ramdisk 太大，超出分区 {-pad} B')
        w.write(b'\x00' * pad)
    print(f'cpio {len(out)} B, lz4 {len(comp)} B -> {a.out}')


if __name__ == '__main__':
    main()
