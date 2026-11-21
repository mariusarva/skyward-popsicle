# 升级提示词模板（直接粘给 AI）

升级时把下面整段复制给助手，并把方括号里的内容换掉。

---

我在维护一个 skyward recovery 的定制分支，代码在你的工作目录里（也可以用
`https://github.com/Ff428/skyward-popsicle` 拉下来）。目标是给**小米 17 Pro Max（popsicle，
OS4.0.6.0.XPBCNXM）** 出 recovery 镜像。

先读这三个文件，再动手：

- `docs/SYNC-UPSTREAM.md` —— 同步上游主仓库与布丁仓库的流程、已知冲突清单
- `docs/CUSTOMIZATIONS.md` —— 本分支的五处定制（跨版本必须逐条确认还在）
- `docs/PACK-IMAGE.md` —— 编译出包与自检流程

上游：
- 主仓库 `https://github.com/kirklin/skyward`，新版本 tag 是 **[vX.Y.Z]**
- 布丁仓库 `https://github.com/mariusarva/skyward-pudding`，最新补丁是 **[v0.2.5x]**

请按 `SYNC-UPSTREAM.md` 的三方合并流程做：

1. base 用布丁补丁所针对的上游 tag，pudding 分支 = base + 补丁，up 分支 = 新上游 + 五处定制；
2. `git merge pudding`，冲突按文档里的 9 处清单处理（保留上游新接口，补回布丁的进度标记与后台加载机制）；
3. 逐条确认五处定制仍在，尤其 `haptics.rs` 的 `FF_SINE=0x5a`、`input.rs` 的面板打分、`ui.rs` 的 `Haptics::open` 非致命、`app.rs` 的 `vibration: 0.7` 与点击震动、`pages.rs` 的三个品牌常量；
4. 编译：`ANDROID_NDK_HOME=<ndk> ./scripts/build-android.sh`；
5. 出包 + 自检：`repack_image.py` → `patch_avb.py` → `verify_image.py`，必须打印 `ALL CHECKS PASSED`；
6. 产物按 `v<N>-skyward-popsicle-OS4.0.6.0.XPBCNXM-weaver-haptics.img` 命名（版本号放最前面），告诉我改了什么、以及刷入后要重点验哪几项。

注意：
- 不要改 `LICENSE_ORIGINAL`（GPL 许可页的原始署名必须保留）；
- 不要把 `Haptics::open` 的失败重新变成致命错误；
- 不要把 `scan()` 改回「取第一个面板」；
- 出包必须用「骨架替换」的方式改 cpio，不要整目录重新打。

---

## 只改界面/文案时（不升级版本）

```
代码在 skyward-popsicle 仓库。请只改 [首页大标题 / 关于本机大字 / 署名行 / 其它文案]：
对应 crates/skyward-ui/src/pages.rs 里的 HOME_TITLE / WORDMARK / AUTHOR。
改完用 scripts/build-android.sh 编译，再走 repack → patch_avb → verify 出包，
产物名 v<N>-skyward-popsicle-OS4.0.6.0.XPBCNXM-weaver-haptics.img，并告诉我改了哪几个常量。
```
