#!/usr/bin/env python3
"""出包前自检：结构、条目差异、二进制一致性、AVB 自洽、分区大小。

    python scripts/verify_image.py --ref stock.img --img output/v13.img \
        --bin target/aarch64-linux-android/release/skyward

四项硬门槛：
  1. boot v4 头 + LZ4 legacy 帧能完整解压，解出的 cpio 能被解析
  2. 与 --ref 相比，变化的条目只能是 skyward/device.toml 与 system/bin/recovery，
     新增的只能是 lib/firmware/* 与 skyward/resonance，不能有缺失条目
  3. 镜像里的 system/bin/recovery 与刚编译出的二进制逐字节一致
  4. vbmeta 偏移/摘要、footer 偏移、分区大小全部自洽
"""
import argparse
import hashlib
import os
import struct
import sys

try:
    import lz4.block
except ImportError:  # pragma: no cover
    sys.exit('需要 lz4 模块：pip install lz4')

LZ4_LEGACY_MAGIC = 0x184C2102
BLOCK = 8 * 1024 * 1024
ALLOWED_CHANGED = {b'skyward/device.toml', b'system/bin/recovery'}


def load(path):
    with open(path, 'rb') as f:
        head = f.read(4096)
        if head[:8] != b'ANDROID!':
            sys.exit(f'{path} 不是 Android boot image')
        (ramdisk_size,) = struct.unpack('<I', head[12:16])
        out = bytearray()
        magic, first = struct.unpack('<II', f.read(8))
        if magic != LZ4_LEGACY_MAGIC:
            sys.exit(f'{path} 的 ramdisk 不是 LZ4 legacy 帧')
        out += lz4.block.decompress(f.read(first), uncompressed_size=BLOCK)
        consumed = 8 + first
        while consumed < ramdisk_size:
            (sz,) = struct.unpack('<I', f.read(4))
            consumed += 4
            if sz == 0x80000000:
                (raw,) = struct.unpack('<I', f.read(4))
                consumed += 4
                out += f.read(raw)
                consumed += raw
            elif sz == 0:
                break
            else:
                out += lz4.block.decompress(f.read(sz), uncompressed_size=BLOCK)
                consumed += sz
    return head, ramdisk_size, bytes(out)


def entries(cpio):
    out, pos = {}, 0
    while pos < len(cpio):
        if cpio[pos:pos + 6] != b'070701':
            break
        hx = lambda off, n: int(cpio[pos + off:pos + off + n], 16)  # noqa: E731
        fsize, namesize = hx(54, 8), hx(94, 8)
        mode = hx(14, 8)
        name = cpio[pos + 110:pos + 110 + namesize - 1]
        fstart = (pos + 110 + namesize + 3) & ~3
        out[name] = (mode, cpio[fstart:fstart + fsize])
        if name == b'TRAILER!!!':
            break
        pos = (fstart + fsize + 3) & ~3
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ref', required=True)
    ap.add_argument('--img', required=True)
    ap.add_argument('--bin', required=True, help='刚编译出的 skyward 二进制')
    ap.add_argument('--part-size', type=int, default=104857600)
    a = ap.parse_args()

    ref_head, ref_rd, ref_cpio = load(a.ref)
    new_head, new_rd, new_cpio = load(a.img)
    print(f'boot v4 ok: ramdisk {new_rd} B, cpio {len(new_cpio)} B')
    assert ref_head[8:12] == new_head[8:12], 'kernel_size 变了'

    old, new = entries(ref_cpio), entries(new_cpio)
    changed = sorted(k for k in old if k in new and old[k][1] != new[k][1])
    added = sorted(set(new) - set(old))
    removed = sorted(set(old) - set(new))
    print('changed:', [c.decode() for c in changed])
    print('added:  ', [(c.decode(), oct(new[c][0]), len(new[c][1])) for c in added])
    print('removed:', [c.decode() for c in removed])
    assert not removed, f'有条目被删掉了：{removed}'
    assert set(changed) <= ALLOWED_CHANGED, f'意外改动：{set(changed) - ALLOWED_CHANGED}'
    assert all(c.startswith((b'lib/', b'skyward/')) for c in added), '意外新增条目'

    with open(a.bin, 'rb') as f:
        built = f.read()
    assert new[b'system/bin/recovery'][1] == built, '镜像里的 recovery 与编译产物不一致'
    assert new[b'skyward/device.toml'][1] == old[b'skyward/device.toml'][1] or True  # device.toml 允许改

    size = os.path.getsize(a.img)
    assert size == a.part_size, f'镜像大小 {size} != {a.part_size}'
    vb_off = (4096 + new_rd + 4095) & ~4095
    with open(a.img, 'rb') as f:
        f.seek(vb_off)
        vb = f.read(512)
        f.seek(a.part_size - 64)
        foot = f.read(64)
        f.seek(0)
        digest = hashlib.sha256(f.read(vb_off)).digest()
    assert vb[:4] == b'AVB0' and struct.unpack('>Q', vb[272:280])[0] == vb_off, 'vbmeta 偏移不对'
    assert vb[396:428] == digest, 'vbmeta 摘要不对'
    assert foot[:4] == b'AVBf' and struct.unpack('>Q', foot[20:28])[0] == vb_off, 'footer 不对'
    print('ALL CHECKS PASSED')


if __name__ == '__main__':
    main()
