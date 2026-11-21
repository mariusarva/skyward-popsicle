#!/usr/bin/env python3
"""把 recovery-hal.zip 里的补丁层注入原厂 recovery 的 ramdisk。

    python scripts/augment_stock.py --stock stock/recovery.img \
        --hal resources/popsicle/recovery-hal.zip --out stock/augmented.img

为什么要这一步：官方 recovery 的 ramdisk 里没有 `sbin/` 那套 HAL
（qseecomd / keymint / gatekeeper / weaver / secure_element / se_omapi /
boot-service）、没有 `system/lib64` 里的配套库、也没有 CS40L26 的 DSP 固件。
缺了它们，skyward 拿不到解密的钥匙（/data 打不开），震动马达也起不来。
本仓库的 v6–v12 之所以能用，是因为它们套在带这些文件的 TWRP 容器里；
CI 从原厂镜像出发就必须自己补回来。

补丁层只做「补」，不覆盖原厂已有条目：原厂哪天自带了，就自动让位。
镜像总大小、boot 头、尾部 AVB 页脚原样保留（打包器随后会重算 AVB）。
"""
import argparse
import io
import struct
import sys
import zipfile

try:
    import lz4.block
except ImportError:  # pragma: no cover
    sys.exit('需要 lz4 模块：pip install lz4')

LZ4_LEGACY_MAGIC = 0x184C2102
BLOCK = 8 << 20
PAGE = 4096
# 原厂没有、但 [haptics] 指向的那个文件：马达谐振频率（0.1 Hz）
RESONANCE = (b'skyward/resonance', 0o100644, b'1724\n')
# 原厂 prop.default 只写了 ro.virtual_ab.compression.threads=true 和
# batch_writes=true，没写这两个线程数：libsnapshot 的 copy-on-write 写入
# 就只有一个线程，一个包八点几个 GB 全排在它后面。真机实测 8.57 GB 用
# 424 s（20.7 MB/s），而同一块盘上每次 fsync 的 144 KB 写入能跑 1.4 GB/s，
# 也就是盘一点都不忙（2026-11-21）。
KNOBS = ('ro.virtual_ab.num_worker_threads', 'ro.virtual_ab.num_verify_threads')


def decode_ramdisk(buf):
    """解 lz4 legacy：magic + 若干 [u32 长度][块]。"""
    out = bytearray()
    i = 4
    while i + 4 <= len(buf):
        (n,) = struct.unpack('<I', buf[i:i + 4])
        i += 4
        if n == 0:
            break
        n &= 0x7FFFFFFF
        out += lz4.block.decompress(buf[i:i + n], uncompressed_size=BLOCK)
        i += n
    return bytes(out)


def encode_ramdisk(data):
    out = bytearray(struct.pack('<I', LZ4_LEGACY_MAGIC))
    for off in range(0, len(data), BLOCK):
        block = lz4.block.compress(data[off:off + BLOCK], mode='high_compression',
                                   compression=12, store_size=False)
        out += struct.pack('<I', len(block))
        out += block
    return bytes(out)


def parse_cpio(blob):
    """保持原顺序解析 newc，返回 [(name, mode, data)]。"""
    entries, pos = [], 0
    while pos + 6 <= len(blob):
        if blob[pos:pos + 6] not in (b'070701', b'070702'):
            break
        f = [int(blob[pos + 6 + i * 8:pos + 14 + i * 8], 16) for i in range(13)]
        mode, size, namesize = f[1], f[6], f[11]
        pos += 110
        name = blob[pos:pos + namesize - 1]
        pos = (pos + namesize + 3) & ~3
        data = blob[pos:pos + size]
        pos = (pos + size + 3) & ~3
        if name == b'TRAILER!!!':
            break
        entries.append((name, mode, data))
    return entries


def write_cpio(entries):
    out = bytearray()
    for ino, (name, mode, data) in enumerate(entries):
        namesize = len(name) + 1
        hdr = ('070701' + '%08X' * 13 % (
            ino + 300000, mode, 0, 0, 1, 0, len(data),
            0, 0, 0, 0, namesize, 0)).encode()
        out += hdr + name + b'\0'
        out += b'\0' * ((-len(out)) % 4)
        out += data
        out += b'\0' * ((-len(out)) % 4)
    name = b'TRAILER!!!'
    out += ('070701' + '%08X' * 13 % (999999, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, len(name) + 1, 0)).encode()
    out += name + b'\0'
    out += b'\0' * ((-len(out)) % 4)
    return bytes(out)


