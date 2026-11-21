# 官方系统升级后：把出包基线跟上去

recovery 镜像不是凭空造的：它拿**官方全量包里的 recovery 分区**当底，只在 ramdisk 里换上 skyward 的几个条目。所以官方每发一版系统，这个底就变了——内核、init、SELinux 策略、adbd、一堆 so 都跟着换。

基线不跟会怎样：skyward 起得来，但模块和库对不上，表现是触屏或显示异常、WiFi 起不来、AVB 校验不过。**不致命，但没必要踩。**

---

## 先确认到底落不落后

手机上跑的是哪个版本：

```sh
adb shell getprop ro.mi.os.version.incremental
# OS4.0.6.0.XPBCNXM
```

仓库里配的是哪个版本：

```sh
grep -A4 '^\[stock\]' devices/popsicle.toml
```

两个一样就不用动。不一样就按下面走。

---

## 一条命令做完（推荐）

```sh
python scripts/bump_stock.py --version OS4.0.7.0.XPBCNXM
```

脚本会按顺序做完这些事：

1. 去 xmfirmwareupdater 查这个版本的**全量包文件名**，拼出官方下载地址；
2. 改 `devices/popsicle.toml` 的 `[stock]`，把 `recovery` 摘要清空；
3. 提交并推送，等 CI 跑完；
4. 从 CI 日志里抓出新包的 recovery 分区 SHA-256，回填进 `recovery` 字段；
5. 再提交推送一次——从此每次构建都做摘要强校验。

已知摘要、想跳过等待的话：`--version ... --sha256 <64位>`。只想改文件不推送：加 `--dry-run`。

---

## 手工三步（脚本不适用时）

### 1. 找新版全量包的地址

打开 <https://xmfirmwareupdater.com/hyperos/popsicle/>，在表格里找目标版本那一行，**Type 必须是 `Recovery`**（`Fastboot` 包里没有 `payload.bin`，取不出分区），点进去记下 `Package Name`：

```
popsicle-ota_full-OS4.0.7.0.XPBCNXM-user-17.0-ab12cd34ef.zip
```

官方下载地址的规律是「服务器 + 版本号目录 + 包名」：

```
https://bkt-sgp-miui-ota-update-alisgp.oss-ap-southeast-1.aliyuncs.com/OS4.0.7.0.XPBCNXM/popsicle-ota_full-OS4.0.7.0.XPBCNXM-user-17.0-ab12cd34ef.zip
```

先 HEAD 一下确认 200，再往下走：

```sh
curl -sI "<上面的地址>" | head -3
```

### 2. 改 `devices/popsicle.toml`

```toml
[stock]
version = "OS4.0.7.0.XPBCNXM"
package = "https://bkt-sgp-miui-ota-update-alisgp.oss-ap-southeast-1.aliyuncs.com/OS4.0.7.0.XPBCNXM/popsicle-ota_full-OS4.0.7.0.XPBCNXM-user-17.0-ab12cd34ef.zip"
recovery = ""
```

摘要**先留空**。空着的时候打包器不校验，只把实际值打印出来。

### 3. 推上去，把摘要回填

```sh
git add devices/popsicle.toml && git commit -m "feat(device): 出包基线升到 OS4.0.7.0.XPBCNXM" && git push
```

CI 跑到 `Pack the recovery` 时日志里会出现（`skyward-pack` 打的）：

```
the recovery partition in https://.../popsicle-ota_full-OS4.0.7.0.XPBCNXM-...zip
has the SHA-256 3f9c...e21a, which the configuration does not name yet
```

把这一串 64 位十六进制填进 `recovery`，再提交推送一次。从这以后摘要对不上就会直接报错，能挡住下载被截断或换包的情况。

---

## 几个容易踩的点

| 坑 | 说明 |
| --- | --- |
| 拿成 Fastboot 包 | 只有 Recovery 类型的 zip 里才有 `payload.bin` |
| 版本串不完整 | `XPBCNXM` 后缀不能省，`OS4.0.7.0` 和 `OS4.0.7.0.XPBCNXM` 是两个东西 |
| 忘了回填摘要 | 不影响出包，但从此失去「包被换过」的保护；日志里留着那行 SHA-256，随时能补 |
| 以为要重下整包 | CI 用 `actions/cache` 按**版本号**缓存 recovery 分区，同版本第二次起是秒过；换版本必然重下一次 8~9 GB |
| 拿 `stable beta` 的包配稳定版系统 | 版本号一致即可，但优先用与手机 `ro.mi.os.version.incremental` 完全相同的那个包 |
| 忘了改 `devices/popsicle.toml` 就以为 CI 会自己跟 | 不会。CI 只按这个文件办事 |

---

## 为什么要盯着 `[stock]` 而不是别处

`devices/popsicle.toml` 里其余段落（`[display]`、`[modules]`、`[haptics]`、`[wlan]`…）描述的是**这台机器的硬件**，官方发版不会改变硬件，不用动。只有 `[stock]` 指的是「拿哪一版官方包当底」，它必须跟着系统版本走。

顺带一句：`[modules]` 里的驱动模块是运行时从手机自己的分区读的，所以 recovery 的内核版本和系统小版本不完全一致通常也能开——但那是运气，不是设计。基线对齐才是正路。

## 换基线时补丁层怎么办

`resources/popsicle/recovery-hal.zip` 是**独立于基线版本**的一层：换 4.0.7.0 时
照旧注入，不需要跟着改。里面的 HAL 是 vendor 侧的可执行文件与库，只通过内核
驱动与 TEE / eSE 打交道，和 recovery 的内核小版本没有绑定关系。

只有在两种情况下才需要重新抽一份：

- 新固件把 vendor 侧的 HAL 接口换了（表现为开机能进 skyward，但解密时
  `weaver` / `keymint` 起不来，`logcat` 里能看到 `Failed to start omapiSeService`）；
- 换了机型或换了容器（比如又刷了别的 TWRP 版本）。

重新抽取的办法：从一份**能正常解密 /data 的成品镜像**里，把原厂没有的那些
条目捞出来，打成同样结构的 zip（`sbin/**`、`system/lib64/**`、
`system/etc/vintf/manifest/*.xml`、`lib/firmware/cs40l26*`，保留权限位），
覆盖 `resources/popsicle/recovery-hal.zip` 即可。脚本
`scripts/augment_stock.py --list` 可以核对注入结果。
