#!/usr/bin/env python3
"""给重打包好的镜像补 AVB：vbmeta 块 + 尾部 footer（字段大端）。

    python scripts/patch_avb.py --ref stock.img --img output/v13.img

原理：boot 分区在 Android 上是 AVB 链上的一环，原厂镜像末尾带 vbmeta 与
footer，开机时 bootloader 会用 footer 里记录的偏移去读 vbmeta，校验
[0, vbmeta_offset) 的 sha256。我们把 ramdisk 改大了，vbmeta 与 footer
就必须整体后移并重算摘要。

脚本从 `--ref`（原厂镜像）里抄一份 vbmeta/footer 的模板，只改三件事：
  * vbmeta 里 vbmeta_offset 字段（272..280，大端）
  * vbmeta 里 hash 描述符的摘要（396..428）
  * footer 里 vbmeta_offset / 原数据大小（12..36，大端）
写完后自检：偏移、摘要、分区大小三项都要对得上。

注意：这是把原厂签名块原样搬移的做法。若 bootloader 处于锁定的严格校验
状态，仍需关闭校验（fastboot flashing unlock / avbctl disable-verification）；
skyward 自己写回 boot 时用的是同一套逻辑（见 crates/skyward-pack）。
"""
import argparse
import hashlib
import os
import struct
import sys

PART = 104857600


def read_template(ref, part_size):
    with open(ref, 'rb') as f:
        head = f.read(4096)
        if head[:8] != b'ANDROID!':
            sys.exit('--ref 不是 Android boot image')
        (ramdisk_size,) = struct.unpack('<I', head[12:16])
        vb_off = (4096 + ramdisk_size + 4095) & ~4095
        f.seek(vb_off)
        vb = f.read(512)
        f.seek(part_size - 64)
        foot = f.read(64)
    if vb[:4] != b'AVB0' or foot[:4] != b'AVBf':
        sys.exit(f'--ref 在 {vb_off} / 尾部没有 AVB 结构（vbmeta={vb[:4]!r}, footer={foot[:4]!r}）')
    return bytearray(vb), bytearray(foot)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ref', required=True, help='原厂镜像：抄 vbmeta / footer 模板')
    ap.add_argument('--img', required=True, help='repack_image.py 产出的镜像')
    ap.add_argument('--part-size', type=int, default=PART)
    a = ap.parse_args()

    vb, foot = read_template(a.ref, a.part_size)
    with open(a.img, 'r+b') as f:
        (ramdisk_size,) = struct.unpack('<I', f.read(16)[12:16])
        img_end = 4096 + ramdisk_size
        vb_off = (img_end + 4095) & ~4095
        print(f'{os.path.basename(a.img)}: ramdisk {ramdisk_size} B, vbmeta@{vb_off}')

        h = hashlib.sha256()
        f.seek(0)
        left = vb_off
        while left:
            chunk = f.read(min(1 << 24, left))
            if not chunk:
                sys.exit('读镜像时提前结束')
            h.update(chunk)
            left -= len(chunk)
        digest = h.digest()

        vb[272:280] = struct.pack('>Q', vb_off)
        vb[396:428] = digest
        foot[12:20] = struct.pack('>Q', vb_off)
        foot[20:28] = struct.pack('>Q', vb_off)
        foot[28:36] = struct.pack('>Q', 512)

        f.seek(vb_off)
        f.write(vb)
        f.seek(a.part_size - 64)
        f.write(foot)

    # 自检
    with open(a.img, 'rb') as f:
        f.seek(vb_off)
        v = f.read(512)
        assert v[:4] == b'AVB0' and struct.unpack('>Q', v[272:280])[0] == vb_off, 'vbmeta 偏移没写对'
        assert v[396:428] == digest, 'vbmeta 摘要没写对'
        f.seek(a.part_size - 64)
        ft = f.read(64)
        assert ft[:4] == b'AVBf' and struct.unpack('>Q', ft[20:28])[0] == vb_off, 'footer 没写对'
        f.seek(0, os.SEEK_END)
        assert f.tell() == a.part_size, f'镜像大小 {f.tell()} != {a.part_size}'
    with open(a.img, 'rb') as f:
        print('  AVB OK, sha256:', hashlib.sha256(f.read()).hexdigest())


if __name__ == '__main__':
    main()
