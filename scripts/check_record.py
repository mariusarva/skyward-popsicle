"""检查一段录屏的容器能不能放：MP4（H.264）与旧的 AVI（MJPEG）都按字段读一遍。

用法：python scripts/check_record.py <文件> [<文件> ...]

不看画面，只看结构。MP4：顶层三个盒子首尾相接、`stsz` 合计等于 `mdat` 身子、
`stsc` + `stco` 走完盖住每个 sample、每个 sample 都是一串「4 字节长度 + NAL」且
正好走完、`stsd` 里是 `avc1` 且 `avcC` 里 SPS/PPS 都在、`stss` 记的正好是真
IDR 的那几帧。AVI（v104 及更早）：四个长度字段、`idx1` 条数、每条索引指到
`00dc` + 一张 SOI/EOI 完整的 JPEG。
"""

import struct
import sys
from pathlib import Path


def fourcc(at: int, data: bytes) -> str:
    return data[at:at + 4].decode("latin-1")


def jpeg_ok(buf: bytes) -> bool:
    return buf[:2] == b"\xff\xd8" and buf[-2:] == b"\xff\xd9"


def nals(buf: bytes) -> list[int] | None:
    """AVCC 字节流：4 字节大端长度 + NAL 反复，正好消耗完就是对的。
    返回每个 NAL 的类型；走不齐就 None。"""
    i, n, types = 0, len(buf), []
    while i < n:
        if i + 4 > n:
            return None
        (ln,) = struct.unpack_from(">I", buf, i)
        i += 4
        if ln == 0 or i + ln > n:
            return None
        types.append(buf[i] & 0x1F)
        i += ln
    return types


def check_avi(data: bytes) -> list[str]:
    bad: list[str] = []
    if fourcc(0, data) != "RIFF" or fourcc(8, data) != "AVI ":
        return ["不是 RIFF AVI"]
    riff = struct.unpack_from("<I", data, 4)[0]
    if riff + 8 != len(data):
        bad.append(f"RIFF 长度 {riff + 8} 与文件 {len(data)} 不一致")
    # LIST hdrl 的起点固定是 12
    if fourcc(12, data) != "LIST" or fourcc(20, data) != "hdrl":
        bad.append("12 处不是 LIST hdrl")
    hdrl_len = struct.unpack_from("<I", data, 16)[0]
    movi_list = 12 + 8 + hdrl_len
    if fourcc(movi_list, data) != "LIST" or fourcc(movi_list + 8, data) != "movi":
        bad.append("hdrl 之后不是 LIST movi")
        return bad
    movi_len = struct.unpack_from("<I", data, movi_list + 4)[0]
    movi_at = movi_list + 8
    index_at = movi_list + 8 + movi_len
    if fourcc(index_at, data) != "idx1":
        bad.append(f"movi 之后（{index_at}）不是 idx1，而是 {fourcc(index_at, data)!r}")

    avih = data.find(b"avih", 12, movi_list)
    frames = struct.unpack_from("<I", data, avih + 8 + 16)[0]
    strh = data.find(b"strh", 12, movi_list)
    length = struct.unpack_from("<I", data, strh + 8 + 32)[0]
    if frames != length:
        bad.append(f"avih 帧数 {frames} 与 strh 长度 {length} 不一致")

    n = struct.unpack_from("<I", data, index_at + 4)[0] // 16
    if n != frames:
        bad.append(f"idx1 {n} 条与头里 {frames} 帧不一致")
    for i in range(n):
        at = index_at + 8 + i * 16
        tag = fourcc(at, data)
        offset, size = struct.unpack_from("<II", data, at + 8)
        chunk = movi_at + offset
        if tag != "00dc":
            bad.append(f"第 {i} 条索引的标记是 {tag!r}")
            break
        if fourcc(chunk, data) != "00dc":
            bad.append(f"第 {i} 条索引指向 {chunk}，那里不是 00dc")
            break
        clen = struct.unpack_from("<I", data, chunk + 4)[0]
        if clen != size:
            bad.append(f"第 {i} 帧长度 {clen} 与索引 {size} 不一致")
            break
        if not jpeg_ok(data[chunk + 8:chunk + 8 + size]):
            bad.append(f"第 {i} 帧不是完整的 JPEG")
            break
    print(f"  AVI：{frames} 帧 / {n} 条索引 / movi {movi_len} B / 文件 {len(data)} B")
    return bad


