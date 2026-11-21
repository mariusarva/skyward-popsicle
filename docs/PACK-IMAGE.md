# 出包：从 boot 镜像到可刷的 skyward.img

popsicle 的 boot 分区是 **Android boot image v4**（`ANDROID!` 魔数，kernel 为空，只有 ramdisk），ramdisk 用 **LZ4 legacy 帧**（magic `0x184C2102`，每块 8 MiB），整分区固定 **100 MiB（104857600 B）**，末尾带 **AVB vbmeta + footer**。

出包不改 kernel，只换 ramdisk 里的两个条目、追加固件，然后重算压缩与 AVB。

## 全流程

```sh
# 0) 准备：参考镜像（原厂 skyward 或上一版成品）+ 新二进制 + 固件目录
#    固件来自本机 odm 分区：odm/firmware/cs40l26*.bin

# 1) 解包
python scripts/unpack_image.py --ref stock.img --out work
#    → work/ramdisk.cpio（骨架）、work/ramdisk/（目录树）、work/head.bin

# 2) 换二进制与配置
cp target/aarch64-linux-android/release/skyward work/ramdisk/system/bin/recovery
#    work/ramdisk/skyward/device.toml 按需改（[haptics] strength、[data] weaver_first 等）

# 3) 重打包（替换 MANAGED 条目 + 追加 lib/firmware/* 与 skyward/resonance）
python scripts/repack_image.py --ref stock.img --cpio work/ramdisk.cpio \
    --tree work/ramdisk --fw work/fw --out output/skyward-v13.img

# 4) 补 AVB
python scripts/patch_avb.py --ref stock.img --img output/skyward-v13.img

# 5) 自检（四项硬门槛）
python scripts/verify_image.py --ref stock.img --img output/skyward-v13.img \
    --bin target/aarch64-linux-android/release/skyward
```

`verify_image.py` 通过才会打印 `ALL CHECKS PASSED`，它检查：

1. boot v4 头与 LZ4 legacy 帧能完整解压、cpio 可解析；
2. 与参考镜像相比，改动只能是 `skyward/device.toml`、`system/bin/recovery`，新增只能是 `lib/*`、`skyward/*`，**不允许有条目缺失**；
3. 镜像里的 `system/bin/recovery` 与刚编译出的二进制**逐字节一致**；
4. vbmeta 偏移 / 摘要、footer 偏移、分区大小（100 MiB）自洽。

## 为什么必须用「骨架替换」而不是「整目录重新打 cpio」

recovery ramdisk 里条目的**顺序、inode、4 字节对齐**都参与校验与挂载顺序。整目录重新打包会把顺序打乱，个别机型直接起不来。`repack_image.py` 的做法是逐条复制原始 cpio，只替换 `MANAGED` 里列出的条目，其余原样保留（含 `TRAILER!!!` 之后可能存在的尾部填充）。

## AVB 是怎么补的

原厂镜像在 `align4096(4096 + ramdisk_size)` 处放 512 字节 vbmeta，分区最后 64 字节放 footer。改了 ramdisk 大小后：

- vbmeta 整体后移，`vbmeta_offset`（偏移 272，大端 u64）与新位置的**镜像前缀 sha256**（偏移 396，32 字节）一起重写；
- footer 里两处 `vbmeta_offset`（12 / 20）与 vbmeta 大小（28）重写。

`patch_avb.py` 从 `--ref` 里抄一份模板再改这三个字段，写完自检。bootloader 若处于严格校验状态，仍需 `fastboot flashing unlock` / 关闭 verification，这一点和原厂镜像没有区别。

## 刷入与回退

```sh
fastboot flash boot output/skyward-v13.img
```

- **不要**在 recovery 里点「检查更新」：那是 Skyward 官方自更新，会把自己的镜像写回 boot，本分支的改动全部丢失。
- 回退：把上一版 img 同样 `fastboot flash boot` 刷回；分区大小一致，无需其它操作。

## device.toml 里与本机相关的几项

```toml
[haptics]
resonance = "/skyward/resonance"   # 内容 "1724\n"，cs40l26 的谐振频率
strength  = { lightest = 0.45, strongest = 1.0 }

[data]
weaver_first = true                # 先进解密页，记住密码/过渡动画依赖它

[backup]
# efs / data 跳过……
```

## CI 出包：先补 HAL 层再打包

官方 recovery 的 ramdisk 里**没有** `sbin/` 那套 HAL——`qseecomd`、
`android.hardware.security.onekeymint-service-qti`、`gatekeeper-rust-service-qti`、
`android.hardware.weaver-service.nxp-qti`、`secure_element-service.qti`、`se_omapi`、
`android.hardware.boot-service.qti`、`hlosminkdaemon`，也没有配套的 `system/lib64`
库和 `system/etc/vintf/manifest/*.xml`。缺了它们，skyward 拿不到解密的钥匙，
`/data` 打不开；CS40L26 的 DSP 固件也不在里面，马达起不来。

v6–v12 能跑，是因为它们套在带这些文件的 TWRP 容器里。CI 从原厂镜像出发，
所以要先把这层补回去：

```sh
python scripts/augment_stock.py --stock stock/recovery.img \
    --hal resources/popsicle/recovery-hal.zip --out stock/base.img
```

`augment_stock.py` 只做「补」：解出原厂 ramdisk 的 cpio，把补丁层里原厂没有的
条目（外加 `skyward/resonance`，内容 `1724\n`）按原顺序追加，重新压成 LZ4 legacy，
镜像总尺寸与尾部 AVB 页脚保持原样。随后 `skyward-pack recovery --stock stock/base.img`
才动手替换 `system/bin/recovery`、写 `skyward/device.toml`、加字体、改 prop、重算 AVB。

补丁层来自本机 4.0.6.0 的 TWRP 容器，共 226 个文件 / 约 41 MiB，压缩后 13.9 MiB，
存放在 `resources/popsicle/recovery-hal.zip`（zip 的每个条目都带原始权限位）。
要看它包含什么：

```sh
python scripts/augment_stock.py --stock stock/recovery.img \
    --hal resources/popsicle/recovery-hal.zip --list
```