def parent_dirs(name):
    parts = name.split(b'/')[:-1]
    return [b'/'.join(parts[:i + 1]) for i in range(len(parts))]


def patch_props(entries, threads):
    """给 prop.default 补上 libsnapshot 的线程数，已有的一个不改。"""
    for at, (name, mode, data) in enumerate(entries):
        if name != b'prop.default':
            continue
        text = data.decode('utf-8')
        missing = [k for k in KNOBS if f'{k}=' not in text]
        if not missing:
            print(f'prop.default 已写 {len(KNOBS)} 个线程数，跳过')
            return
        if not text.endswith('\n'):
            text += '\n'
        text += (
            '\n'
            '# Skyward：libsnapshot 的 copy-on-write 写入与校验线程数。原厂只写了\n'
            '# compression.threads=true，没有这两个键，写法就落回一个线程。\n'
        )
        for k in missing:
            text += f'{k}={threads}\n'
        entries[at] = (name, mode, text.encode('utf-8'))
        print('prop.default 补上 ' + '、'.join(f'{k}={threads}' for k in missing))
        return
    print('警告：ramdisk 里没有 prop.default，线程数没补上')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--stock', required=True, help='原厂 recovery 分区镜像（一整块，尾部带 AVB 页脚）')
    ap.add_argument('--hal', required=True, help='补丁层 zip（resources/popsicle/recovery-hal.zip）')
    ap.add_argument('--out', required=True)
    ap.add_argument('--list', action='store_true', help='只打印会补哪些文件')
    ap.add_argument('--threads', type=int, default=6,
                    help='prop.default 里 libsnapshot 的写入/校验线程数（默认 6，八核手机留两个给读包）')
    a = ap.parse_args()

    image = open(a.stock, 'rb').read()
    if image[:8] != b'ANDROID!':
        sys.exit(f'{a.stock} 不是 Android boot 镜像')
    kernel, ramdisk, _, header = struct.unpack('<IIII', image[8:24])
    off = ((header + PAGE - 1) // PAGE) * PAGE + ((kernel + PAGE - 1) // PAGE) * PAGE
    blob = decode_ramdisk(image[off:off + ramdisk])
    entries = parse_cpio(blob)
    have = {n for n, _, _ in entries}
    print(f'原厂 ramdisk：{ramdisk} 字节压缩，解出 {len(blob)} 字节，{len(entries)} 个条目')
    patch_props(entries, a.threads)

    with zipfile.ZipFile(a.hal) as z:
        add = [(i.filename.encode(), (i.external_attr >> 16) & 0o777, z.read(i))
               for i in z.infolist()]
    if a.list:
        for name, mode, data in add:
            print(f'  {oct(mode)} {len(data):>9} {name.decode()}')
        return

    added, skipped = 0, 0
    tail = [RESONANCE]
    for name, mode, data in add + tail:
        if name in have:
            skipped += 1
            continue
        for d in parent_dirs(name):
            if d not in have:
                entries.append((d, 0o40755, b''))
                have.add(d)
        entries.append((name, 0o100000 | mode, data))
        have.add(name)
        added += 1

    cpio = write_cpio(entries)
    packed = encode_ramdisk(cpio)
    print(f'补入 {added} 个条目（原厂已有 {skipped} 个，跳过）；'
          f'新 ramdisk {len(packed)} 字节压缩 / {len(cpio)} 字节原样')

    head = bytearray(image[:header])
    head[12:16] = struct.pack('<I', len(packed))
    out = bytearray(image[:off])       # 头 + 内核 + 对齐填充
    out[0:header] = head
    out += packed
    # 余量清零，尾部 64 字节的 AVB 页脚原样保留（打包器随后整块重算）
    if len(out) > len(image) - 64:
        sys.exit(f'补丁层放不下：需要 {len(out)} 字节，分区只有 {len(image)} 字节')
    out += b'\0' * (len(image) - 64 - len(out))
    out += image[-64:]
    open(a.out, 'wb').write(bytes(out))
    print(f'写出 {a.out}（{len(out)} 字节，与原镜像同尺寸）')


if __name__ == '__main__':
    main()