def boxes(data: bytes, start: int, end: int):
    at = start
    while at + 8 <= end:
        size = struct.unpack_from(">I", data, at)[0]
        name = fourcc(at + 4, data)
        if size == 0:
            size = end - at
        if size < 8:
            break
        yield name, at, size
        at += size


def find(data: bytes, start: int, end: int, path: list[str]):
    """按容器路径找盒子，返回 (起点, 大小)。"""
    at, size = start, end - start
    for want in path:
        hit = None
        for name, b_at, b_size in boxes(data, at, at + size):
            if name == want:
                hit = (b_at, b_size)
                break
        if hit is None:
            return None
        at, size = hit[0] + 8, hit[1] - 8
    return at, size


def check_avcc(data: bytes, avcc_at: int) -> tuple[list[str], int, int]:
    """读 avcC，返回 (问题, SPS 数, PPS 数)。"""
    bad: list[str] = []
    body = avcc_at
    version = data[body]
    if version != 1:
        bad.append(f"avcC 版本是 {version}")
    sps_at = body + 5
    n_sps = data[sps_at] & 0x1F
    at = sps_at + 1
    for _ in range(n_sps):
        ln = struct.unpack_from(">H", data, at)[0]
        at += 2
        if data[at] & 0x1F != 7:
            bad.append("avcC 里的 SPS 不是 NAL 7")
        at += ln
    n_pps = data[at]
    at += 1
    for _ in range(n_pps):
        ln = struct.unpack_from(">H", data, at)[0]
        at += 2
        if data[at] & 0x1F != 8:
            bad.append("avcC 里的 PPS 不是 NAL 8")
        at += ln
    if n_sps == 0 or n_pps == 0:
        bad.append(f"avcC 里 SPS {n_sps} 个 / PPS {n_pps} 个")
    return bad, n_sps, n_pps


