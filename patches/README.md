# patches/

## skyward-pudding-v0.2.49.patch

来自 [mariusarva/skyward-pudding](https://github.com/mariusarva/skyward-pudding) 的累积式补丁副本，**针对上游 `kirklin/skyward` 的 tag `v0.2.0`** 生成。

内容就是本分支「多出来的那批功能」：下拉控制中心、WiFi 持久化并自动连接、双击亮屏、记住解密密码自动解密、解密过渡动画、网络下载（高级 → 网络下载）、锁屏滑动解锁、排序记忆、AOD、无线传输。

用法（必须在 `v0.2.0` 的源码树上）：

```sh
git checkout v0.2.0
git apply --whitespace=nowarn patches/skyward-pudding-v0.2.49.patch
```

跨版本套补丁必然冲突。v0.2.0 → v0.3.0 那次合并的实际处理办法见
[docs/SYNC-UPSTREAM.md](../docs/SYNC-UPSTREAM.md)。

## 为什么补丁没有跟着本仓库的源码树一起「消解」

v8 之后本仓库的源码树已经是「上游 + 补丁」合并后的结果，补丁本身不再参与编译。保留它是因为：

1. 下次上游出新版本时，需要拿**新补丁**做同样的三方合并，留一份参照；
2. 想追溯某个功能到底来自补丁的哪一段（`git apply` 之前先 `git apply --stat` 看清单）。
