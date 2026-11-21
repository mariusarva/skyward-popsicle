#!/usr/bin/env python3
"""出包后的硬门槛自检：结构、关键条目、AVB 自洽、分区大小。

    python scripts/check_build.py --img out/skyward-xxxx.img

跑在 CI 的打包步骤之后。任何一项不过就让构建红掉，免得又交出一个
「能开机但解不开 /data」的镜像——原厂 ramdisk 里本来就没有那套 HAL，
补丁层哪次失效了，这里必须拦住。

四项：
  1. Android boot v4 头 + LZ4 legacy 帧能完整解压，cpio 能解析
  2. [device] 声明的服务程序、cs40l26 固件、skyward/resonance、device.toml 都在
  3. vbmeta 偏移与摘要、footer 偏移一致（能过 bootloader 的 AVB 校验）
  4. 镜像总长等于分区大小
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
BLOCK = 8 << 20
# 补丁层必须落在镜像里的东西：缺任何一项，/data 打不开或马达不转
REQUIRED = [
    'sbin/qseecomd',
    'sbin/hlosminkdaemon',
    'sbin/android.hardware.security.onekeymint-service-qti',
    'sbin/android.hardware.gatekeeper-rust-service-qti',
    'sbin/android.hardware.weaver-service.nxp-qti',
    'sbin/android.hardware.secure_element-service.qti',
    'sbin/se_omapi',
    'sbin/android.hardware.boot-service.qti',
    'system/etc/vintf/manifest/se_omapi.xml',
    'system/lib64/android.hardware.weaver-V2-ndk.so',
    'lib/firmware/cs40l26.wmfw',
    'skyward/resonance',
    'skyward/device.toml',
    'skyward/system-init',
    'system/bin/recovery',
]


def ramdisk_entries(path):
    with open(path, 'rb') as f:
        head = f.read(4096)
        if head[:8] != b'ANDROID!':
            sys.exit(f'{path} 不是 Android boot image')
        kernel, size = struct.unpack('<II', head[8:16])
        f.seek(((len(head) + 4095) // 4096) * 4096 + ((kernel + 4095) // 4096) * 4096)
        blob = bytearray()
        magic, first = struct.unpack('<II', f.read(8))
        if magic != LZ4_LEGACY_MAGIC:
            sys.exit(f'{path} 的 ramdisk 不是 LZ4 legacy 帧（{hex(magic)}）')
        blob += lz4.block.decompress(f.read(first), uncompressed_size=BLOCK)
        used = 8 + first
        while used < size:
            (n,) = struct.unpack('<I', f.read(4))
            used += 4
            if n == 0:
                break
            n &= 0x7FFFFFFF
            blob += lz4.block.decompress(f.read(n), uncompressed_size=BLOCK)
            used += n
    names, pos = {}, 0
    while pos + 6 <= len(blob):
        if blob[pos:pos + 6] not in (b'070701', b'070702'):
            break
        f = [int(blob[pos + 6 + i * 8:pos + 14 + i * 8], 16) for i in range(13)]
        mode, size_, namesize = f[1], f[6], f[11]
        pos += 110
        name = blob[pos:pos + namesize - 1].decode('utf-8', 'replace')
        pos = (pos + namesize + 3) & ~3
        pos = (pos + size_ + 3) & ~3
        if name == 'TRAILER!!!':
            break
        names[name] = (mode, size_)
    return size, names


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--img', required=True)
    ap.add_argument('--part-size', type=int, default=104857600)
    ap.add_argument('--min-entries', type=int, default=1200,
                    help='ramdisk 条目下限；低于它说明补丁层没进去')
    a = ap.parse_args()

    total = os.path.getsize(a.img)
    assert total == a.part_size, f'镜像 {total} 字节，分区是 {a.part_size}'

    rd_size, entries = ramdisk_entries(a.img)
    print(f'ramdisk {rd_size} 字节压缩，{len(entries)} 个条目')
    assert len(entries) >= a.min_entries, f'只有 {len(entries)} 个条目，补丁层没进 ramdisk'

    missing = [n for n in REQUIRED if n not in entries]
    assert not missing, f'缺条目：{missing}'
    for n in REQUIRED:
        mode = entries[n][0]
        if n.startswith('sbin/') and (mode & 0o111) == 0:
            sys.exit(f'{n} 没有可执行位（{oct(mode)}）')
    print(f'关键条目齐全（{len(REQUIRED)} 项，sbin 下的都带可执行位）')

    # AVB：vbmeta 紧跟在 ramdisk 之后按页对齐，摘要覆盖它前面的全部字节
    vb_off = ((((1584 + 4095) // 4096) * 4096) + rd_size + 4095) & ~4095
    with open(a.img, 'rb') as f:
        data = f.read()
    vb = data[vb_off:vb_off + 512]
    foot = data[-64:]
    assert vb[:4] == b'AVB0', f'vbmeta 魔数不对：{vb[:4]}'
    assert struct.unpack('>Q', vb[272:280])[0] == vb_off, 'vbmeta 记录的偏移不对'
    assert vb[396:428] == hashlib.sha256(data[:vb_off]).digest(), 'vbmeta 摘要不对'
    assert foot[:4] == b'AVBf', f'footer 魔数不对：{foot[:4]}'
    assert struct.unpack('>Q', foot[20:28])[0] == vb_off, 'footer 指向的 vbmeta 偏移不对'
    assert struct.unpack('>Q', foot[12:20])[0] == vb_off, 'footer 记录的原镜像长度不对'
    print(f'AVB 自洽：vbmeta @{vb_off}，摘要一致，footer 一致')
    print('ALL CHECKS PASSED')


if __name__ == '__main__':
    main()