def check_mp4(data: bytes) -> list[str]:
    bad: list[str] = []
    top = [(n, a, s) for n, a, s in boxes(data, 0, len(data))]
    names = [n for n, _, _ in top]
    if names[:3] != ["ftyp", "mdat", "moov"]:
        return [f"顶层盒子是 {names}"]
    mdat_at, mdat_size = top[1][1], top[1][2]
    moov_at, moov_size = top[2][1], top[2][2]
    if mdat_at + mdat_size != moov_at:
        bad.append("mdat 与 moov 之间有缝")
    if moov_at + moov_size != len(data):
        bad.append("moov 之后还有字节")

    root = (moov_at + 8, moov_at + moov_size)
    stbl = ["trak", "mdia", "minf", "stbl"]
    stsz = find(data, root[0], root[1], stbl + ["stsz"])
    stco = find(data, root[0], root[1], stbl + ["stco"])
    stts = find(data, root[0], root[1], stbl + ["stts"])
    stsc = find(data, root[0], root[1], stbl + ["stsc"])
    stsd = find(data, root[0], root[1], stbl + ["stsd"])
    if None in (stsz, stco, stts, stsc, stsd):
        return bad + ["moov 里缺 stsd / stsz / stco / stts / stsc"]

    # stsd：find 给的是盒子身子（版本+flags 4 字节、entry_count 4 字节），
    # 之后才是第一条 sample entry。VisualSampleEntry 的头 78 字节，宽高在 +32。
    entry = stsd[0] + 8
    fmt = fourcc(entry + 4, data)
    if fmt != "avc1":
        bad.append(f"stsd 里的格式是 {fmt!r}，不是 avc1")
    if struct.unpack_from(">H", data, entry + 32)[0] == 0:
        bad.append("sample entry 的宽高是 0")
    avcc = find(data, entry + 86, entry + struct.unpack_from(">I", data, entry)[0], ["avcC"])
    n_sps = n_pps = 0
    if avcc is None:
        bad.append("sample entry 里没有 avcC")
    else:
        more, n_sps, n_pps = check_avcc(data, avcc[0])
        bad += more

    sample_size, count = struct.unpack_from(">II", data, stsz[0] + 4)
    sizes = list(struct.unpack_from(f">{count}I", data, stsz[0] + 12)) if sample_size == 0 else [sample_size] * count
    entries = struct.unpack_from(">I", data, stco[0] + 4)[0]
    chunks = list(struct.unpack_from(f">{entries}I", data, stco[0] + 8))
    delta_count = struct.unpack_from(">I", data, stts[0] + 4)[0]
    total = 0
    # 每种格子有几个 sample：恒定帧率的文件只有一种（90000/fps），编码迟到
    # 的那几格是它的两倍或三倍。真机上「设 30 帧却是 17 帧」这个 bug 就是
    # 靠这一行看出来的，所以一直打出来（2026-11-21）。
    ticks: dict[int, int] = {}
    for i in range(delta_count):
        n, d = struct.unpack_from(">II", data, stts[0] + 8 + i * 8)
        total += n * d
        ticks[d] = ticks.get(d, 0) + n
    spread = " ".join(f"{d}×{n}" for d, n in sorted(ticks.items(), key=lambda kv: -kv[1])[:4])

    if sum(sizes) != mdat_size - 8:
        bad.append(f"sample 合计 {sum(sizes)} 与 mdat 身子 {mdat_size - 8} 不一致")

    # 按 stsc 的排布走一遍：一秒一个 chunk
    runs = struct.unpack_from(">I", data, stsc[0] + 4)[0]
    first, per = [], []
    for i in range(runs):
        a, b, _ = struct.unpack_from(">III", data, stsc[0] + 8 + i * 12)
        first.append(a)
        per.append(b)
    seen = 0
    idr: list[int] = []
    for c in range(len(chunks)):
        take = per[0]
        for i, a in enumerate(first):
            if c + 1 >= a:
                take = per[i]
        take = min(take, count - seen)
        at = chunks[c]
        for k in range(take):
            size = sizes[seen + k]
            types = nals(data[at:at + size])
            if types is None or not types:
                bad.append(f"sample {seen + k} 不是完整的 AVCC（长度串走不齐）")
                break
            if 5 in types:
                idr.append(seen + k + 1)
            at += size
        seen += take
    if seen != count:
        bad.append(f"chunk 走完只盖到 {seen} 个 sample，共 {count}")

    # stss：有它就要求「记的正好是 IDR 那几帧」；全都 IDR 时本仓库不写 stss
    stss = find(data, root[0], root[1], stbl + ["stss"])
    if stss is not None:
        n = struct.unpack_from(">I", data, stss[0] + 4)[0]
        listed = list(struct.unpack_from(f">{n}I", data, stss[0] + 8))
        if listed != idr:
            bad.append(f"stss 记了 {len(listed)} 帧，实际 IDR {len(idr)} 帧（{listed[:4]} vs {idr[:4]}）")
    elif len(idr) != count:
        bad.append(f"没有 stss，但只有 {len(idr)}/{count} 帧是 IDR")

    if not idr or idr[0] != 1:
        bad.append("第一帧不是 IDR")

    print(f"  MP4：{count} 个 sample / {len(chunks)} 个 chunk / mdat {mdat_size} B / "
          f"stts 共 {total} tick（{spread}）/ {fmt} + avcC（SPS {n_sps} / PPS {n_pps}） / "
          f"IDR {len(idr)} 帧 / 文件 {len(data)} B")
    return bad


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__)
        return 2
    worst = 0
    for name in argv:
        path = Path(name)
        data = path.read_bytes()
        print(f"{path.name}  {len(data)} B")
        if path.suffix.lower() == ".avi":
            bad = check_avi(data)
        elif path.suffix.lower() == ".mp4":
            bad = check_mp4(data)
        else:
            bad = [f"不认识的容器 {path.suffix}"]
        for line in bad:
            print(f"  ✗ {line}")
        if bad:
            worst = 1
        else:
            print("  ✓ 结构自洽")
    return worst


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
