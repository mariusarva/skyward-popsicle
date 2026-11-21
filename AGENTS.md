# 给 AI 的说明

这个仓库是「小米 17 Pro Max（popsicle）定制 recovery」。接手前按顺序读两份文档：

1. **[RESTORE.md](RESTORE.md)** —— 这是什么、环境怎么配、凭据怎么拿、出包怎么走、踩过哪些坑
2. **[docs/SYNC-UPSTREAM.md](docs/SYNC-UPSTREAM.md)** —— 上游发新版时怎么跟（第零节是日常四步）
3. **[docs/LOCAL-BUILD.md](docs/LOCAL-BUILD.md)** —— 不靠 Actions 在本机出包（含跨平台改动清单）

## 三条硬规则

1. **别 merge 上游。** 本仓库历史是从源码快照重建的，`v0.3.x` tag 指向的真实上游提交
   不是 HEAD 的祖先。`git cherry-pick <上游提交>` 正常（按内容合并），
   `git merge upstream/vX` 会报 unrelated histories。要走 `scripts/sync_upstream.py`。
2. **出包有两条路，都别手工拼镜像。** 推 main 走 Actions 是发布用；
   `python scripts/local_build.py` 在本机一条命令跑完同样八步（断网、额度用完、
   或者只想快点看到成品时用），细节在 [docs/LOCAL-BUILD.md](docs/LOCAL-BUILD.md)。
   两条路产物都过 `scripts/check_build.py` 的四项门槛、ramdisk 都是 **1279 条**。
   不要退回 `repack_image.py` 手工拼。
3. **品牌可以改，许可不能动。** 大字/署名在 `crates/skyward-ui/src/pages.rs` 的常量里，
   首页标题是 `HOME_TITLE`；但 `LICENSE_ORIGINAL` 里 Skyward / Copyright (C) 2026 Kirk Lin
   必须原样保留（GPL 要求）。

## 改之前先确认

- 设备配置 `devices/popsicle.toml` 用了本 fork 的版本（`[data] weaver_first`、`[backup]`、
  8 个 `[[service]]` 指向 `/sbin/*`、cs40l26 的 `[haptics] resonance`），不要被上游版本覆盖。
- 原厂 recovery 的 ramdisk 缺 HAL 层，打包前必须过 `scripts/augment_stock.py`，
  否则镜像能开机但解不开 `/data`、马达也不起。成品 ramdisk 应是 **1279 条**。
- 交付物命名把版本号放最前面：`v15-skyward-popsicle-OSx.x.x.x-<sha>.img`。

## 备份怎么维护

记忆或脚本更新后跑一次：

```bash
python scripts/backup_task.py --push
```
