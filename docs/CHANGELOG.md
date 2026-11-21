# 版本演进（v6 → v115）

每版都在真机上验证过；出问题的两版（v4/v5 触控失灵）把原因写在下面，避免以后重犯。

| 版本 | 基线 | 主要变化 | 结果 |
| --- | --- | --- | --- |
| v4 | 上游 v0.2.0 + 布丁 v0.2.49（源码编译） | 首次从源码出包：`FF_SINE` 震动、点击震动、默认强度 0.7 | ❌ 完全不能触控 |
| v5 | 同上 | `Haptics::open` 改为非致命 + `gentle_click` 懒重试 | ❌ 仍不能触控 |
| v6 | 同上 | `scan()` 双面板打分选屏 + 热插拔替换 | ✅ 触控/震动都正常 |
| v7 | 纯上游 v0.3.0 | 升级到 0.3.0；当时误判「0.3.0 已内置布丁功能」，实际没有 | ⚠️ 可用但少了一半功能 |
| v8 | 0.3.0 + 布丁 v0.2.49 三方合并 | 把 v6 的功能全部补回，仅 9 处冲突 | ✅ 真机确认无问题 |
| v9 | v8 | 关于本机 hero：大字改「叼着奶瓶逛酒吧」、署名改「© 2026F」 | ✅ |
| v10 | v9 | 大字改「叼着🍼逛酒吧」（🍼 用矢量画，字体无 emoji）、署名改「© 2026 卑鄙坑Baby」 | ✅ |
| v11 | v10 | 回到纯文字「叼着奶瓶逛酒吧」 | ✅ |
| v12 | v11 | 首页大标题「Skyward」→「卑鄙坑Baby」 | ✅ |
| v13 | v12 | 打通 GitHub Actions 自动出包；补回原厂 ramdisk 缺的 HAL 层（`recovery-hal.zip` + `augment_stock.py`），基线升到 OS4.0.6.0 | ✅ 真机确认无问题 |
| v14 | v13 + 上游 0.3.1 的 4 个 fix | DNS 多服务器回退、长日志分条、属性读不到时页面不崩、写 boot 分区前先比对；工具链钉到 1.100.0 | ✅ |
| v15 | v14 + 上游 main 两个 fix + 布丁 v0.2.55 的三处修复 | 等模块生成的文件（5 秒）；`/vendor/lib64/hw` 进库搜索路径；明文 data 直接开分区；`dm::remove` 前先查状态 | ✅ |
| v16 | v15 + 布丁 v0.2.55 的下载与解密改进 | 8 个 GitHub 镜像 + 9 地址并发测速（3 秒测完）+ 记住上次最快的 30 分钟；只写 `keydirectory` 的分区按默认 aes-xts 解密；recovery 里跑不起来的 weaver/SE 程序直接跳过 | ✅ |
| v17 | v16 + 布丁 0.2.55 的首页布局与文案 | 首页照布丁重排：无线网络+文件管理 / 安装刷机+创建备份+恢复备份 / 第二系统+高级工具+恢复设置 / 页脚重启系统+清除数据+版本更新；文案「高级→高级工具」「设置→恢复设置」「重启→重启系统」「双系统→第二系统」；挂载与解密挪进高级工具页 | ✅ |
| v18 | v17 + 布丁 0.2.55 的最后几处界面差异 | 首页 `WLAN` 改「无线网络」；文件管理器排序胶囊挪到路径条下方（`files::strip_with_sort`）；设置页「WiFi」节换成「解密」节（`Switch::Remember`）；AVB 确认改「滑动以写入」、网络下载英文改 `Download` | ✅ |
| v19 | v18 + 对照布丁 v0.3.9 的官方镜像 | 逐函数比对镜像里的 `system/bin/recovery`：文件管理排序行（画布 51 dp 装不下 54 dp 起的胶囊，整行被裁掉）、无线传输提示补「WLAN 需先连接。」；同版顺手补的「解密页进度条」是误判，v20 已撤回 | ✅ |
| v20 | v19 | 撤回 v19 在 `data_status` 里补的进度条：进度条与转圈一直由 `app::paint_preparing` 每帧叠画，两套一起画就是两条错位横条（240×8 @cy+126 与 200×4 @cy+124），转圈也成两个环（r=48 / r=40）。改回 `None`，与布丁一致 | ✅ |
| v21 | v20 | 默认深色主题：`State::default()` 的 `dark` 改 `true`，深色调色板的背景是纯黑 `#000`（面板是 OLED）。设置页顶部两个预览块仍可切回浅色，选择照旧存 `/metadata/skyward/settings.json` | ✅ |
| v22 | v21 | 深色主题去蓝：调色板拆出 `fill`/`on_fill`/`meter`/`switch_on`，按钮、开关、滑块、进度条、键盘回车、Dock 末键全部改成黑灰白；`accent` 在深色改成 `#EDEDED`（只当墨用），`badge` 改中性 `#262626`。浅色主题取值与原色相同，一个像素没变 | ✅ |
| v23 | v22 + 布丁 v0.3.7 的 U 盘互传与批量文件操作 | 文件管理器「内部存储 / U 盘」并排切换、跨位置复制移动、多选批量复制/移动/删除、面包屑中间折成「…」、高级工具加「U 盘管理」、U 盘改读写挂载并在挂载前清旧挂载；解密页的记住密码改成可点胶囊 + 滑动风险窗；版本页显示 4 条精简更新日志；panic 写内核日志 | ⚠️ 4 处复查修正见 v24 |
| v24 | v23 + 出包后复查 | 多选栏「全选 / 全不选」的动作跟着标签走（原来标签变了动作没变）；未选中时三个动作胶囊变灰并提示；排序行右侧胶囊按传入顺序优先保留（空间不够先丢「选择」，不再先丢「内部存储」）；**选中后复制/移动退出选择模式**（原来停在选择态，粘贴栏永远不出现，U 盘互传断在第一步）；批量删除改成滑动确认（`ask_sheet` 从记住密码窗抽出，`Confirm::DeleteMarked`） | ✅ |
| v25 | v24 + 真机反馈 | 下拉面板的 `SHADE_PULL_DP` 原来写 110 但**当像素用**（只有 40 dp），手指动一点面板就全开；改成按 dp 换算、取面板高一半（254 dp），约 2:1 跟手。松手动画 1.8 s → 0.34 s，曲线 smoothstep → ease-out。面板底色去掉 alpha 238（圆的下半部走混合、矩形走直写，脚角两个三角发灰），改不透明 `#18181E`。振动滑块：控制中心松开原来**完全不震**，现在拖动每跨一档按当前档强度回震、松手再震一次确认；设置页滑块同样按档回震。`lightest` 0.45 → 0.30，两端差 3.3 倍（默认 0.7 仍是 0.70 满强度）。每帧 `eprintln!("frame …")` 改成只打超过 120 ms 的慢帧 | ✅ |
| v26 | v25 + 对照布丁 0.3.7（20261116）官方镜像逐条比文案 | 粘贴栏三条：复制到此处 / 移动到此处 / 取消（原来按剪贴板操作只画一条，到了目标文件夹改不了主意）；`Action::Paste` 带上 `FileOp`，`dock_bar_at` 按「当初选的那个」点亮。文件页胶囊「选择」改「多选 / Select」（进入后不再变「完成」，按一下退出）。未插 U 盘提示改「请先连接 U 盘」。U 盘空时单独说「U 盘为空」。记住密码的风险窗换成官方的整段（「任何拿到这台手机的人…」）。设置页去掉 0.3.7 已删的「解密」分组（开关只在解密页胶囊上）。同名冲突页文案改「该文件夹中已有“X”。」。版本更新页的更新日志分组改「本次更新」。AVB 写完回一句「已写入：逐块校验 开/关，启动校验 开/关」。顺手修掉 `skyward-preview` 两处编译不过（v23 的 `file_clash` 签名、`Newest::notes`） | ✅ |
| v27 | v26 + 官方 0.3.7 的九张真机截图 | 深色主题的 `accent` / `fill` / `meter` / `switch_on` 从灰白改回蓝色 `#4A80F7`，滑块圆点改白。首页：大标题改回 `Skyward`、删掉搜索栏、账户与机型拆成两张卡、「恢复设置」改「选项设置」。高级工具页按官方顺序重排（U 盘管理挪到导出日志之后），U 盘图标改紫。解密页恢复**一条**进度条（原来叠层的 200×4 细条与页面的 240×8 并存，v20 干脆两条都不画，现在由页面自己画），并补回返回箭头；已解密页新增「已记住密码」开关卡片。文件管理页顶部拆成两行：路径 + 另一侧位置 / 排序三枚 + 多选。版本更新页去掉「本机」分组，更新日志挪到页尾。控制中心两个滑块改成**并排胶囊**（原来上下两条细线带圆点），面板高 508 → 464 dp。锁屏底改纯黑，不再透出页面的光晕 | ✅ |
| v28 | v27 + 真机反馈 | 首页大标题恢复「卑鄙坑Baby」（v27 照官方截图误改成 `Skyward`，品牌是本 fork 的，不跟官方）。解密页的环与进度条改回**每帧叠画**（`App::paint_preparing`），页面本身不再画：页面是布局一次就缓存的，v27 把动态的条画进页面，条只在页面重建时跳一下，看着卡。进度改成跟着 `skyward_crypto::data::progress()` 平滑追：等待阶段以时间常数渐近到 0.9 以下，不会跑满干等；真实进度上来后按每帧耗时归一的指数追赶，30 帧和 60 帧一个速度 | ✅ |
| v29 | v28 + 真机反馈 | 文件页「内部存储 / U 盘」改成**固定顺序的两段控件**，内部存储永远在左、U 盘在右，当前那段亮；顺手修掉 `strip_room` 的右对齐基准：原来按屏幕右边算，路径胶囊压在右侧胶囊下面，「内部存储」只露出一个「内」。版本号 `0.3.1 → 0.3.7`（更新页/关于页不再写着 0.3.1）。解密进度条不再回跳：`set_data_progress` 删掉「真实进度更小就立刻拽回去」那一句——正是它让条在 creeping 值与真实步进之间来回跳；`tick` 只在目标领先时前进 | ✅ |
| v30 | v29 + 真机截图与官方首页照片 | 文件页位置胶囊**不再重复**：v29 一边留着路径条自带的「内部存储」胶囊、一边又加了两段控件，屏幕上成了「内部存储 内部存储 U 盘」。现在路径条不再画位置，两枚胶囊分贴行首与行尾（内部存储永远左、U 盘永远右、当前那枚亮），进文件夹后面包屑排在两者之间、放不下就从最早的折成「…」。首页账户卡补上**头像**：读手机里账号的照片（`/data/data/com.xiaomi.account/files/xiaomi_user_avatar_*`，JPEG/PNG 都认），圆形裁切 + 双线性缩放；也可以自己放一张在 `/sdcard/.skyward/avatar.jpg`（或 `.png`），两处都没有才退回灰色电话图标。新依赖 `zune-jpeg`（纯 Rust，官方那套解码同源） | ✅ |
| v31 | v30 + 出包后复查 | 面包屑折叠按**实际**宽度算（`span_of(&shown, shown.len() < below.len())`），不再把「…」的宽度预先算进去、白折一级；账号头像按修改时间从新到旧逐个试读，坏掉的那张不会让整页没有头像，目录项先判是不是文件。其余同 v30 | ✅ |
| v32 | v31 + 解密提速；同一份代码出两个品牌 | **先挂载、挂不上才查**：`open()` 原来每进一次解密页都先跑 `fsck.f2fs -a` 扫全部分区的表（512GB 的 data 要几秒），现在直接按手机的方式挂载，**只有内核拒绝时才检查一遍再挂**，失败退回原来的顺序。进度从 5 档改成千分位（`AtomicU16` + `at(‰)`）：映射 180、挂载 420、共享密钥 540、各用户 DE 键 660–880、建出来的凭据 900，条跟着真实进度走而不是卡在 90% 等。`data::resume(0.62)` 让「用记住的密码自动解锁」接着上一段走，不再把进度打回 0、也不再多空转一个线程队列。等待期的爬升从 `0.9 / 2.4s` 降到 `0.35 / 0.9s`，真实进度一上来就接管。品牌改成编译期可指定（`pages::brand` + `SKYWARD_BRAND_TITLE` / `_WORDMARK` / `_AUTHOR`），不指定就是本 fork 那套 | ✅ 品牌版（v32） |
| v33 | 与 v32 同一份代码 | 只换品牌：首页大标题 `Skyward`、关于页大字 `Skyward`、署名 `© 2026 Kirk Lin`（官方 0.3.7 里读出来的原样），其余与 v32 完全一致 | ✅ |
| v34 | v33 + 进度条走满才切页、更新状态一秒一读 | 解密分两段（分区 0→720‰，用户凭据 730→1000‰）：第一段结束不再把页面切到「已解密 / 输入锁屏」，有记住的密码就**留在同一页**接着走；整段做完后条在约 0.3 s 里补满，**满了才切页或离开**（`App::data_end` 存「条满之后做什么」）。`readings` 第一次就读全部，**更新状态读不到时每秒重读**（原来 30 秒一轮，开机那行要挂十几秒）；准备完成前首页不显示「无法读取更新状态」；两段之间条以 4%/s 缓爬（最多领先 8%） | ✅ 品牌版（v34） |
| v35 | 与 v34 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v35） |
| v36 | v35 + 布丁 0.3.10–0.3.15 | 振动波形按时长生成（22 ms / 24000 Hz）、所有滑动确认与解锁补振动；锁屏充电显示瓦数；拔盘卸载失败不再终止 recovery；首页 WLAN 开电台阶段写「正在连接」；WLAN 驱动预热提到固件挂载后；新增**模块管理**（列表/开关/下次启动移除/从 zip 安装）与**应用管理**（分页、搜索、卸载、系统应用只读页）；更新下载离开页面照跑、进度变化任何页面都重画；版本号 0.3.15 | ✅ 品牌版（v36） |
| v37 | 与 v36 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v37） |
| v38 | 布丁 v0.3.15 整棵树（0.3.1 + 官方补丁） | **不再逐条抄**：`crates/skyward-ui` 换成布丁的 `app/` 模块拆分（29 个文件）与 `pages.rs`/`fmt.rs`/`files.rs`/`widgets.rs`/`canvas.rs`/`theme.rs`/`avatar.rs`，`crates/skyward/src` 换成布丁的 `ui.rs`（4318 行）、`waveform.rs`、`keys.rs` 等；新文件页 `FilesOtg`（U 盘多分区）、`PasteMany`/`DeleteMany` 批量操作、`KeepSort` 排序记忆随之到位。去掉只有旧线才有的 `aod.rs`/`omapi.rs`/`touchprobe.rs`。工作区 edition 2021 → **2024**、resolver 3（布丁用 2024，`let` 链只有它认） | ✅ 品牌版（v38） |
| v39 | 与 v38 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v39） |
| v40 | 布丁 v0.3.15 的树，只留本机必需的补丁 | 剩下 28 个还带着上游 0.3.1 之后补丁的文件**全部换回布丁原版**：备份不再跳过读不了的文件、`hex` 用 workspace 的 crate、`Installer`/`MTP` 用 `worker::Worker`、`libraries_of` 回到布丁的三组路径、预览回到 178 张截图、`avbctl` 去掉主仓库新加的 `Switch`/`Copy`。`crates/` 下 14075 个文件有 **14066 个与布丁逐字节相同**，只剩 9 个本机必需的定制 | ✅ 品牌版（v40） |
| v41 | 与 v40 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v41） |
| v42 | v41 + 解密进度条回到页面 | 布丁 2026-11-16 把 decrypt 页的条整个删了（注释写的是「条只是猜长度，跟做的事对不上」），页面上只剩转圈。这一版按**真实打点**把条加回来：`skyward-crypto::data` 里一个千分位 `AtomicU16`，`map` 前 80、映射完成 220、探测 250、挂载前 280、挂载后 360、共享键 400、各用户键 470、各用户 DE 520–680、建出来的凭据 690、第一段收 700；第二段 `unlock` 700 → 900 → 990。UI 只增不减地追（`App::set_data_progress` + `step_data_bar`：向已知进度缓动、等待时 3.5%/s 爬、最多领先 6%），两段之间不停顿；整段做完条在 0.45 s 里补满，**满了才切页 / 才显示「已解密」**（`data_finish: Option<DataAfter>`）。手机自身部件起来那段的阶段（`progress::done()`）按 35% 折算，条从开机第一帧就在走、不回跳 | ✅ 品牌版（v42） |
| v43 | 与 v42 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v43） |
| v44 | v43 + 真机反馈「卡在自动解密页、进不去首页」 | 页面对「还在等的事」加了三条硬边界：①「记住的密码」那次尝试 **15 秒**没回来就让出页面（`Decryptor::detach` 不 join、不阻塞），改成让用户自己输；②分区已开、条在收尾时，**最多 0.7 秒**必定离开/显示已解密，条卡住也不拦路；③页面说「正在打开」却**没有任务在跑**超过 3 秒，重开一次或明确报错（不再无声干等）。另修 `remember::writable()`：`/metadata` 没挂载时先挂再改，`forget()` 真的删掉那个文件（原来写在挂载点空目录里、报成功，密码一直留着每次开机重试）；保留的凭据超过 512 字节视为无效 | ✅ 品牌版（v44） |
| v45 | 与 v44 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v45） |
| v46 | v45 + 找回头两次整树同步丢掉的定制 | 「自动解密→黑屏」无限循环的真因：**`Haptics::open` 又变回 `?`**。马达模块在页面之后加载，打开失败就把整个 UI 线程结束掉 → init 再起 → 又是同一页；顺带把 `synaptics_tcm2` 的 insmod 打断（模块记成已加载但没 probe）→ 主屏触控彻底没有。同一批还丢了 **`scan()` 双面板打分**（抓到副屏 976×596，主屏点不动）、**`FF_SINE=0x5a`**（cs40l26 不认 FF_CUSTOM，所以没震动）、**默认强度 0.7**。四处全部找回，另加：`run()` 起不来重试 4 次、`turn()` 出错只记日志不结束进程、`panic=unwind` + 工作线程 panic 变成页面上的失败原因、连续 3 次启动都没开出分区就直接进首页 | ✅ 品牌版（v46） |
| v47 | 与 v46 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v47） |
| v48 | v47 + v46 的真机日志 | **条走完了页面却走不掉**：`tick` 为了让转圈每帧重画，先 `self.frame = None`，紧接着又调 `leave_decrypt()`；滑页取页面快照时帧是空的，panic 在 `draw.rs:707`，被 `catch_unwind` 兜住后页面永远停在解密页——条满了、下面还挂着「已记住密码」那一行。改成：决定离开时先留着帧、等它随本回合的绘制回来再走；离开统一交给 `tick`（`decrypted()` 只置 `data_finish`），`leave()` 拿不到帧时退化成底色不再 panic。另：**每一下点按都回一次马达**（只算 `state.vibrate`，控件自己已经震过的不重复），原来只有滑动确认 / 解锁 / 返回手势有震动 | ✅ 品牌版（v48） |
| v49 | 与 v48 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v49） |
| v50 | v49 + v48 的真机日志 | **解密页底下那条英文**：`Pages::open` 把上一版留在 `/skyward/run/last-failure` 里的 panic 原文（`the recovery ended on a panic at … draw.rs:707`）用 `report()` 弹成了 toast，正好落在自动解密页下方。现在只写进 kmsg，页面上一个字都不显示。**震动强度调节没差别**：①设置页与控制中心的强度滑块拖动时**一次都不震**，只在松手时震一下，没法边拖边比；现在**每跨一档就回一次马达**（20 档，落手那一档不震）；②跨度从 `0.30–1.0` 拉到 **`0.15–1.0`**（6.7 倍）；③点击的**时长也跟着强度走**（最轻约 11 ms、最重 20 ms），让两端不只是音量差一点，长度也差得出来 | ✅ 品牌版（v50） |
| v51 | 与 v50 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v51） |
| v52 | v51 + v50 的真机日志 | **解密页底下又一条英文**：`Pages::open` 切 USB 端口失败时把 `{e:#}` 原样 `report()` 上去了（`sys.usb.state is Some("none") after 5000 ms, not "mtp,adb"`）。真因在 `crates/skyward-pack/src/mtp.rc`：**那条照抄 adbd 的规则**——端口每次切换都先走 `"none"`，adbd 一停就把 `sys.usb.ffs.mtp.ready` 清 0；而 MTP 服务用 `no_disconnect=1`，进程一直开着，不会重写一次 → 绑定条件永远凑不齐，端口停在 `none`，**连 adb 一起丢了**。规则删掉（标志由 MTP 服务自己置位/清零），切失败时退回 `adb` 而不是把端口留在 `none`，页面只说一句中文。**下拉面板底部两角**：`paint_shade` 的圆脚是「矩形 + 两个圆 + 一条矩形」，半透明叠两次的地方深一档，两个下角各有一道弧形接缝。改成不透明。另：等触控面板 5 s → 10 s（真机 5.1 s 才出现，差 50 ms 误报「用音量键操作」） | ✅ 品牌版（v52） |
| v53 | 与 v52 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v53） |
| v54 | v53 + v52 的真机日志 | **解密条三段卡顿**：条的下限一直是 `progress::done()*0.35`（启动阶段 0/20/40/60/80/100% × 0.35 = 0/7/14/21/28/35%），每过一个阶段就停一下；上限又是 `data_reported + 6%`，真实进度每报一跳（80→220→700‰）就再停一下。2 秒的等待加 0.6 秒的真解密，被切成 7%/15%/30% 几段。改成**一条按时间走的扫掠**（`0.86·(1-e^(-t/1s)) + 0.134·(1-e^(-t/6s))`，上限 99.4%），真实进度只当**下限**；开完分区后最后一段固定 0.55 s 从当前位置补满（`data_finish_from`），**满了才离开**；启动阶段的百分数不再参与画条。**系统应用点不开**：`system_apps` 的行根本没挂 hit；现在每行可点，进的是与用户应用同一张详情页（包名/位置/占用/类型），动作为**导出安装包**到 `/sdcard/Skyward/apps/<包名>.apk`（分区只读，卸不了，第二行点一下会说明原因）。顺带读出系统应用的「占用」（原来恒为 0），列表副标题在名字等于包名时改显示所在分区，不再重复一行 | ✅ 品牌版（v54） |
| v55 | 与 v54 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v55） |
| v56 | v55 + 官方完整包刷不进去的真机日志 | **刷不进官方完整包**：`install: Failed(Error("move off the installed system: unmount /vendor: Device or resource busy (os error 16)"))`。`detach` 里的 `pin` 只扫「配置里列出的程序」，而这些路径它一个都没匹配上（`/proc/<pid>/exe` 与配置不同源），于是 `/vendor` 被当成没人用，走了硬 `umount`；真正占着它的是 16 秒时起来的 `wpa_supplicant`（`/vendor/bin/hw/wpa_supplicant` + `/vendor/lib64`）。改成**扫所有进程**的 maps 与 fd，把落在这些分区上的文件全部 mlock，并且按挂载点汇总一行；`release_partitions` 遇到 EBUSY 不再把整次安装打断，改成 `MNT_DETACH` 让它离开目录树（块设备本身不动），并在日志里说明是哪个挂载点 | ✅ 品牌版（v56） |
| v57 | 与 v56 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v57） |
| v58 | v57 + 正式刷完官方完整包的真机日志 | **安装成功、写回 recovery 失败**：`write /dev/block/by-name/recovery_b: Operation not permitted (os error 1)`（截图里那一行英文）。`open(O_RDWR)` 成功但每个 `write` 都 EPERM，只有两种情况：分区被 `BLKROSET` 标了只读，或被别的持有者以 `BLK_OPEN_RESTRICT_WRITES` 打开。现在写前先 `BLKROGET`、需要就 `BLKROSET 0`，写到一半又被标只读就再清一次重写同一块；仍然 EPERM 就**改从分区所在的磁盘、按 `start`（512 字节扇区）换算出的偏移写**，写前先逐字节比对「磁盘该处」与「分区内容」，一致才落笔。页面不再出现 `{e:#}`：`Reflash::Failed` 改带原因（只读 / 没有该分区 / 被占用 / 其它），中文一句话，原文只进 kmsg。**顺带**：`/vendor` 被 `MNT_DETACH` 之后 `supervise` 还在每秒重启 `/vendor/bin/qseecomd`、`android.hardware.secure_element-service.qti`，exec 全失败，一次安装刷了 819 行 `No such file or directory`；现在「错误是 NotFound 且程序文件真的不在了」就写一行说明并停掉那个监督线程 | ✅ 品牌版（v58） |
| v59 | 与 v58 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v59） |
| v60 | v59 + 刷完官方完整包的真机日志（`recovery.log` 第一行 `misc command "boot-recovery" args []`，两次启动都是） | **四件事都在 recovery 侧**：①**进度条不动**——条只认 update_engine 自报的 `Completed … bytes downloaded`，而它 30 秒才一行；现在取「FUSE 已服务字节」与它自报值的**较大者**，`step_install_bar` 按 0.1 秒一帧平滑跟随，条下写明「已写入 X / Y · Z/s · 约剩 T」。②**刷得慢**——本地包的 FUSE 块 64 KiB→1 MiB（`max_read` 一起到 1 MiB），同一块重读不再算 SHA-256（文件一直开着）；日志里 9.2 GB 用了 6 分 13 秒。③**无限重启 / 没有 REC**——`misc` 里只剩 `boot-recovery` 而没有要做的事时，bootloader 每次重启都把手机送回 recovery；现在页面起来时清掉这种空请求（`Work::Menu`/`Exit` 且 command 非空），安装结束时也清。④**必须手动合并 super**——安装完直接 `start_merge`（目标槽由 `/metadata` 的 source 换算），页面显示合并百分比；合并没结束点「重启系统」会被拦下并提示，系统请求的更新合并完自动重启。⑤**root 掉了**——更新覆盖 `boot`/`init_boot`，安装完把目标槽的启动镜像存到 `/sdcard/Skyward/boot/<分区>_<槽>.img` 并在结果页写明路径。⑥**REC 写不回去**——写回失败时把镜像存到 `/sdcard/Skyward/recovery.img`，页面告诉用户可以 `fastboot flash recovery` 刷回 | ✅ 品牌版（v60） |
| v61 | 与 v60 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v61） |
| v62 | v61 + 「能不能自己把 root 补回去」 | **更新覆盖启动镜像后，用手机自己的 root 工具把新镜像重新打一遍再写回**，不再只是把镜像存到卡上让用户手动弄：①**Magisk**——用 `/data/adb/magisk/boot_patch.sh` + `magiskboot`，`KEEPVERITY`/`KEEPFORCEENCRYPT` 都保持 true（关掉 verity 在 HyperOS 上直接起不来），`PREINITDEVICE=metadata`；recovery 里 `getprop` 答不出 `ro.build.version.sdk` 和 `ro.product.cpu.abi`，patcher 会把手机当成 32 位的，所以临时目录里放了个 `getprop` 替身顶在 `PATH` 最前。②**KernelSU**——用设备上自己的 `/data/adb/ksu/bin/ksud boot-patch -b <镜像> -o <输出>`，模块和 init 都在这个二进制里，需要 `magiskboot` 时用 Magisk 留下的那份。③打好的镜像**补零补齐到分区大小再整块写**——镜像比分区短时，尾部残留的 AVB footer 会让 bootloader 去校验一个对不上的签名，直接开不了机。④另存一份到 `/sdcard/Skyward/boot/<分区><槽>-root.img`，页面写明 `fastboot flash <分区>` 刷回即可。⑤APatch 的 patcher 在它管理器里（不在分区上），这一类只存镜像并在页面点名 | ✅ 品牌版（v62） |
| v63 | 与 v62 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v63） |
| v64 | v63 + 刷完 v60 的真机日志（`misc command "boot-recovery" args []`、主页「更新已写入，等待合并」、合并 28% 被打断） | **三件事**：①**条下那行「已写入」卡住不动的真因**——`done` 取的是 `max(FUSE 已服务字节, engine 自报字节)`，而 update_engine 会**预读整个包**，FUSE 的 `served` 早早冲到 9.2 GB，条瞬间满格（99%）然后停住，`install_rate` 也因为 `done` 不再增长而算不出来。改成 `done = min(served, engine + 64 MiB)`：只在 engine 自报值前一小段领跑，既不 30 秒一跳也不瞬间满格。②**条下加滚动行**（`Event::Step`）：`PartitionInfo new` / `Completed N/M operations` / `Hashing partition k` / `Step 2` 各翻成一句中文，`INSTALL_LINES = 4`，最新在最下、旧的往上淡出，`Partition`/`Applied`/`Hashing`/`Finishing` 四类，同一句连着两次不重复。③**自动合并真生效**——v60 的两处都没跑到：`begin()` 里只在 `Standing::Merging` 且 `snapshots_mapped` 时才继续，重启后状态是 `Unverified`（`Waiting`）所以直接跳过；而安装结束时 `/metadata` 根本还没挂载，`standing()` 返回 `Unknown`，`merge_after_install` 什么也没做。现在 `begin()` 先 `open_metadata()`（只读挂载并保持），`resume_merge()` 对 `Installed`/`Waiting`/`Merging`/`Failed` 一律起合并、合并完**自动重启进系统**；`merge_after_install` 直接用 `Outcome::Installed` 给的槽，不再问 `info.slot`。④**不再往卡上放启动镜像**——删掉 `save_boot_set`/`Kept::saved`/`save_copy`，安装结束时顺手删掉老版本留下的 `/sdcard/Skyward/boot`；root 仍由 `keep_root` 用手机自己的 `ksud`/`boot_patch.sh` 打回新镜像，只是不另存 | ✅ 品牌版（v64） |
| v65 | 与 v64 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v65） |
| v66 | v65 + 刷完 v64 的真机日志（`recovery.log` 只有 `recovery up`，没有任何合并行；页面「更新已写入，等待合并」，重启反复回 REC） | **合并从来没真正开始过，而且失败后还在重启**：①`initiate` 要求「这个槽的每一个快照都已映射」（`everything has to merge at once`），而 recovery 是重启起来的，一个都没映射——v64 只把手动合并的路径修了，自动路径第一步就失败。新增 `skyward_vab::map_snapshots`：按 `/metadata/ota/snapshots` 逐个把更新过的分区映射成 dm-snapshot（已经是 snapshot 表就跳过，被普通映射过就先卸挂载点再拆掉重映），`merge::run` 在 `start_merge` 之前调用它。②**boot control 不再是前提**：`merge::run` 里 `Binder::open` / `BootControl::open` 失败时原来整条合并直接报错；现在改成 `Option<BootControl>`，拿不到就写 misc 里同一字节（`bcb::set_merge_status`，AOSP `bootloader_message_ab.snapshot_merge_status`，offset 4060），合并照跑。③**`MergeStatus` 的编号是错的**：`Cancelled` 写成 4，而 AOSP 里 4 是 `kMergeNeedsReboot`、5 是 `kMergeCompleted`、6 是 `kCancelled`；`merge_status()` 又把 `MergeCompleted` 映射成 `None`、`MergeNeedsReboot` 映射成 `None`，于是合并完成后告诉 bootloader 的是「什么都没发生」。现在枚举补齐 `NeedsReboot=4` / `Completed=5` / `Cancelled=6`，映射逐项对正，boot.rs 的读回也认 4/5/6。④**合并失败不再重启**：`merge_told` 原来 `m.ended()` 就 `leave_after_merge` → 重启 → 系统起不来 → 回 REC → 再失败 → 无限循环。现在只有真的读到 `Merge::Done`/`NeedsReboot`（`merge_done` 记着，跨帧）才离开；失败就停在页面上。⑤**页面上不再有英文**：`Standing::Failed(code)` 的 `{code:?}`（`SnapshotOpenFailed` 之类）和 `merge_failed(lang, why)` 里的原始 `why` 都不再上屏，`why` 只进 kmsg。⑥**多一条出口**：`Standing::Failed` 时页面除了「滑动以合并」再给一个「滑动以放弃这次更新」（`Confirm::GiveUp` → `Effect::GiveUp` → `skyward_vab::cancel`：state 置 `Cancelled`、拆掉这个槽的快照并删记录、告诉 bootloader `kCancelled`），放弃后开机回到更新前的系统，不用再被卡在 REC。⑦合并前重启也先合并：`Effect::Reboot(Target::System)` 时若还有未合并的更新，先起合并、合并完自己重启；`Effect::Merge` 手动合并成功后也自动重启进系统。⑧日志：`recovery up` 那行补 `; cleared, so a restart starts the system`；`resume_merge` 不匹配时也打一条 `no update waits to be merged in the … slot: <Standing>`，下次日志能直接看出是 `None`/`Compressed`/`Unknown` 还是真在 `Waiting` | ✅ 品牌版（v66） |
| v67 | 与 v66 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v67） |
| v68 | v67 + 刷完 v66 的真机截图（写入 99% 卡住、`已写入 9.2GB / 9.2GB` 时 `682/5820` 个操作；结果页写「正在合并 94%」；KernelSU 没补回去） | **四件事**：①**条一下冲到 99%**——`done` 之前取「FUSE 已服务字节」，而 update_engine 是**边读边写**，9.2 GB 早被读满，条先到顶；现在主进度改用 update_engine 自己的 `overall progress N%`（按每个操作实际写入的字节加权，唯一和真正写完多少对得上的数），operations 比例只作没有 overall 时的兜底，读到的字节只当下限。`step_install_bar` 从「一秒冲到 target+1% 再停 30 秒」改成**按上一次的速率线性外推**（最多领先 3%，每 0.1 秒一帧追），条在半分钟的空档里也一直在走。条下滚动行加百分比（`已应用 682/5820 个操作（11%）`）。②**写入慢**——本地包不再经 FUSE 喂给 update_engine：`apply()` 直接把真实路径交给它（`--payload=file:///sdcard/…`），省掉 9.2 GB 从内核到本进程再回内核的两趟拷贝和九千多次 `/dev/fuse` 往返；adb sideload 与 blockmap 仍走 FUSE。③**合并没有进度页**——安装页在写入完之后**不切结果页**，同一张卡片标题换成「正在合并更新」、详情「已完成 N%」、条接着走（数据来自 `Standing::Merging`，`set_update` 同步到 `Install::Merging`）；合并期间按返回只提示不能离开，合并结束才显示结果页（「已合并」/「可以重启」），并在 **4 秒**后自己重启进新系统（原来只对系统请求的更新自动重启，本地包要手动点）。结果页不再写「正在合并 94%」那一段。④**root 保住**——新增 `root::carry`：把**当前运行槽**的 `init_boot`（里面就是 KernelSU 打好的 LKM 与 init，Magisk 同理）**整个分区**读出来、补零到目标分区长度、写进**新槽**的同名分区，一次读写、不依赖 `ksud`/`magiskboot`；只在 `carry` 失败（分区读不了、长度不合）时才退回 v62 的 `ksud boot-patch` / `boot_patch.sh`。`Kept` 多一个 `from`，页面写明「已把 A 槽的 init_boot 整个刷进新系统」 | ✅ 品牌版（v68） |
| v69 | 与 v68 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v69） |
| v70 | v69 + 刷完 v68 的六点真机反馈（8 分钟才刷完、条 41% 时操作数是另一回事、合并完自动重启把 root 截断、WiFi 45 秒才连上、解密条太细） | **写入提速四招**：①问 `update_engine_sideload --help` 认不认识 `--max_threads`，认识就按核数夹 **2–6** 传（一个核写、其余拆操作），不认识的版本一个字不加，免得整包刷不进去；②写入期间把每个 policy 的 `scaling_governor` 抬到 `performance`（写不进去就顶 `scaling_min_freq`），`cpu::Demand` 在 `Drop` 里恢复原值；③`/sys/block/*/queue/read_ahead_kb` 一律抬到 **4096**；④子进程 `setpriority(-10)`。安装期间帧间隔固定 **100 ms**，把 CPU 让出去；每 30 秒 kmsg 一行 `install: X of Y GB in T s (R MB/s), P%`，收尾再一行总耗时/速度/核数。**两个百分比对不上是口径问题**：`overall progress` 把每个操作按写入量加权（条跟它），`已应用 682/5820` 是按个数算的，天然不等——顺手把操作数那行的括号百分比删掉，页面上不再有两套数互相打架。**不再自动重启**：删掉 `leave_after_merge`，合并完停在这一页，页脚写明「合并完成后会停在这一页，由你手动重启进入新系统」。**保 root 三层兜底**：真因是「carry 在合并之前跑、页面第一帧就有『重启到系统』按钮」，用户在写入未完成时点重启 → 截断；现在先起合并、合并结束再补 root —— `carry` 整分区搬运 + **1 MiB 回读比对**，`keep` 用手机自己的 `ksud`/`boot_patch.sh` 打新镜像 + 回读比对，两条都不成再 `save` 一份到 `/sdcard/Skyward/init_boot.img` 供 `fastboot flash init_boot`；`Root::rooted` 只要求 `adb/ksu|ksud|magisk|ap|apd` 任一目录存在，不再依赖能不能发现 `ksud` 二进制（`ksud` 本身改成 5 条绝对相对路径候选）。**WiFi 秒连**：`wlanmem::Saved` 多存一个 `security`，电台一 On 就拿记住的 SSID+密码+加密类型直接 `Join`，不等扫描；`SETTLE` 600→300 ms、接口轮询 100→50 ms。**解密条加粗**：`(196,4)` → `(240,9)`，百分比下移 22→26 dp | ✅ 品牌版（v70） |
| v71 | 与 v70 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v71） |
| v72 | v71 + 刷完 v70 的真机日志（`recovery.log` 500.9 s 说「the update has merged」而页面停在 95%；`root: ksud boot-patch ended as exit status: 1` + `write out new boot failed: No such file or directory`；`init_boot_b holds nothing`；819 行 qseecomd/secure_element 每秒重启） | **五件事，两件是 root 一直失败的真正原因**：①**`ksud boot-patch -o` 要的是文件夹不是文件名**——给 `…/ksu-patched.img` 它会解包、加 LKM、重打包，最后「write out new boot failed: No such file or directory」，一个字节都没写出来；改成给一个干净的目录，取它留下最新的 `.img`，`--magiskboot` 只在它自己的 `--help` 里认这个词时才传（这版不认）。②**`root::carry` 把块设备当文件量大小**——块设备的 `st_size` 是 0，`init_boot_b holds nothing` 就是这么来的，`carry` 从来没有成功过一次；改用 `BLKGETSIZE64`，普通文件才退回按文件长度。③**先打更新写的镜像、再搬运行槽的镜像**：前者是那个槽自己的、为将要启动的系统做的，后者等于把另一版本的 ramdisk 塞到新系统前面，只当兜底。④**95% 不完成的真因**：`Merge::Done` 事件和线程结束是两轮读到的，`tell_merged` 是当轮局部量，事件落在上一轮就再也不会告诉页面——改成 `tell_merged \|\| merged`，看的是跨帧记着的 `merge_done`。⑤**安装页卡顿**：`floor = 100 ms` 原来只要「有 installer」就生效，翻包、等待确认的滑入动画都被压到 10 帧；现在只在真正写入（`Install::writes()`）时压。另：一个程序启动即退出（`/vendor` 被 detach 后 qseecomd 与 secure_element 都是 exit 255）连续 3 次就放手不再重启，一次安装省下 800 多次 fork 与 800 多行日志 | ✅ 品牌版（v72） |
| v73 | 与 v72 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v73） |
| v74 | v73 + 刷 v72 的真机日志与实测（`install: 8.57 GB of payload written in 424 s (20.7 MB/s)`、`root: back in the init_boot of the A slot, patched by KernelSU`；同一块盘上每次 fsync 的 144 KB 写入能跑到 1.4 GB/s） | **三件事**：①**保 root 也进进度条**——`root::put_back` 从 UI 挪进 install crate，四段（读出槽里的启动镜像 / 用好手机里的 root 工具打补丁 / 写入启动镜像 / 回读校验）按权重 0–0.15 / 0.15 / 0.15–0.9 / 0.9–1.0 画同一条条，root 在独立线程跑、页面用新的 `Install::Rooting` 接着合并条画下去，安装页不再是「合并完就干等 8 MB 读写」。②**不再往 `/sdcard/Skyward/` 留 init_boot**：`SAVED_AT` / `save()` / `Rooted::Missed { saved }` 整条删掉，三层兜底只剩 `keep`（打更新写的那个镜像）与 `carry`（搬运行槽的整分区），都不成就在页面上说一句；要复制的那份本来就是手机正在跑的那个镜像，多留一份只是多一个要对齐的东西。③**提速的凭据与旋钮**——安装结束时把 `update_engine.log` 的最后 4 MB 拷到 `/mnt/vendor/persist/update_engine.log`（`/skyward` 是内存盘，重启后日志就没了，而「为什么八点几个 GB 走了七分钟」正是重启后才来读的），日志导出也会在重启后从 persist 取这份；`Worker thread count:` / `Num verification threads:` / `Not using snapshot on VAB device` 这些只说一次的行顺手抄进 Skyward 自己的日志；`prop.default` 补上 `ro.virtual_ab.num_worker_threads=6` 与 `num_verify_threads=6`——原厂只写了 `compression.threads=true`，COW 写入就落回一个线程，一个包八点几个 GB 全排在它后面，而盘是闲的（`augment_stock.py --threads`，八核留两个给读包） | ✅ 品牌版（v74） |
| v75 | 与 v74 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v75） |
| v76 | v75 + 刷 v74 的真机日志（`snapshot.cpp(3507) Userspace snapshots disabled: recovery`、`Compression disabled: recovery`、`Using legacy Virtual A/B (dm-snapshot)`，全日志没有 `Worker thread count:`；5820 个操作、9.19 GB 载荷、`8.57 GB of payload written in 380 s (23.1 MB/s), 8 cores`；同一台机 `dd iflag=direct` 读包 2.1 GB/s） | **不再让 update_engine 写包**：VABC 在 recovery 里整条关掉，`ro.virtual_ab.num_worker_threads` 只在那条路上被读，v74 补的属性等于没补——所以这一版自己做 applier。`payload.rs` 只读 payload 的头与 manifest（`CrAU`、24 字节头、protobuf 里 58 个分区 / 5820 个操作 / 4096 块 / `partial_update` / `dynamic_partition_metadata`）；`applier.rs` 把每个分区按目标槽的 liblp metadata 铺开（逻辑分区写 super 的物理 extent、`_b` 后缀的名字自己认、`odm_b-cow` 这类残留不会误匹配；物理分区找 `/dev/block/by-name/<name>_b`），一个线程顺序读 payload、四到六个线程并行解 xz、各自 `pwrite` 到位，每条操作解码前比对包给的 SHA-256、每个分区写完读回比对整分区摘要，全部对上了才 `set_active` 切槽——**不再有 COW 副本，也就没有 12 GB 的 merge**。任何一条不成立（增量的 `MOVE`/`BSDIFF`/`ZSTD`、分区大小与 super 不一致、操作不按顺序铺满、块大小不是 4096）都在写第一个字节前退回 update_engine，日志里留一句原因 | ✅ 品牌版（v76） |
| v77 | 与 v76 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v77） |
| v78 | v77 + 刷 v76 的真机日志（`install: the payload is left to update_engine: bluetooth has an operation of kind 1, which this recovery does not apply`，整包仍走 update_engine，`8.57 GB of payload written in 365 s (24.1 MB/s)`） | **把 REPLACE_BZ 补上，门槛一次报全**：拿真机包的 manifest 复核了一遍全部 5820 个操作——`REPLACE` 565、`REPLACE_BZ` 231、`REPLACE_XZ` 5024，没有 `MOVE`/`BSDIFF`/`ZERO`/`DISCARD`，v76 缺的只有 bzip2 一种。①`payload.rs` 的 `Kind` 加 `ReplaceBz`（`update_metadata.proto` 里编号 1），`carries_data` 一并认；`applier.rs` 用 `bzip2 0.6.1`（默认 `libbz2-rs-sys`，纯 Rust，不拉 C 库）的 `MultiBzDecoder` 解一条操作的整串 bzip2，解出长度必须等于这条操作写出的字节数，否则整包退回。②门槛从「遇到第一条不认识的就退出」改成先扫全 manifest：把每个不支持的 kind 按分区汇总成一句 `the payload has operations this recovery does not apply: <分区> (<n> operations, kind <k>); …`，一次把整包缺什么都写进日志，不再一个分区一个分区地试 | ✅ 品牌版（v78） |
| v79 | 与 v78 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v79） |
| v80 | v79 + 逐文件核对 `mariusarva/skyward-pudding` v0.3.15 的补丁（69 个文件：39 个与本仓库逐字节相同，其余 30 个只是本机定制，没有新内容可并） | **控制中心改成 12 个圆磁贴（3 行 4 列）+ 设备状态卡**：时间与日期同一行，磁贴标签 10 dp，图标 24 dp，三排都能点（旧版按 `i % 2` 排两列，后两排落不到手里）；磁贴是 WLAN / 截图 / 重启 / 熄屏 / 语言（简中→繁中→English 轮转）/ 文件 / 终端 / 挂载 / 更新 / 备份 / 设置 / 关机；设备卡写机型、系统版本、槽位与电量（充电时另说「充电中」）。新增三页：**任务**（安装进度、安装脚本最后一行、系统下载，各行通向能动手的那页）、**分区管理**（导出镜像 / 从镜像写回 / 文件系统 / 挂载，下接分区表与挂载状态）、**关机**（先说清安装、备份会被切断，再滑动关机）；高级页改长页可滚动，并接上三页入口与搜索 | ✅ 待真机 |
| v81 | 与 v80 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v81） |
| v82 | v81 + 布丁 v0.3.17（`patches/` 仍停在 v0.3.15，v0.3.16/17 只有 docs） | **同步源改成只跟布丁**：`upstream-baseline.json` 的基线钉在 `pudding@c05106c`（v0.3.17），`sync_upstream.py` 默认 remote/base/head 全走 `pudding`，`watch_upstream.py` 只盯 `mariusarva/skyward-pudding`，主仓库标为 retired。**补 v0.3.17 的 vbmeta 备份与写回**：`avbctl::keep/kept/put_back`（存到 `/sdcard/Skyward/vbmeta/vbmeta_<槽>-r<回滚>-<秒>.img`，写回前整份核对 `AVB0` 与头里的长度，回滚索引低于当前槽就拒绝），AVB 页加备份区与「滑动以写回」。**补 v0.3.16 的内置拼音**：`src/pinyin.rs`（Unihan 读音 ∩ GB2312 的 413 音节 / 4133 字 + 1717 词，字频按 jieba 摊），键盘上方一条（拼音串、三个候选、翻页、中/EN），空格选第一条、退格先退串、回车只落字不触发页面动作；密码与网址页不收汉字，终端与编辑器不带这条 | ✅ 品牌版（v82） |
| v83 | 与 v82 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v83） |
| v84 | v83 | **版本报到 0.3.17**：`[workspace.package] version` 从 0.3.15 起一直没动，更新页拿它跟 GitHub 上最新的 release（v0.3.17）比，于是明明已经补上了 0.3.16/0.3.17 的改动，页面还在推「新版本 0.3.17 / 没有本机的安装包」。**拼音条真正出现**：那条以前只在 `pinyin.on` 为真时才画，而 `on` 只能按那条上的「中/EN」才切得开 —— 永远到不了；现在收汉字的页面一直带着它，默认就是中文（密码、网址、终端、编辑器不带，搜索只收 ASCII，因为它按拼音首字母匹配）。**安装再快一点**：解压线程 `cores-2` → `cores-1`（8 核上 7 个），xz 不再校验 CRC64（载荷本身的 SHA-256 在解压前已经比过，写完还会整分区读回比对）。**校验段单独计时**：`install: the writing was read back in X s (Y MB/s)`。**WLAN 首次 `set_up` 的 EBUSY 原地重试**：网卡刚出现时驱动还没准备好接 IFF_UP，以前把失败交回页面按秒退避，现在 120 ms 一问、最多 0.9 s，外层退避也从 5 s 收到 1 s。**录屏**：`skyward-ui/src/record.rs` 把画面每一帧去掉 alpha 编成 JPEG（`image` 0.25，4:2:2，质量 80），三个编码线程 + 一个写线程按固定 30 帧/秒铺时间轴（来不及就补上一帧，页面不等编码），装进标准 AVI（`RIFF`/`hdrl`/`movi`/`idx1`，MJPEG）；文件写到 `DCIM/Screenrecords/Screenrecord_<日期>-<时间>.avi`；控制中心第一行第三个磁贴由「熄屏」换成「录屏」（录时红点变实、图标转红），高级工具页与搜索各有一条，录的时候状态栏钟右边点一个红点、录屏页每秒更新时长。上限 3.9 GB（AVI 长度字段是 32 位），到顶就不再写并记一条 kmsg | ✅ 品牌版（v84） |
| v85 | 与 v84 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v85） |
| v86 | v85 | **高级工具页上滑变黑页**：`long_page` 给的是三屏高的画布，而 `advanced` / `partitions` / `serve` 三处没把画布裁到内容（`fit`），`max_scroll` 就多出两屏 —— 上滑两屏全是黑。三处补上 `fit`。**录屏**（对着 v84 那份 849 MB 的录像查出来的）：`Screenrecord_2026-11-21-10-45-57.avi` 一共 5017 帧，其中 3550 帧是同一张图（48.9 s 之后一直不动），真正在动的只有 808 帧。原因三条：写线程追不上就一直补重复帧、编码线程和写线程之间八格通道堵死后页面丢帧再也回不来、一帧 1200×2608 三个线程只编得出 16 帧/秒。改法：写线程先把通道收干净只留最新帧、落后两格就把时间轴挪到当下不再补、编码搬进编码线程（页面只整块抄一次画布）、`idx1` 与头里的帧数按真写进去的算（以前头写 5373、索引只有 5017）。**录屏设置**：分辨率（原画 1200×2608 / 高清 900×1956 / 标准 600×1304 / 流畅 450×978）、画质（高 q88 / 中 q75 / 低 q60）、帧率（60 / 30 / 24 / 15），点一下换一档，默认标准 + 中 + 30 帧（每分钟约 90 MB）；录制中不让改。**计时窗口**：录的时候右上角一枚胶囊，红点加 `0:42`，画在所有页面与卡片之上。**安装校验不再卡**：第 2 步的条以前按「第几个分区」走，`odm` 一个占 1/58 却要读回 21 秒、`product` 29 秒，那两处条一动不动；现在按字节走（每 32 MB 报一次），条一直在动 | ✅ 品牌版（v86） |
| v87 | 与 v86 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v87） |
| v88 | v87 | **校验改成按核并行**：v84 的日志里写着 `11.29 GB written in 71 s`、`the writing was read back in 69.1 s` —— 读回那一段是一条线程一个分区挨着算 SHA-256，单核约 170 MB/s（盘能跑 1.4 GB/s，`dd iflag=direct` 实测 2.1 GB/s），所以 `odm` 21 秒、`product` 29 秒都压在一次循环里。现在每个分区交给最先空下来的线程，最多 8 条一起读，进度仍是按字节走（每 4 MB 报一次、100 ms 最多报一次，分区名 700 ms 才换一次，免得闪）；报错与逐分区那行日志不变。**WLAN 第一次 up 必回 EBUSY**：日志每个开机都有一行 `the WLAN did not come up: bring wlan0 up: Device or resource busy (os error 16)`，紧接着 1 秒后重试 0.3 秒就成了 —— v84 里原地只等 0.9 秒（1.9 秒才收下 `IFF_UP`），于是页面先闪一下「打开失败」。现在原地等 3 秒，成了以后日志只写一句等了多久。**日志其余部分复查**：`qseecomd` / `secure_element` 的 `exit 255` / `exit 1` 是 `/vendor` 被 detach 之后的常态（连续 3 次就放手）；`ksud ... write out new boot failed` 与 `recovery_b` 的 `EPERM` 都是 11-20 21:21 之前老版本留下的记录，之后每次安装都是 `written back to [A/B]: Ok(())`；kernel.log 里只有 `regulatory.db` / IPA 固件的原厂缺口与 permissive 下的 `avc: denied` | ⏳ 待真机（校验时长） |
| v89 | 与 v88 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v89） |
| v90 | v89 | **计时胶囊能拖着走**：手指按在胶囊上挪，它就停在那儿（`Record::pill` 记着，下一段还从这儿开始；推到屏幕边上贴着边），点一下红点暂停、再点接着录。暂停时页面不再抄画布（一帧十二兆，抄了也是丢），写线程把这一段的暂停时长记进时间轴，继续之后接着写 —— 文件里既没有空档也没有停住的一张图，已录时长也不算暂停的那一段；红点变成白色播放三角，胶囊上多一句「已暂停」，录屏页的状态行同样改口。**录屏页底部的说明被切**：这一页比一屏高一点，用的是固定一屏的画布，最后一行正好卡在屏幕边缘；改成按内容裁的长页（`long_page` + `fit`），能滚到底。**搜索框打不上字**（v88 录屏里那段）：`takes_chinese()` 漏了搜索页 —— 它的键盘不画拼音条（`keyboard_hits` 传 `false`），字母却被攒进拼音串，于是中英文都落不进搜索框，`state.search` 一直是空的。搜索页按拼音首字母与全拼匹配 ASCII，现在字母直接落进框里 | ⏳ 待真机 |
| v91 | 与 v90 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v91） |
| v92 | v91 | **胶囊上多一个结束**：右边加一枚白色方块，按下就结束这一段（和录屏页的「停止录制」走同一条 `Action::Record` 路，`Effect::RecordStop` → `Recorder::finish`）；一条细竖线把时间与它分开。左红点仍是暂停/继续（暂停时变白色三角、胶囊上写「已暂停」）—— 暂停、继续、结束三件事现在都在胶囊上。手指的落点按上一帧实际画出来的三个矩形认（`Pill { body, dot, stop }`），拖动只认整枚；录完把 `record_pill` 清成 `None`，否则停下之后那块地方还会被当成胶囊吃掉手指。停录时顺手把暂停那笔账清零（`paused`/`paused_at`/`held`），下一段从头算。 | ✅ 品牌版（v92） |
| v93 | 与 v92 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v93） |
| v94 | v93 | **录屏改 MP4**：容器从 AVI 换成 MP4（`crates/skyward-ui/src/record.rs`）—— `ftyp`（isom/iso2/mp41）+ `mdat`（一帧一个 JPEG sample，直接贴着写）+ 收尾的 `moov`；`stsd` 里是 `mp4v` 加 `esds` 的对象类型 **0x6C（MJPEG）**，`stts` / `stsc` / `stsz` / `stco` 是**一秒一个 chunk** 的 sample 表，时间刻度 90 kHz（60/30/24/15 帧都整除）。`mdat` 长度先占位、收尾补；sample 表收尾一次写出（那时每帧偏移才定）。文件名 `Screenrecord_<日期>-<时间>.mp4`，规格行报 `… · MP4`。 | ✅ 品牌版（v94） |
| v95 | 与 v94 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ✅ 官方版（v95） |
| v96 | v95 | **录屏改回 AVI**（照用户要求）：`crates/skyward-ui/src/record.rs` 整段换回 v92 的 `Avi`（`RIFF` / `LIST hdrl`（`avih`、`strh`、`strf`）/ `LIST movi` 一串 `00dc` / `idx1`），文件名回到 `Screenrecord_<日期>-<时间>.avi`，规格行回到 `… · MJPEG`；三档设置、计时胶囊（拖动 / 暂停 / 结束）、暂停不写帧这些都不动，`Avi::finish` 按 `index.len()` 写真帧数。为什么改回：这一版相册与播放器对 MP4（`mp4v` + `esds` 0x6C）的 MJPEG 认不认没法在主机上验，AVI 是手机自带播放器与 VLC 都直接放得开的容器；等哪天真机上确认 MP4 能放再换。 | ✅ 品牌版（v96） |
| v97 | 与 v96 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 官方版（v97） |
| v98 | v97（同一棵树，`5be6a16`） | 把 `dd4ca38` 的 host 侧修复与 `5be6a16` 的文档整理带进 main 之后重出。改动只有 `skyward-block` 的 ioctl 请求改用 `libc::Ioctl`（bionic 上仍是 `c_int`）：`recovery` 与 v96 逐字节比只差 `.data.rel.ro` 里 15 个字节，`system-init` 完全相同，ramdisk 条目数（1,280）、解出的文件数（1,004）与压缩后大小（67,080,603 B）都不变——功能面与 v96 无差别，这一版是把 CI 的编译错误修掉之后重出的干净版本。README 重排为「索引 + 折叠块」，`archive/` 收进三份留档 | ⏳ 待真机（品牌版 v98） |
| v99 | 与 v98 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 待真机（官方版 v99） |
| v100 | v99 | **录屏同时支持 MP4 与 MJPEG**：`crates/skyward-ui/src/record.rs` 把容器拆成 `Container`（`Avi` / `Mp4`）与格式 `Format`（`Avi` / `Mp4` / `Both`，`FORMATS` 三档、默认 AVI），写线程手上从「一个 `Avi`」变成一串 `Sink`：一帧 JPEG 只编一次，同一格按顺序交给每个容器（AVI 包 `00dc`、MP4 追一个 sample），两份文件因此是同一批帧、同一条时间轴、同样的暂停账。`Recorder::start` 改成收 `Vec<Take>`（已打开的文件 + 路径 + 容器），`mp4` 那套 `ftyp` / `mdat` / `moov`（`mp4v` + `esds` 0x6C、90 kHz、一秒一个 chunk）整段回到代码里。录屏页多第四行「格式」：`AVI` / `MP4` / `AVI + MP4`，录制中照样锁住；选「两个都写」时同一次录屏开两个文件（共用一个时间戳，`Screenrecord_<日期>-<时间>.avi` 与 `.mp4`），录屏页文件名显示成 `…avi + .mp4`。`Summary` 改成 `files: Vec<Recorded>`（路径 / 容器 / 帧数 / 字节数），收尾对每个文件各记一行 kmsg，再记一行总的；3.9 GB 上限按每个容器各算。`fmt::record_name_as(t, zone, ext)` 让两种后缀共用一个时间戳 | ⏳ 待真机（品牌版 v100） |
| v101 | 与 v100 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 待真机（官方版 v101） |
| v102 | v101 | **同步布丁 `v0.3.23`（`pudding@aa4b52a`），版本号报到 `0.3.23`**。先核了实际差异：`v0.3.23` 的 tag 打在 `c05106c`（就是本仓库 v82 之前对的 `v0.3.17` 那个提交）上，分支头只多两个提交 `d5cf649` / `aa4b52a`，两个提交合计改动只有 `docs/update.json`（tag 与资产名/大小/digest 四处字符串）与 `docs/CHANGES.md`（加一行）——**源码零改动**，`patches/` 里的最后一个补丁仍是 `v0.3.15`。录屏那部分本来就是布丁从本仓库的 `Ff428/skyward-popsicle-1` 分支拿过去的，发布说明末尾写着致谢，所以不存在要往回搬的实现。**版本号**：`[workspace.package] version` `0.3.17` → `0.3.23`，`skyward-info::RUNNING` 跟着变，更新页拿它比 `mariusarva/skyward-pudding` 的最新 release（现为 `v0.3.23`）时不再推「新版本」。**MP4 的两条实情写进界面**：录屏页「格式」行的副标题从「MP4 相册认」改成「AVI 最稳；MP4 相册不一定认这码流」，页脚补上「MP4 里装的是 MJPEG，ffmpeg 与 VLC 都能放，相册对这种码流支持不一定好，打不开就换回 AVI；MP4 索引在末尾，录制中途断电那份打不开，AVI 相对宽容」。`docs/录屏.md` 新增「关于 MP4」一节，`docs/SYNC-UPSTREAM.md` 与 `.github/upstream-baseline.json` 的基线改到 `v0.3.23` / `aa4b52a` | ⏳ 待真机（品牌版 v102） |
| v103 | 与 v102 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 待真机（官方版 v103） |
| v104 | v103 | **录屏格式回到两档，严格照上游 `v0.3.23` 的说明**：`Format` 删掉 `Both`，`FORMATS = [Avi, Mp4]`，默认 AVI；一次录屏只开一个文件，`Recorder::start` 收 `Take` 而不是 `Vec<Take>`，写线程手上一枚 `Sink`。`Summary` 随之收回单文件（`path` / `container` / `frames` / `bytes` / `millis` / `dropped`），页面结束提示就是文件名，kmsg 逐文件一行 + 一行总账（`in two file(s)` 那句删掉）。录屏页「格式」副标题按档说：AVI「自带播放器、相册、VLC 都直接放得开」，MP4「相册按视频收；MJPEG 相册不一定认，打不开就换 AVI」；页脚去掉双写那句，改成「格式两档，里面装的都是 MJPEG」。3.9 GB 上限只剩一份账 | ⏳ 待真机（品牌版 v104） |
| v105 | 与 v104 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 待真机（官方版 v105） |
| v106 | v105 | **录屏删掉 AVI，只剩 MP4（MJPEG）**。`crates/skyward-ui/src/record.rs` 里 `Avi` 结构连同 `RIFF` / `hdrl` / `movi` / `idx1` 那套索引代码整段删除（245 行），`Sink` 枚举随之消失，写线程直接持 `Mp4`；`Container` 只留 `Mp4`（`Container::ONLY`），`Format` 与 `FORMATS`、`DEFAULT_FORMAT` 一起删掉；`Take` 只剩 `file` / `path`，`Summary` 只剩 `path` / `frames` / `bytes` / `millis` / `dropped`。录屏页第四行改成一个只报「容器 · MP4」的说明行（点它不换），`RecordSetting` 回到三项，页脚重写成「容器只有 MP4，里面是 MJPEG；相册不一定认，索引在末尾」。`fmt::record_name` 的后缀改 `mp4`。本机 5 个单测全绿，e2e 真录 75 帧 / 8,149,851 B，`check_record.py` 结构自洽（`stts` 225,000 tick） | ⏳ 待真机（品牌版 v106） |
| v107 | 与 v106 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 待真机（官方版 v107） |
| v108 | v107 | **录屏码流 MJPEG → H.264 Baseline**，治「相册说文件损坏」。新增 `crates/skyward-ui/native/`（openh264 2.4.1 静态库 + `h264_shim.cpp`）与 `crates/skyward-ui/build.rs`；`record.rs` 换成 `H264` 编码器 + `to_i420`（BT.601 整数近似）+ 单编码实例 + 按采集时刻摊 `stts` + `stss`，`moov` 的 `stsd` 改 `avc1` + `avcC`，`ftyp` 品牌 `isomiso2avc1mp41`；画质档从 JPEG q 值改成目标码率（0.10 / 0.16 / 0.28 bit 每像素每帧）；录屏页三处文案与规格行（改报 Mbps）同步；`scripts/check_record.py` 改验 AVCC / `avcC` / `stss`；新增 `scripts/build_openh264.py`。`recovery` 11,253,024 → 11,572,784 B | ✅ 真机验过：能放、相册缩略图正常；但 30 帧档录出 67.7 fps（v110 修） |
| v109 | 与 v108 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 官方版未刷（与 v108 同一份代码） |
| v110 | v109 | **录屏的帧率设定接上了**。页面跟着屏幕刷新率画（这块屏 120 Hz），`Recorder::push` 以前每画一帧就抄一帧，30 帧档实际录出 67.7 fps、6.6 Mbps，IDR 每 0.25 秒一个；开头还有 638 ms + 474 ms 两个空洞（openh264 初始化与首帧编码都发生在第一帧之后）。现在 `push` 按 `next += 1/fps` 放行，编码线程先编一帧黑丢掉的预热帧、就绪前不收帧，第一帧真实画面 `ForceIntraFrame` 成 IDR 保住 `avcC` 的 SPS/PPS；帧通道 1 → 2 格、mux 通道 4 → 8 格。`recovery` 11,572,784 → 11,574,736 B | ✅ 真机验过：能放、`duration=44093`；但 44.09 s 只有 762 帧 = 17.28 fps（v112 修） |
| v111 | 与 v110 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 官方版未刷（与 v110 同一份代码） |
| v112 | v111 | **录屏改成恒定帧率**。v110 只把 `push` 的放行压住了，没管「页面不画」的时候：`show()` 只在 `take_dirty()` 为真时调 `push`，录屏页大部分时间是静的，于是 44 秒的片子里 762 帧、平均 17.28 fps、21.5 秒一个帧都没有。现在 `push` 只把画布放进「最新一帧」那一格（`Mutex<Option<Job>>` + `Condvar`），编码线程自己按 `1/fps` 出帧，这一格没有新画面就再编一遍手里的 YUV（P 帧整帧 skip）；迟到的帧按实际发出时刻记 `at`，`stts` 是一格 3000 tick。`due()` / `next` / 帧通道整个删掉。`recovery` 11,574,736 → 11,557,328 B | ⏳ 2026-11-21 20:27 的日志是这一版的启动记录（19:44 那次开机到 67 s），**没有录屏**：帧率仍未在真机上量过 |
| v113 | 与 v112 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 官方版未刷（同一份代码） |
| v114 | v113 + v112 的启动日志（2026-11-21 20:27） | **三处**。① **马达懒重试**：日志里 `the motor came up` 落在 40.87 s，正好是第一次点屏幕的时刻 —— `Haptics::open` 只由 `gentle_click` 触发，而 cs40l26 的 FF 设备要等模块装完、固件下载完才出现；现在 `show()` 每帧调一次 `take_up_motor`（一秒最多一次，两分钟为限），马达一到位就装上，第一次点按也有震动。② **`push` 只在那一格空着时才抄画布**：屏 120 Hz、编码 30，四分之三的 12.5 MB 抄写本来就会被顶掉，白占一半内存带宽；画面语义不变（每个编码格取的是该格第一帧）。③ **没有 ready 条件的 program 记 `started`**：配置里 8 个 `/sbin` 服务本来都被读成 `was ready in 0.00 s`，像已经报到了；有 ready 条件的照旧记耗时。`check_record.py` 同时开始打 `stts` 的格子分布。`recovery` 11,557,328 → 11,562,544 B | ✅ 四项硬门槛与 AVB 过；录屏待真机 |
| v115 | 与 v114 同一份代码 | 只换品牌：首页与关于页 `Skyward`、署名 `© 2026 Kirk Lin` | ⏳ 官方版未刷（同一份代码） |

## v108：相册说录屏文件损坏（2026-11-21）

真机给的 v106 日志（`20261121-175928.zip`）里录屏只有两行，说明文件**写完了**：

```
[   18.569081] record: started, 600×1304 · q75 · 30 fps · MP4 (MJPEG)
[   30.999065] record: stopped after 371 frames, 26940729 bytes in
                /sdcard/DCIM/Screenrecords/Screenrecord_2026-11-21-17-59-10.mp4
```

371 帧 / 12.43 秒 ≈ 30 fps、26.9 MB（MJPEG 每帧约 72 KB），收尾正常、`moov` 也补了。
所以「损坏」不是文件坏了，是**系统不认里面的码流**：

| 步 | 发生什么 |
|---|---|
| 1 | Android 的 `MPEG4Extractor` 用 `FourCC2MIME` 按 `stsd` 的 fourcc 定 MIME，`mp4v` 被硬映射成 `MEDIA_MIMETYPE_VIDEO_MPEG4` —— 它**不看** `esds` 里的 objectTypeIndication，写 `0x6C`（JPEG）也没用 |
| 2 | MediaCodec 于是去创建 `c2.android.mpeg4.decoder`，喂进去的是 JPEG |
| 3 | 解第一帧就 `ERROR_MALFORMED`，相册把「解不出第一帧」显示成**文件损坏** |
| 4 | 换容器写法没用：系统里没有 MJPEG 解码器（`c2.android.mjpeg.decoder` 不存在，软解表里也没有） |

只能换码流。改成 H.264 Baseline：`avc1` + `avcC` 落在 `c2.android.avc.decoder` 上，
Android 4.1 起每台设备都有。

实现上踩过的坑，逐条记在这儿：

| 坑 | 结局 |
|---|---|
| openh264 2.4.1 的 `codec/common/arm64/*.S` 缺 `WelsI4x4LumaPredDDLTop_AArch64_neon` 等实现，链接报 undefined | **不带 `-DHAVE_NEON_AARCH64`**，编纯 C++ |
| `cpu-features.c` 被 `-Dandroid_getCpu*=wels_*` 改名，`cpu.cpp` 不跟着改名就找不到 `android_getCpuCount` | 那六个 `-D` 对**所有文件**生效，不只 `cpu-features.c` |
| openh264 靠函数指针表互相引用，纯按需拉取会漏成员 | `cargo:rustc-link-lib=static:+whole-archive=skyward_h264` |
| 整包要链 libc++ | `h264_shim.cpp` 里自带 `operator new` / `__cxa_guard_*` / `__cxa_pure_virtual` |
| 两条编码线程各编各的，ffmpeg 报 `reference picture missing during reorder` | **只留一个编码实例**：P 帧引的是同一实例的上一帧 |
| 写线程按时间轴补帧（MJPEG 那套）会重发 P 帧、断参考链 | 改成「来多少写多少」，`stts` 的间隔按**采集时刻**（`Job.at`）算，帧少但时长不变 |
| `Settings::of` 的 scale 是面板的一半，单测传 `(600,1304)` 算出的码率差一倍 | 面板要传 `(1200,2608)` |

本机验的（没有真机也能确认链路通）：`h264test` 编 30 帧 500,995 B，ffmpeg 认
`h264 (Constrained Baseline) 600x1304`、解 30 帧无错；`Recorder` 全链 e2e 输出
`MP4 name take.mp4 · 20 frames · 2212 ms · 311984 B`，ffmpeg 报
`h264 (Constrained Baseline) (avc1 / 0x31637661), 600x1304, 9.04 fps, Duration 00:00:02.21`，
输入流信息干净；`check_record.py` 报「结构自洽」。8 个单测全绿。

还有两条与码流无关的实情（页面页脚也写了）：`moov` 在文件末尾，录制中途断电那份
打不开；上限 3.9 GB（`stco` 是 32 位偏移）。

## v112：设的 30 帧，文件里只有 17.28（2026-11-21）

v110 真机录的那段（`Screenrecord_2026-11-21-19-41-20.mp4`，11,485,671 B，44 秒）能放：
MediaStore 里 `duration=44093, width=600, height=1304, mime=video/mp4`，相册缩略图是
recovery 主界面（品牌串、胶囊都对）。`check_record.py` 也自洽：762 sample / 26 chunk /
`avc1` + `avcC`（SPS 1 / PPS 1）/ `stss` 26 帧（间隔恒 30 帧），SPS/PPS 没混进 sample。
问题在时间轴：

| 项 | 设定 | 这段实际 |
|---|---|---|
| 帧数 / 时长 | — | 762 帧 / 44.093 s = **17.28 fps** |
| 码率 | 3.8 Mbps | 11,475,672 B / 44.09 s = **2.08 Mbps** |
| IDR | 1 秒一个 | 每 30 帧一个（对的） |
| 单帧大小 | — | 平均 15,059 B，最小 122 B（整帧 skip），最大 87,800 B（IDR） |

`stts` 的间隔分布：

| 间隔 | 格数 |
|---|---|
| 27–39 ms | 516 |
| 39–55 ms | 55 |
| 55–111 ms | 64 |
| 111–333 ms | 32 |
| 333 ms–1 s | 19 |
| 小于 27 ms | 75 |
| 大于 1 s | 1 |

≥ 66 ms 的那 89 格占掉 1,931,499 tick = **21.5 秒**，将近一半时长一帧都没有。根因
是 v110 只压住了「页面画得多快」，没管「页面根本不画」：`Pages::show` 只在
`self.app.take_dirty()` 为真时 `compose` + `push`，而录屏页静止时只有胶囊上的计时
每秒跳一次、红点在闪。没有重绘就没有帧，`Job.at` 记的采集时刻就在 `stts` 里留下
几百毫秒的空洞 —— 开头那 883 ms + 429 ms 也是这么来的。

改法是把「出帧」从页面手里交给编码线程：

| 处 | 改动 |
|---|---|
| `Recorder::push` | 不再挡帧、不再排队：整块抄进「最新一帧」那一格（`Mutex<Option<Job>>` + `Condvar`），旧的一张还回池子 |
| 编码线程 | 自己起表：`next` 每次 `+ 1/fps`，到点就编；这一格没有新画面就再编一遍手里的 YUV（P 帧整帧 skip，几十字节到几 KB） |
| 编码线程 | 迟到的帧按**实际发出时刻**记 `at`，所以 `stts` 正常是一格 3000 tick，编码真跟不上时才是 6000、9000 |
| 暂停 | 暂停期间不出帧、时间轴也不走；继续之后从当下重新起算（写线程那侧 `last_at = None`） |
| 收尾 | `stop` 置真 + `Condvar::notify_all`，编码线程交出手里这一格就走，写线程照旧补 `moov` |

本机 e2e 两个场景（host 的 openh264 没有 NEON，一帧要几十毫秒，所以 busy 到不了 30）：

| 场景 | 画面 | 墙钟 | 文件 |
|---|---|---|---|
| busy | 8 ms 一帧（模拟 120 Hz），60 次 `push` | 815 ms | 17 帧 / 804 ms / 21.1 fps |
| idle | 4 帧撒在 1.5 s 里，再空等 1 s | 3.033 s | 84 帧 / 3.017 s / 27.8 fps |

idle 那 84 帧里有 77 格的 `stts` 是 3000 tick、6 格是 6000（host 上一帧三十毫秒出头，
偶尔错过一格），时间轴与墙钟差 16 ms。`recovery` 11,574,736 → 11,557,328 B。

## v110：帧率设的 30，录出来 67.7（2026-11-21）

v108 真机录的那段（`Screenrecord_2026-11-21-18-52-30.mp4`，18,516,571 B）能放：
MediaStore 收进去了、`duration=22443`、相册缩略图正常，`MJPEG → H.264` 这条修对了。
把它的 `stts` 摊开看，另外三个问题露出来：

| 项 | 设定 | 实际 | 差多少 |
|---|---|---|---|
| 帧率 | 30 fps | 稳态 `delta` 749 tick = 8.32 ms → **120 fps** | 4 倍 |
| 码率 | 3.8 Mbps | 18,497,533 B / 22.44 s = **6.6 Mbps** | 1.7 倍 |
| IDR 间隔 | 1 秒 | `stss` 51 个、间隔恒 30 帧 = **0.25 秒** | 4 倍 |
| 开头 | — | 前两个 `stts` 间隔 57,485 + 42,633 tick = **638 ms + 474 ms** | 空洞 |

前三条是同一个根因：`push` 没有任何节流。页面跟着屏幕刷新率（这块屏 120 Hz）画，
画一帧就抄一帧交给编码器，`settings.fps` 只进了 `openh264` 的 `fRcFps` 与 chunk 粒度，
从来没挡过帧。于是编码器按 30 帧算的码率预算被摊到四倍帧数上，`uiIntraPeriod = fps`
这个"按帧数算"的设定也跟着变成四分之一秒一个 IDR。

第四条是另一回事：`H264::new` 与首帧编码都在编码线程里同步做，通道只有一格 ——
第一帧进去之后 `try_send` 一直失败，页面这几百毫秒的帧全丢，而 `Job.at` 记的是
采集时刻，写线程如实把这 638 ms 记进时间轴。

改法：

| 处 | 改动 |
|---|---|
| `Recorder::push` | `due()` 按 `next += gap` 放行，`gap = 1/fps`；落后超过四格（页面卡过）就从当下重新起算，不连着放行一串 |
| `Recorder::push` | `ready` 为假时直接返回：编码器还在建，这段时间的画面不进文件，`stts` 从第一帧真实画面起算 |
| 编码线程 | 先编一帧黑当预热（不进文件），把 openh264 的表、参考帧、码率控制建起来 |
| 编码线程 | 预热后 `ForceIntraFrame`，并在拿到 SPS/PPS 之前每帧都要一次 IDR —— 预热帧占掉了初始化那次 IDR，不补的话第一帧真实画面是 P 帧，`avcC` 空的，相册缩略图黑 |
| 通道 | 帧通道 1 → 2 格，mux 通道 4 → 8 格，扛住一次写盘抖动 |

本机 e2e（60 次 `push`、8 ms 间隔，模拟 120 Hz）：`15 frames · 704 ms · 332,207 B`，
`stts` 前两条 `3000, 2994` tick（33.3 ms，正好 30 帧），第一帧在 `stss` 里，
`avcC` 有 SPS/PPS，码率 3.78 Mbps。host 上 21.8 fps 是 mingw 编的 openh264 没有 NEON，
真机 arm64 编得动 30 帧。


## v58/v59：刷完官方完整包，Skyward 写不回 recovery 分区（2026-11-20）

真机：v56/v57 把「刷不进官方完整包」修好了，9.2 GB 的包写进去了（`Installed { slot: B, wipe: false }`，安装 384 秒），但页面上多出一段英文：

```
未能将 Skyward 重新刷入 recovery 分区。
write /dev/block/by-name/recovery_b
Operation not permitted (os error 1)
```

### 为什么 open 成功、write 却 EPERM

写 `/dev/block/by-name/recovery_b` 时 `OpenOptions::read(true).write(true)` 是成功的——设备节点在、权限够、设备也在。`write` 返回 `EPERM` 只有两种来源，都在内核的块设备写入口：

| 来源 | 触发方式 | 能否解除 |
| --- | --- | --- |
| `bdev_read_only(bdev)` | `BLKROSET 1`（另一个写者、`fsck`、`mke2fs`、某些 `dm` 目标设过） | `BLKROSET 0` 可以 |
| `bdev_writes_blocked(bdev)` | 有持有者以 `BLK_OPEN_RESTRICT_WRITES` 打开过该设备（`dm-verity`、`dm-default-key` 一类） | 不能，只能等持有者关闭，或绕开分区节点写 |

安装期间 `update_engine` 在 `super` 上建了 9 个 `dm-snapshot`，结束后 `UnmapAllSnapshots` 全部删掉——那个窗口正好覆盖「装完 → 我们写回 recovery」这一步。v56/v57 的日志里 `written back to [B]: Err(write …)` 就落在这个窗口里。

### 现在怎么写

```
open 分区 → BLKROGET → 只读则 BLKROSET 0
  └─ 写一块；若 EPERM 且能清掉只读 → 清掉、重写同一块
  └─ 仍 EPERM → 读 /sys/dev/block/<maj>:<min>/partition + start
       ├─ 不是分区（整盘或 dm 设备）→ 报「只读」
       └─ 是分区 → 打开 /dev/block/<disk 的 maj:min>，偏移 = start × 512
            ├─ 逐字节比对「磁盘该处」与「分区内容」，不一致就停手
            └─ 一致 → 从这个偏移写，读回校验同样走偏移
```

写整盘偏移这件事只对「真是分区」的设备做，而且**先证明那个位置的内容就是分区的内容**（100 MB 级别的比对，UFS 上一秒内）；任何一处不一致就放弃，不会往别的地方落笔。`dm` 映射出来的设备没有 `partition` 文件，走不了这条，也不该走。

### 页面

`Reflash::Failed` 从 `Failed(String)` 改成 `Failed { slots, why: Unflashed }`：

| `Unflashed` | 页面（简中） |
| --- | --- |
| `ReadOnly` | 未能将 Skyward 重新刷入槽位 B 的 recovery 分区：分区被锁成了只读。 |
| `Missing` | 未能将 Skyward 重新刷入槽位 B：这台手机没有那个 recovery 分区。 |
| `Busy` | 未能将 Skyward 重新刷入槽位 B 的 recovery 分区：分区正被占用。 |
| `Other` | 未能将 Skyward 重新刷入槽位 B 的 recovery 分区，原因记在日志里。 |

原始错误（`write /dev/block/...: Operation not permitted (os error 1)`）只进 kmsg 和 `/skyward/run/logs`，页面上一个字都不出现。

### 顺带修掉的刷屏

`/vendor` 在 `detach` 时用 `MNT_DETACH` 离开目录树，但 `supervise` 线程还在按 `RESTART_DELAY = 1 s` 重启 `/vendor/bin/qseecomd` 和 `/vendor/bin/hw/android.hardware.secure_element-service.qti`——路径已经不在树里，`execve` 每次都是 `ENOENT`：

```
[   83.487482] /vendor/bin/hw/android.hardware.secure_element-service.qti: start ...: No such file or directory (os error 2)
[   83.680221] /vendor/bin/qseecomd: start /vendor/bin/qseecomd: No such file or directory (os error 2)
...（整个安装期间，共 819 行）
```

现在 `supervise` 遇到「错误是 `NotFound` 且 `Path::new(&s.path).exists()` 为假」时写一行 `left alone, the partition it runs from is out of the tree for the install` 并让这个监督线程停下；路径还在、只是起不来的（缺链接器之类）照旧重试。

## v56/v57：官方完整包刷不进去（2026-11-20）

真机：选 `popsicle-ota_full-OS4.0.8.0.XPBCNXM-user-17.0-b074141719.zip`（9,199,922,375 字节），三次都在同一处失败。

### 日志

```
[   16.014] starting /vendor/bin/hw/wpa_supplicant on wlan0, ... with LD_LIBRARY_PATH=/system/lib64:/sbin:/vendor/lib64:...
[   28.755] install: /sdcard/.../popsicle-ota_full-OS4.0.8.0.XPBCNXM-user-17.0-b074141719.zip, 9199922375 bytes
[   28.772] install: an update to 17OS4.0.261112.160620958.QCPECN.S
[   34.604] kept /odm/lib64/ese_weaver.so in memory
[   34.604] kept /odm/lib64/libjc_keymint_transport_nxp.so in memory
[   34.604] kept /odm/lib64/vendor.xiaomi.hardware.miauthsecretd-V1-ndk.so in memory
[   34.658] install: Failed(Error("move off the installed system: unmount /vendor: Device or resource busy (os error 16)"))
```

### 为什么

写包之前要「离开已安装系统」：把开机槽的分区从目录树上撤掉（`detach`）。撤之前先 `pin`——把跑着的程序用的文件锁在内存里，然后把这些**挂载点**记成 busy，busy 的走 `MNT_DETACH`（一定能撤），不 busy 的走硬 `umount`（撤不掉就报错，好知道有问题）。

问题在 `pin` 的取样范围：它只查 `self.programs` 里列出的程序（密钥那批 + 配置里那几个），而 `/proc/<pid>/exe` 与配置里的路径对不上，一个都没匹配到；日志里只有 3 个 `/odm` 的库被锁，`/vendor` 一个都没有。于是 `/vendor` 被当成没人用 → 硬 `umount` → **EBUSY**。

真正占着 `/vendor` 的是 **wpa_supplicant**：16 秒时从 `/vendor/bin/hw/wpa_supplicant` 起，`LD_LIBRARY_PATH` 里有 `/vendor/lib64`，它的可执行文件和几十个 `.so` 全在 `/vendor` 上。它不在 `programs` 里，`pin` 自然看不见。

### 改法

- `services::pin(mounted)` 不再接受程序清单：遍历 `/proc` 下**每一个进程**的 maps 与 fd，把落在待撤分区上的文件全部收进来，逐个 mlock；读取失败的进程（正好退出）跳过，mlock 失败只记一行、不拦安装。
- 每个挂载点只打一行：`N files of /vendor kept in memory for the update`。
- `release_partitions` 返回「没能干净卸载、改成脱离目录树」的挂载点列表；`detach` 把它写进 kmsg。EBUSY 不再是死路：`MNT_DETACH` 之后 update_engine 照常写块设备（设备本身没被撤，页面还在内存里）。
- 顺带：`install_asked` 不再需要程序清单，`detach()` 也不再需要。

## v54/v55：解密条一次性走满、系统应用点得开（2026-11-19）

真机：「自动解密的进度条为什么会有三个阶段性（7%、15%、30% 几）的卡顿？我要的是一次性走满，走满之后就解密完成」，
「应用管理的系统应用，为什么点击没有反应？不会和第三方应用似的点进去可以让你选择卸载什么的」。

### 三个阶段性：条的下限和上限都在按「阶段」跳

`recovery.log` 里这段时间是这样的：

| 时刻 | 日志 | 说明 |
| --- | --- | --- |
| 3.445 | `the phone's own parts are not up yet; the opening waits for them` | 页面已经在等，分区还没开始开 |
| 3.465 | `opening the data partition for 0s, at 0 thousandths` | 条在 0，接下来 **2 秒**没有任何进度 |
| 5.427 | `opening the data partition for 0s, at 80 thousandths` | 手机自己的服务起来了，真正开始映射 |
| 5.809 | `opening the data partition for 0s, at 700 thousandths` | 用户 0 的密钥进去了 |
| 6.030 | `the users whose own files are still closed: []` | 真正的解密到这里就完了 |
| 6.032 | `the decrypt page is done; the menu shows` | 页面离开 |

也就是说**真正的活只有 0.6 秒**（5.43→6.03），前面 2 秒全在等 `keymint`/`gatekeeper` 起来。
那条 0.6 秒的活被切成了 `0/80/700‰` 三跳，而画条的时候又叠了一个下限：

```rust
let boot = crate::progress::done() * 0.35;   // 0/20/40/60/80/100% × 0.35
let done = cx.state.data_progress.max(boot);
```

`progress::done()` 是启动阶段（分区、驱动、服务描述、服务、就绪）数出来的，**一阶段一跳**，
乘 0.35 就是屏幕上那 7% / 14% / 21% / 28% / 35%。再加 `step_data_bar` 原来把
`data_reported + 0.06` 当**上限**（领先不能超过 6%），真实进度每报一跳就顶到天花板停住——
三个卡顿点就是「启动阶段两条 + 真实进度一跳」。

现在的画法：

- `step_data_bar` 自己走一条按时间的扫掠：`0.86·(1-e^(-t/1s)) + 0.134·(1-e^(-t/6s))`，
  1 秒到 56%、2.5 秒到 84%，之后慢慢爬向 99.4%，**不会满**；
- `data_reported` 只当**下限**（`sweep.max(reported)`），再也不会因为「领先太多」而停下；
- 起点的时钟 `data_started` 只在**重新开分区**时清零：等锁屏、离开页面再回来都接着走，
  不会出现「条在 84% 站两秒等扫掠追上来」；
- 分区开完，最后一段用 `data_finish_from` 记下当前位置，**固定 0.55 秒**补满（smoothstep），
  满了才离开页面（原来是从 0 到满 0.45 秒的匀速，从 84% 起来时几乎是瞬移）；
- 页面不再读 `progress::done()`，启动阶段的百分数与条无关。

### 系统应用：行上根本没挂 hit

`system_apps()` 那一页当时是「只读」——`widgets::list` 画完就把结果丢了（`let _ = listed;`），
一行都没有 `Hit`，所以点了当然没反应。现在：

- 每行挂 `Action::SysAppOpen(k)`，进的是**与用户应用同一张详情页**（`Screen::DataAppInfo`，
  用 `state.sys_at` 区分来源）：包名、位置、占用、类型（系统分区，只读）；
- 动作是**导出安装包**：`Effect::ExportApk` 把 `base.apk`（或目录里第一个 apk）复制到
  `/sdcard/Skyward/apps/<包名>.apk`，成败都用现有 `fmt::exported` / `export_failed` 说一句；
- 第二行「系统应用，不能在这里卸载」点一下会说清楚原因：系统分区是只读的，删了系统起不来；
- `system_apps()` 顺手把每个应用的「占用」算出来（原来恒为 0），单个 apk 的条目记它自己的路径；
- 列表副标题在「名字 == 包名」时改显示所在分区，不再把包名写两遍。

## v46/v47：「自动解密 → 黑屏」一直循环（2026-11-19）

真机：「和上个版本一样进不去 rec，一直自动解密、黑屏、自动解密、黑屏，这个顺序；震动还没了。」

**真因是 v38/v40 那两次「整树换成本仓库」时，把五处定制丢了三处**（`docs/CUSTOMIZATIONS.md`
里早就写着「跨版本升级时这五处需要逐条确认还在」，这次没确认）：

| 丢的东西 | 后果 |
| --- | --- |
| `Haptics::open` 的 `?`（第 3 条） | 马达模块在页面之后才加载，`no input device takes force-feedback effects` 直接冒泡 → **整个 UI 线程在 `synaptics_tcm2` 正 insmod 时结束** → 模块半途而废记成已加载却没 probe → 主屏触控消失；init 再起服务，又是同一页 → 无休止的「解密页 → 黑屏」 |
| `scan()` 双面板打分（第 2 条） | 副屏 Goodix 976×596 的节点先注册，`见一个就用` 抓到副屏，主屏点不动 |
| `FF_SINE = 0x5a`（第 1 条） | 发回 `FF_CUSTOM`，cs40l26 不翻译成 BUZZGEN → **一点震动都没有** |
| 默认强度 `0.7`（第 4 条） | 回到 `SLIDER_START`（0.1），就算响也只是一下轻抖 |

四处全部照 `docs/CUSTOMIZATIONS.md` 的原文找回。另外加了三层「不许再无限循环」：

1. `ui::run()`：`Pages::open` 失败**重试 4 次**（间隔 0.6/1.2/1.8 s），模块晚到就等它一下，不再一次失败就退出。
2. `ui::run()` 的回合：`turn()` 返回 `Err` 只写日志（同一原因 1 秒最多一条）并继续，**不再结束进程**；`catch_unwind` 兜住 panic 也一样。为此 `[profile.release]` 从 `panic = "abort"` 改成 `"unwind"`，
   `Worker::drop` 里 join 到 panic 也不再 `panic!`，解密线程的 panic 变成 `Ended::*::Err("the work came apart: ...")` 直接显示在解密页。
3. `/skyward/run/starts` 计数：连续第 3 次启动还没把分区打开，就**直接进首页**（分区留作未解密，等同「暂不解密」），
   并把上一次结束的原因（`/skyward/run/last-failure`，由 panic hook 和 `recovery failed` 写入）用 toast 说一次。

也就是说，即使真因还没找全，现在最多两次重启之内一定能进首页，而且屏幕上会写明上一次死在什么上。

## v52/v53：解密页的英文、下拉面板的角、USB 端口（2026-11-19）

真机：「自动解密面还会显示英文 adb 5000ms 什么的」，「下拉状态栏底部左右圆角透明与主体色不一致」。

### 那条英文：端口切换失败，原文被弹到页面上

`recovery.log` 里只有一行对得上，`8.491 s`：

```
the port did not take file transfer: sys.usb.state is Some("none") after 5000 ms,
not "mtp,adb"; config=Some("mtp,adb") state=Some("none") ffs.ready=Some("1")
ffs.mtp.ready=Some("0") configfs=Some("1")
```

`Pages::open` 里 `self.app.report(&format!("{e:#}"), ...)` 把这整句弹成了页面底部的 toast——
和上一版那条「上次启动怎么死的」是同一类错误：**给读日志的人看的话跑到了页面上**。
现在只进 kmsg，页面换成一句中文（`fmt::transfer_port_kept`）：

> 文件传输未能接入 USB，已退回只提供 ADB

但它不只是提示难看——**端口当时真的废了**。`usb::switch` 的走法是 `set("none")` 再 `set(目标)`，
`mtp.rc` 里有一条照抄 adbd 的规则：

```
on property:init.svc.adbd=stopped
    setprop sys.usb.ffs.mtp.ready 0
```

走 `"none"` 必然停掉 adbd，这条就把 MTP 的标志清了；而 MTP 服务用 `no_disconnect=1` 一直开着，
启动时置过一次 `1` 就再也不写。于是绑定规则
`ffs.ready=1 && ffs.mtp.ready=1 && config=mtp,adb` 永远凑不齐，`state` 停在 `none`：
**既没有 MTP，也没有 adb**。把这条规则删掉，标志交给 `skyward-mtp` 自己管（`Service::start` 置 1、
`end()` 置 0）；真失败时主动切回 `adb`，不让端口停在 `none`。

### 下拉面板的两个下角

`paint_shade` 的圆脚是四笔画出来的：一条矩形、左右两个圆、再一条补中间的矩形，颜色带 α238。
圆压在矩形上的那一块被**叠了两次**，α 从 238 变成约 253——比主体深一档。真机截图里量得出来：
主体 `(20,20,26)`，叠到的那圈 `(21,21,26)`，边缘外面 `(19,19,24)`，于是每个下角都有一道弧形接缝。
面板改成不透明：重叠处不再有差别，整块一个颜色，圆角也就是干净的圆角。

### 顺带：等触控面板的时限

`8.491` 那句 `no touch panel came up; the pages are worked with the keys` 比真正的面板出现
（`8.547 input /dev/input/event8: "Xiaomi_Touch_Input_0"`）**早了 50 ms**——`TOUCH_WAIT` 是 5 s，
面板用了 5.1 s。虽然第一次触摸就会退出按键模式，但那一下会先亮起一个焦点框。时限放到 10 s。

### 日志里另外看到的（这次没动）

| 现象 | 判断 |
| --- | --- |
| `qseecomd`（exit 255）与 `secure_element-service`（exit 1）**每秒被重拉一次**，一次开机上千次 | v46/v48/v50 的日志里都一样，不是这次引入的。`secure_element` 拿不到 eSE 就退出，属恢复环境常态；不影响解锁（`9.026 the decrypt page is done`）与备份，只是刷日志、每秒两次 fork+exec。要治得给它加退避 |
| `wlan0 is there 10.33 s after the driver was asked for it` | 无线驱动本来就慢，之后 `wpa_supplicant` 正常起来 |
| `the motor is not up yet`（3.2 s） | 马达节点 8.5 s 才出现，第一次点击时懒加载成功（`16.77 the motor came up`），不是故障 |

## v50/v51：解密页的英文提示、震动的强弱（2026-11-19）

真机：「自动解密的时候下面会出现一条英文提示」，「震动调节没有效果，一直是一个震感」。

**英文提示是自找的**：v46 加的那条「上次启动怎么死的」提示走 `App::report()`，也就是页面底部的 toast，
内容原样是 `/skyward/run/last-failure` 里那句英文（v46 那次的

```
the recovery ended on a panic at crates\skyward-ui\src\app\draw.rs:707:41: a page is drawn from its frame
```

）。这段是写给读日志的人看的，不该出现在刚回来的页面上。现在 `Pages::open` 只把它写进 kmsg：

```
the start before this one ended badly: <why>
```

页面上不再有任何一条英文。

**震动**：驱动这一侧其实一直在按上传的幅度走（`cs40l26_sine_upload`：`level = effect->u.periodic.magnitude`，
直接写 `BUZZ_EFFECTS*_BUZZ_LEVEL`，0–255 对应 0–10 Vpk），问题出在两端：

| 改动 | 说明 |
| --- | --- |
| 拖动时每跨一档就震 | 设置页与下拉控制中心的强度滑块原来只在**松手**时震一下，拖动过程中完全没有反馈，等于调的时候听不见。现在滑块分 20 档，跨过一档就按新的强度回一次马达，落手那一档不震 |
| 跨度 0.30 → 0.15 | `devices/popsicle.toml` 的 `[haptics] strength` 拉成 `{ lightest = 0.15, strongest = 1.0 }`，两端差 6.7 倍（默认 0.7 的位置约为满强度的 0.53） |
| 时长跟着强度走 | `haptics.rs` 里 `play_ms` 乘 `0.55 + 0.45 × share`：最轻约 11 ms，最重 20 ms。20 ms 的短促一下本来就只靠电平分辨强弱，长度差一倍更容易听出来 |

## v48/v49：进度条走完却进不了首页（2026-11-19）

真机日志（v46）：`8.62 s` 开始开分区，`9.05 s` 打出「the decrypt page is done; the menu shows」，
`9.28 s` 就是

```
the recovery ended on a panic at crates\skyward-ui\src\app\draw.rs:707:41:
a page is drawn from its frame
```

这是一条 100% 必现的自伤：`App::tick` 在解密页里为了转圈每帧把 `self.frame` 置空（页面快照
只在 `refresh()` 后重建），而条的最后一格正好也在同一段代码里判定——判完就 `leave_decrypt()`，
`leave()` 要用**当前页面的帧**做滑出快照（`nav.rs:37` 的 `paint_page(..., None, ...)`），
帧刚被自己扔掉，于是 panic。`catch_unwind` 把它接住、进程没死，但 `state.data` 已经被置成
`Data::Open`、栈还停在解密页，界面就永远停在那一页——**条是满的，下面还挂着「未记住密码 / 已记住密码」的开关**，
看起来像在问「要不要记住密码」。

| 改动 | 说明 |
| --- | --- |
| 先定去留，再决定放不放帧 | `tick` 里先把「条满 / 超时」判完；要走就保留帧交给 `leave_decrypt`，不走才 `frame = None` 让转圈继续 |
| 离开只在帧回来之后 | 判定加上 `self.frame.is_some()`：上一帧刚被扔掉就等本回合画完，下一回合再走（最多多一帧） |
| `decrypted()` 不再自己走 | 只置 `data_finish = Home`；`data_progress` 已经 0.999 时也交给 `tick`，避免「刷新完立刻离开」拿到空帧 |
| `leave()` 兜底 | 拿不到帧（或标题缓存）时用底色填一张，绝不 panic——页面宁可滑得难看，也不能走不掉 |

同一版补上**点按震动**：`Touch::Up` 命中控件后统一 `Effect::Haptic`，控件自己已经震过的不重复。
以前只有滑动确认、解锁、边缘返回手势会震，点按钮 / 行 / 开关一点反馈都没有（真机反馈：
「只有返回有震动，触摸屏幕还是没有震动」）。测试里 `tap_at` 统一滤掉这一下（跟 `slid` 一致），
三处直写 `app.touch(Touch::Up, ...)` 的断言补上 `Effect::Haptic`。

## v44/v45：卡在自动解密页（2026-11-19）

真机：刷完 v42/v43 停在「解密 data 分区」页，条走到头也不进首页，重启还是这一页。原因不是解密失败，
是**页面在等一件永远不会回来的事**：

| 等的东西 | 原来的行为 | 现在 |
| --- | --- | --- |
| 记住的密码交给安全世界核验 | 失败会回来（v43 已改成重新要密码），但被计入失败次数后那边会**压住半分钟以上**，任务不返回，页面就一直转 | 15 秒没回来就 `detach` 掉这次尝试（线程留着自生自灭，不 join 不卡界面），页面立刻改成手动输入 |
| 进度条最后一段 | 只有条到 100% 才切页 | 最多 0.7 秒，条没到也照走（`LAST_LEN_MS`） |
| 页面说「正在打开」但没任务 | 谁也发现不了 | 3 秒后重开一次，仍不行就报「打开 data 分区停了，请重试」 |

顺带修掉一个**一直存在**的坑：`remember::writable()` 只在 `/metadata` 已挂载时才 remount，
而 `read()` 读完就把它卸了 —— 于是 `forget()` 的 `remove_file` 落在挂载点的空目录上，`NotFound` 被当成
成功，真正的凭据文件还在分区里。结果就是「密码明明已经不对，每次开机还要再试一次」。现在没挂载就先挂、
改完再卸，删得掉才算删掉。

读到的凭据超过 512 字节（不可能是锁屏密码）直接当作没有。

## v42/v43：解密进度条按真实进度加回来（2026-11-19）

布丁 `v0.3.15` 的 `pages.rs::data_status` 里原本写着：

```rust
// No bar: the opening says nothing of how far along it is, and one drawn to
// a guess of its length only ever disagreed with the work (2026-11-16).
```

它删条的理由是「猜的长度跟做的事对不上」。这一版不猜：`skyward_crypto::data` 在真正耗时的每一步后面
打一个千分位点（`at(‰)`，`fetch_max` 保证只增不减），UI 读同一个原子量画条，所以条上的每一段都对应
一件已经做完的事，不会跟实际进度打架。

| 千分位 | 那一刻发生的事 |
| --- | --- |
| 40 | 页面开始打开分区 |
| 80 → 220 | `map`：读 fstab、解出存储密钥、`dm-create` |
| 250 | 探测文件系统 |
| 280 → 360 | 挂载（被内核拒绝时在这里插 `fsck`，检完再挂） |
| 400 → 470 | 共享密钥（`/data/unencrypted/key`）、各用户的 DE 键 |
| 520 → 680 | 逐个用户打开 DE（按用户数均分） |
| 690 → 700 | 手机替用户造出来的凭据（`open_made_up`），第一段结束 |
| 700 → 900 → 990 | 第二段：用记住的锁屏开房主自己的 DE、再带出造出来的那些 |

UI 侧的显示值永不回退：`data_progress` 向 `data_reported` 缓动（时间常数 0.35 s），没有新上报时按
3.5%/s 继续爬，但最多领先真实值 6%，到 99.4% 就停住等下一条；分区真的开了以后由 `data_finish` 接管，
0.45 s 线性补到 100%，然后才切页（`DataAfter::Home`）或显示「已解密」（`DataAfter::Open`）。
开机时手机自身部件（分区、驱动、服务）那一段由 `progress::done()` 折算成 0–35%，所以条从第一帧起就在走。

两段之间也留了个口子：`show_closed` 把页面设成「等锁屏」之后，如果还有记住的密码要接着自动解锁，
`App::data_still_opening()` 会把页面设回 `Opening`，条留在 70% 继续走，而不是先弹一个输入框再自己消失。

## v40/v41：把主仓库的补丁也清掉，只留布丁（2026-11-19）

v38 只换了 `skyward-ui` 与 `skyward/src`，其余 crate 还留着上游 `0.3.1` 之后的补丁。这一版把那 35 个
差异文件里的 28 个也换成布丁原版，`crates/` 下只剩 9 个文件与布丁不同（其中两个只是让 Windows 上能
编出打包器），都是这台机器出包 / 真机跑起来必须的：

| 文件 | 为什么留 |
| --- | --- |
| `skyward-ui/src/pages.rs` | 品牌编译期开关（`SKYWARD_BRAND_*`），不设时是「卑鄙坑Baby」 |
| `skyward-config/src/lib.rs` | Windows 上没有盘符的 `/data` 这类路径按 rooted 收下，否则打包器拒包 |
| `skyward-bootimg/src/ramdisk.rs` | 解包那半截是 Unix 才有的调用，`#[cfg(unix)]` 包住，Windows 上才能编打包器 |
| `skyward-programs/Cargo.toml` | `skyward-omapi` 只在 Unix 上依赖：它经 `skyward-block` 用到 `statvfs`，Windows 没有 |
| `skyward-programs/src/secure.rs` | 没有 omapi 可读时把服务名写出来当常量，配套上一条 |
| `skyward-crypto/src/data.rs` | 先挂载，内核拒了才 fsck：512 GB 的 data 全表扫要几秒 |
| `skyward/src/readings.rs` | 第一次就读全部、更新状态未知时每秒重读，避免「无法读取更新状态」挂十几秒 |
| `skyward-pack/src/main.rs` | 配置没写 recovery 哈希时先打印出来，方便对齐新固件 |
| `skyward-pack/src/mtp.rc` | 这台手机的原厂 init.rc 只认 adb/fastboot，MTP 要自己清 f1/f2 再绑 |

`devices/popsicle.toml` 与 `resources/popsicle/`（HAL 补丁层）也是本机的，不属于布丁那棵树。

更新检查也照布丁指回 `mariusarva/skyward-pudding` 的 releases。那个仓库发的是 recovery 镜像，
不是更新页要的每机一个 zip，所以这台手机的更新页只会看到版本号、下载不到包。


## v38/v39：整棵树与布丁 0.3.15 对齐（2026-11-19）

前面每一版都是「读布丁的补丁、挑功能往自己代码里搬」，搬漏一处就是一个页面不一样。这一版换了做法：
把布丁 `v0.3.15` 的补丁打到上游 `v0.3.1` 上，得到它那一棵树，**整块换掉** UI 与运行时，再把自己那几处
定制补回去。换完 `crates/` 下 14075 个文件里有 14039 个与布丁逐字节相同，剩下的 35 个是官方 0.3.1
之后的上游修复与下面这些定制。

换进去的东西（之前逐条抄时都没到位）：

- **`crates/skyward-ui/src/app/` 29 个模块**：`mod.rs` 2110 行是 App 本体，`pages`/`draw`/`input`/`nav`/`scroll`/
  `state`/`act` 分家，`update`/`wlan`/`manager`/`modules`/`dataapp`/`dataapp_list`/`filesystem`/`relabel` 各管一页。
  之前我们这份把 29 个模块合成一个 `app.rs`（13782 行），代数一多就落后。
- **文件页两套**：`Screen::Files` 与 `Screen::FilesOtg` 分开，U 盘多分区（0.3.9）才真正是一页一页的；
  `Effect::PasteMany` / `DeleteMany` / `ReadOtg` / `KeepSort` 对应批量粘贴、批量删除、读 U 盘分区、记住排序。
- **`crates/skyward/src/ui.rs` 4318 行**：布丁的驱动，`Effect` 分派、滑动确认、页间动画都在这里。
  `waveform.rs`（振动波形）与 `keys.rs`（secure element 服务）随之进来，`haptics.rs` 回到 265 行的薄壳。
- **`theme.rs` 回到布丁原样**：调色板只有 `accent`，深色/浅色都是 `rgb(74,128,247)`，我们之前额外加的
  `meter`/`switch_on` 两个字段没有了——布丁用 `accent` 一并画。
- **`avatar.rs` 229 行**：0.3.6 起的账户头像读取（`xiaomi_user_avatar_` + `zune-jpeg`），比我们那版多一条路径。

补回去的定制（只有这些）：

1. **品牌**：`pages.rs` 里 `brand(option_env!("SKYWARD_BRAND_TITLE|_WORDMARK|_AUTHOR"), 默认)`，默认
   `卑鄙坑Baby` / `叼着奶瓶逛酒吧` / `© 2026 卑鄙坑Baby`；官方版编译时给 `Skyward` / `Skyward` / `© 2026 Kirk Lin`。
   「Recovery 版本」一行也改成 `{HOME_TITLE} {RUNNING}`，不再写死 `Skyward`。
2. **更新源**：`skyward-net/src/github.rs` 的 `LATEST` 指回 `kirklin/skyward`——布丁把它指到了自己的
   releases，而那边发的是 recovery 镜像，不是更新页要的「每台机一个 zip」。
3. **解密提速**：`skyward-crypto/src/data.rs` 改成**先挂载，内核拒绝才 `fsck.f2fs -a` 再挂**。
   布丁这版每次挂载前都跑一遍全表检查，512 GB 的 data 分区要几秒；这条是纯提速，页面表现不变。
4. **更新状态快读**：`skyward/src/readings.rs` 第一次就读全部读数，`snapshot` 还是 `Unknown` 时每秒重读，
   读到就回 30 秒一轮。布丁这版仍是 30 秒一轮，开机那行会挂十几秒。
5. **Windows 出包**：`skyward-config/src/lib.rs` 的绝对路径校验改成「绝对 **或**（在 Windows 上）从根开始」。
   布丁的 CI 在 Linux 上跑，`/skyward/resonance` 这类手机路径在 Windows 上 `is_absolute()` 是 false，
   打包器会直接拒绝这份 `devices/popsicle.toml`。
6. **`system-init`**：`#[no_mangle]` → `#[unsafe(no_mangle)]`（edition 2024 的要求）。

**没跟的一处**：布丁的解密页没有进度条，`Data::Opening` 只画一个转圈。我们 v32–v35 那条
「千分位进度 + 走满才切页」随之去掉，页面回到布丁原样；解密本身因为第 3 条比布丁更快。


## v36/v37：同步布丁 0.3.10–0.3.15（2026-11-18）

上游 `mariusarva/skyward-pudding` 从 0.3.9 一路走到 0.3.15，这个仓库按功能逐条落到自己的代码上。

- **0.3.10**
  - 振动：`made()` 原来按「几个整周期 + 固定 8000 Hz」生成，改成**按 22 ms 时长、24000 Hz 采样率**生成（音圈马达在高采样率下更干脆）；驱动段 2 个周期、刹车段 1.5 个周期不变。
  - 所有 `Confirm` 滑动确认与滑动解锁在**滑到头**那一下补一次 `Effect::Haptic`；测试里的 `swipe()` 把它滤掉，剩下的仍是被确认的那个动作。
  - 锁屏充电显示功率：`Battery` 新增 `current`（µA，内核按充放电极性给正负）与 `watts()`，锁屏写成「95% · 充电中 · 63W」。
  - U 盘拔出时 `umount_detach` 返回 `EINVAL`（已经没有挂载点）不再 `?` 掉整个 recovery，改为写一行 kmsg。
- **0.3.11**（本版只统一了控件尺寸，文件操作弹出层未搬）
  - `widgets` 新增 `PILL_DP/PILL_PAD_DP/PILL_TEXT_DP/PILL_GAP_DP` 与 `pill()/pill_sized()/pill_width()`，新页面统一用它。
- **0.3.12**
  - `download_progress` 不再只在版本更新页才 `refresh()`：下载与校验阶段可以离开页面，任务照跑，首页「版本更新」那一行跟着走。
- **0.3.13**
  - 新页面 `Screen::Modules/ModuleInfo`：读 `/data/adb/modules/*/module.prop`（`disable` = 关、`remove` = 下次启动移除），列表带开关与「已关闭 / 下次启动时移除」，详情页有开关与卸载；卸载走 `ask_sheet` 滑动确认。
  - 文件页的 zip 多一条「作为模块安装」：按 zip 里最短路径的 `module.prop` 取 `id`，整包解到 `/data/adb/modules/<id>`（已存在先清掉即更新），`.sh` 给 0755。
- **0.3.14**
  - 新页面 `Screen::DataApps/DataAppInfo`：扫 `/data/app`（含 `~~` 两级）与 `/data/system/packages.xml` 的 `first-install-time`，缺记录时用 apk 的 mtime 兜底；一屏一页、按安装时间从新到旧；名称与图标在后台线程里读 APK 的 AXML/ARSC（读不到就退回包名），解不出的应用统一显示同一个图标。
  - 卸载只删 `/data/app` 下的那一套与 `/data/data/<包名>`，系统分区的不给删；同样走滑动确认。
- **0.3.15**
  - 首页 WLAN 行在「开电台」阶段就写「正在连接」。
  - WLAN 驱动预热从「模块全部装完」提前到「固件挂载后立刻」，省约 0.9 秒。
  - 应用管理右上角「系统应用」→ 只读页：读 `/mnt/system/system/app|priv-app`、`/mnt/product/*`、`/mnt/system_ext/*`、`/vendor/app`、`/odm/app`，按包名排，分页，不可点、不可卸。
- 版本号 `0.3.7 → 0.3.15`。


## v34/v35：进度条走满才切页；更新状态一秒一读（2026-11-16 夜）

### 一、条没走完就切页

解密是两段：`open_without_credential`（分区本身，原来自己报到 1000‰）和 `unlock`（用户的凭据）。
第一段一结束，页面立刻切到「已解密」或锁屏输入，条停在半路；自动解锁那一段页面根本不是解密页，条也就不画了。

现在：

- 第一段只报到 **720‰**：映射 180 / 挂载 420 / 共享密钥 540 / 各用户 660–700 / 返回 720；第二段 730 → 880 → 1000；
- 有「记住的密码」时**不切页**：`Data` 保持 `Opening`，页面继续写「正在用记住的密码解密」，条从 720 接着走；
- 工作真正结束时不再立刻改状态，而是记下 `App::data_end`（`DataEnd::Waiting` / `Done`），`tick` 里把条以约 0.3 s 的固定步长补到 1.0，**满了才**显示已解密、问锁屏或离开页面；
- 两段之间条也不停：它以 4%/s 缓爬，最多领先最近一次上报 8%（安全世界核对锁屏那一步要一两秒，停了看着像卡死）；
- 条还没补满就离开页面（点「暂不解密」或翻到别的页）时，`tick` 收尾时把挂起的结果落下，不会把 `Data` 永远留在 `Opening`；
- 手动输密码那次不算：页面是键盘页，本来就没有条。

### 二、更新状态挂十几秒

`readings` 线程的「全部读数」原来是 30 次电池读一次，也就是 **30 秒**；而 `/metadata` 是 prepare 线程开头就挂上的。
第一次读到 `Unknown` 之后，那行要等到 30 秒后的下一轮才消失。

现在第一次读数就是全部，并且**只要更新状态还是 `Unknown` 就每秒重读**，读到真正的状态再回到 30 秒的节奏。
另外准备完成前（`state.prepared == false`）首页不显示「无法读取更新状态」那一行：那时候 `/metadata` 还没挂，读不到是正常的，不是错误。

## v32：解密提速，同一份代码出两个品牌（2026-11-16 夜）

### 一、慢在哪：每次进解密页都先扫一遍文件系统

`data::open()` 的顺序原来是「映射 → 探文件系统 → **跑 `fsck.f2fs -a`** → 挂载」。那是 `fs_mgr` 在开机时的顺序，代价是**把 data 分区的全部表走一遍**：512 GB 的分区上这是几秒，而这几秒里页面只有环在转。

现在反过来：先按手机自己的选项 `mount`，**只有内核拒绝挂载**（文件系统确实不一致）才跑那遍检查，检查完再挂一次并把结果记进内核日志。干净的分区一次挂上，省掉整遍扫描；有问题的分区仍然走完整流程，工具页的「修复文件系统」也没动。

`Opened.checked` 因此多半是 `None`，`log_mounted` 照旧只在有的地方打日志。

### 二、进度：5 档 → 千分位

原来是 `STEP: AtomicU8`，只有 0/0.2/0.4/0.6/0.8/1.0 六个值，页面只能靠 `tick` 里的渐近值假装在走。现在 `STEP: AtomicU16` 记千分位，`at(‰)` 在各个真实节点上报：

| 千分位 | 位置 |
| --- | --- |
| 20 | 进 `open_without_credential` |
| 180 | `dm` 映射建好（密钥解出来给了硬件） |
| 420 | 挂载完成 |
| 540 | `/data/unencrypted` 的共享密钥装好 |
| 660 → 880 | 每个用户的 DE 键，按人数均分 |
| 900 | 手机自建凭据那一轮 |
| 1000 | 返回 |

解锁那一段用 `data::resume(0.62)` 接手：**不再 `restart()`**，条不会被打回 0，也不会停在 90% 干等「自动解锁」跑完。

### 三、等待期的爬升降下来

`tick` 里那段「没进度时报个渐近值」原来是 `0.9 * (1 - e^(-t/2.4s))`——两秒多就爬到 90%，真实进度（那时只有 5 档）永远追不上，看着就是卡住。现在 `0.35 * (1 - e^(-t/0.9s))`：只在最开始动一下，真实进度一上来就接管。

### 四、两套品牌，一份代码

`pages::brand(option_env!(…), 默认值)` 把三个字串变成编译期可指定：

| 环境变量 | 默认（本 fork） | 官方版 |
| --- | --- | --- |
| `SKYWARD_BRAND_TITLE` | `卑鄙坑Baby` | `Skyward` |
| `SKYWARD_BRAND_WORDMARK` | `叼着奶瓶逛酒吧` | `Skyward` |
| `SKYWARD_BRAND_AUTHOR` | `© 2026 卑鄙坑Baby` | `© 2026 Kirk Lin` |

`option_env!` 会被 cargo 记进指纹，改环境变量就会重编 `skyward-ui`，所以两个包是同一份代码编译两次，不是两份源码。官方那三个字串是从官方 0.3.7 的 `system/bin/recovery` 里读出来的原样（`© 2026 Kirk Lin`）。

### 五、本次交付

| 包 | ramdisk | sha256 |
| --- | --- | --- |
| `v32-skyward-popsicle-OS4.0.6.0.XPBCNXM-5bbb43c-brand.img` | 65,848,070 | `60a5c85ae6a6d79cc54c26750b53735d546ac02fe49f3379b505983255fb318a` |
| `v33-skyward-popsicle-OS4.0.6.0.XPBCNXM-5bbb43c-official.img` | 65,848,081 | `22ea0e2e14270803458ac8df0039e1ecc073f9af9aff7aa9d4ca4a74afebe1b2` |

两个包都 104,857,600 字节、1279 条目、15 项关键文件、AVB 自洽、`ALL CHECKS PASSED`。镜像内核对：v32 有 `卑鄙坑Baby` 2 处 / `叼着奶瓶逛酒吧` 1 处；v33 这两串为 0，`Skyward` 101 处、`© 2026 Kirk Lin` 1 处（两个品牌字串相同被链接器合并成一份）。

## v30：位置胶囊贴在两端、首页头像（2026-11-16 晚）

### 一、为什么会有两个「内部存储」

v29 修的是「位置胶囊压在路径下面」，做法是**在路径条之外再加一组**「内部存储 | U 盘」，却没把路径条自己那枚位置胶囊去掉。路径条的 `names[0]` 就是位置名（`strip_room(t, f, top, …)`，`top` 传的是「内部存储」/「U 盘」），于是同一行里画了三次：路径条的位置胶囊、控件的第一段、控件的第二段。

v30 让路径条不再负责位置：`files::strip_with_sort` 自己画两枚胶囊，一枚贴行首（内部存储）、一枚贴行尾（U 盘），顺序与位置都固定，当前那枚用 `pal.here` 点亮、另一枚灰底可点。仍在 `Place` 里时点当前那枚等于回该位置的根目录（`Action::OpenAncestor(0)`，只有进了子目录才有这一下）。

文件夹里的路径（面包屑）排在两枚胶囊之间：

- 名称取自 `folder` 的 `Component::Normal`，**不含位置名**（位置已经是左边那枚胶囊）；
- 命中仍然是 `Action::OpenAncestor(k + 1)`——`k` 是 `below` 的下标，`ancestor()` 数的是路径里真实的目录名，位置名不在其中；
- 放不下时从**最早**的一级开始折叠（`shown.remove(0)`），折出来的「…」占一枚灰胶囊、不响应点击；折叠后仍比可用宽度宽（名字本身太长）时宁可不画路径，也不让它钻到「U 盘」底下。

### 二、首页那张头像

官方 0.3.7 的首页账户卡是一张**真人头像**，我们的是一枚灰圆片 + 电话图标。从官方 `system/bin/recovery` 里能翻出它读的地方：

```
/data/data/com.xiaomi.account/files
xiaomi_user_avatar_
the account's picture at … is not read: …
```

也就是说官方读的是**手机里小米账号自己的头像文件**（账号 App 写的那张），再用 `image` / `zune-jpeg` 解码。我们照做：

- 新增 `crates/skyward-ui/src/avatar.rs`：先看 `/data/media/0/.skyward/avatar.{png,jpg,jpeg}`（自己放一张的口子，`/sdcard` 也认），没有再读账号 App 目录里前缀为 `xiaomi_user_avatar_` 的文件、取修改时间最新的那张；
- 解码按文件头分辨：PNG 走已有的 `png` crate（`EXPAND | STRIP_16`，索引色/灰度都摊成 RGB），JPEG 走新加的 `zune-jpeg`（纯 Rust，官方用的同一套，`ColorSpace::RGB` 输出）；
- 读进来先缩到最长边 160 px（`shrink`，块平均）——页面只画 34 dp，原图动辄上千像素，没必要一直扛着；
- `Canvas::fill_circle_picture`：按「短边铺满直径」缩放、居中裁切，逐像素双线性采样，圆边用和 `fill_circle` 同一套覆盖率做抗锯齿，所以头像的边缘和别的圆片一样干净；
- `widgets::device_card` 多一个 `picture: Option<&Picture>`：有就画照片，没有仍然是灰圆片 + 电话图标（机型号那张卡永远是电话图标，官方也是）；
- 账号目录还没挂上时读不到：`picture()` 用 `OnceLock` 只缓存**成功**的那次，失败按 3 秒节流重试，避免数据分区还没打开时每帧都去 `read_dir`。

### 三、v29 的另一半：为什么还显示两个

v29 的截图里三个胶囊**都是琥珀色**，因为路径条把「当前位置」当最后一级点亮，控件又把自己的当前段点亮——两套都在说「你在这儿」。v30 只有位置那枚胶囊说话。

## v29：位置胶囊、版本号与进度条回跳（2026-11-16 晚）

### 一、「内部存储」只剩一个「内」

文件页右上那两个胶囊，原来是「一个路径胶囊 + 一个‘去另一个位置’胶囊」，两段都右对齐：路径用屏幕右边算位置，右侧胶囊也从屏幕右边算，于是叠在一起，后画的「U 盘」把「内部存储」盖掉大半，只露出最左边一个字。

改成固定的两段控件：

- 顺序永远 `内部存储 | U 盘`，当前所在的那段用琥珀色，另一段灰底；
- 两段整体靠右，中间 4 dp 缝，各自圆角；
- 当前那段不响应点击（页面就在这儿），另一段切过去；
- `strip_room` 的 `x0` 从「顶到屏幕右边」改成「顶到调用者给的 `room` 右边」——路径从此在自己那半边里右对齐，不会再钻到胶囊底下。

### 二、版本更新页写着 0.3.1

功能早就同步到布丁 0.3.7，`Cargo.toml` 的 workspace 版本还停在 0.3.1。改成 **0.3.7**，更新页和关于页都显示 0.3.7。

更新**源**没动，还是 `kirklin/skyward`：布丁的 release 是 pudding（小米 17）的包，指过去只会让 17 Pro Max 下到别的机型的镜像，不能这么干。

### 三、进度条到后边来回跳

v28 已经改成「平滑追目标」，但 `set_data_progress` 里还留着 v27 的一句话：

```rust
if progress < self.state.data_progress { self.state.data_progress = progress; }
```

真实进度是 `STEP/5`（0、0.2、0.4…），而条在等待阶段已经按渐近线爬到 0.6~0.9。于是每帧：

1. `set_data_progress(0.2)` 把条一把拽回 0.2；
2. `tick` 又把它平滑推向 0.6+；
3. 下一帧再拽回 0.2 —— 在目标与当前值之间来回跳。

删掉那一句，`tick` 里再补一道「目标落后就不动」，条从此只增不减。

## v28：品牌名回退 + 解密页进度条改回叠画（2026-11-16 晚）

两条真机反馈，都是 v27 的回归。

### 一、首页大标题

v27 照着官方截图把标题改成了 `Skyward`。品牌是本 fork 的，官方截图只用来对布局和控件，名字不跟。改回 `卑鄙坑Baby`（`pages::HOME_TITLE`，v12 起就是它）。

### 二、进度条「卡顿」的真实原因：它根本没在动

v27 把环和条从叠层搬进了 `pages::data_status`，由页面自己画。但页面是**布局一次就缓存**的：`App::render` 只在 `frame == None` 时重新 `pages::render`，`tick` 每帧只置 `dirty`（触发重绘），不会让页面缓存失效。于是条和环被固定在第一次布局的那一帧上，只有 `refresh()`（准备状态变化、真实进度上报）时才跳一下 —— 看起来就是一顿一顿。

改回上游 fork 本来的做法：

- `pages::data_status` 不再画环和条，`bar` 参数整个删掉；
- `App::paint_preparing` 恢复，每帧在页面之上叠画环（r=48 dp）与条（240×8 dp @ cy+126 dp），尺寸沿用 v27 对截图的那一套，所以样子不变；
- `tick` 每帧只改 `data_phase` 和 `data_progress`，叠层跟着走。

### 三、进度本身也不再「跑满干等」

v27 用固定速度 `DATA_RATE` 把条在 5.9 秒内推满，然后停在那儿等真正解密结束 —— 即使帧率正常，看着也像卡住。现在：

- 等待阶段（分区/服务还没起来）按 `0.9 * (1 - e^(-t/2.4s))` 渐近，永远不到头；
- 真实进度由 `skyward_crypto::data::progress()` 每帧上报，`target = max(真实, 渐近值)`，条不会倒退；
- 每帧按 `1 - e^(-dt/100ms)` 追 `target`，`dt` 是上一帧到这一帧的实际毫秒数，所以 30 帧和 60 帧的视觉速度一致，不会因为掉帧变成一跳一跳。

离页时 `data_started` / `data_step_at` / `data_progress` / `data_target` 一起清零，下次进来从头跑。

## v27：按官方 0.3.7 的九张真机截图逐页对齐（2026-11-16 晚）

这一版不看二进制了，看九张真机截图。上一版（v26）是从官方镜像里比中文文案，只对上了**字**，没对上**位置和颜色** —— 截图一放出来，差得最远的三处一眼可见。

### 一、深色主题原来是灰的，官方是蓝的

v22 按「深色下不要蓝」把 DARK 的 `accent` / `fill` / `meter` / `switch_on` 全换成白与灰。官方 0.3.7 的深色是**黑底 + 同一支蓝** `#4A80F7`：开关（图 3、图 5）、进度条（图 2、图 6）、滑动解锁的钮（图 9）、控制中心的亮度和振动（图 8）都是这支蓝。

改回来之后 `slider_knob` 也得跟着改：原来深色返回页面底色（那时 fill 是白），现在 fill 是蓝，圆点必须是白的。

文件页那颗琥珀胶囊（`files::palette` 的 `here`）本来就和官方对得上，没动。

### 二、首页差四处

对着图 1：

- 大标题官方是 `Skyward`，本机是「卑鄙坑Baby」；
- 官方**没有搜索栏**，标题下面直接是卡片；本机顶着一整条搜索框；
- 官方账户（`Mari0us`）和机型（`Xiaomi 17`）是**两张卡**，本机挤在一张卡里；
- 最后一行官方叫「选项设置」，本机写成「恢复设置」—— 当初抄错的字。

`device_card` 拆成两张：第一张只有账户行，第二张从 `account_row_h + card_gap` 起画机型行，机型那行的圆盘用 `Icon::Phone` 小一号（0.72），跟官方一样是灰盘不是彩色牌。返回的 `rows[0]` / `rows[1]` 都打开「关于本机」，后面才是会变的那几行。

搜索栏删掉之后 `Screen::Search` 没有入口了，页面本身留着（`fold` 为 `None` 时那条 `search_capsule` 不再执行）。

### 三、高级工具的顺序

图 7 是「解密 / 挂载 / 终端 / 文件系统 / 无线传输 / 网络下载 / AVB 校验 / 系统更新 / 导出日志 / **U 盘管理** / 修复 SELinux」。本机把 U 盘管理放在网络下载后面了。挪到导出日志之后，图标也从蓝改紫。

顺带确认一件事：官方这一页是**单张卡 11 行，没有分组**。v26 里写着「没法从二进制确认是否分组」，截图证实没分。

### 四、解密页：一条进度条，不是零条

v19 出过两条，v20 的办法是把页面那条传 `None`，只留叠层的 200×4 细条。看图 6 才知道官方是**页面自己画一条** 240×8 的，位置在副标题下 28 dp。

所以这次反过来：`paint_preparing` 整个删掉，`Data::Closed | Opening` 传 `Some(data_progress)`，环和条都由 `data_status` 画，位置和粗细按图 6。v19 的病根是两处都在画，只删一处就对了。

同时 `data_status` 的 `back` 从 `false` 改回 `true` —— 图 3、图 6 顶上都有 `<`。

### 五、已解密页那张卡

图 3 在「你可以在「文件管理」中查看本机文件」下面有一张卡：紫底锁 + 已记住密码 + 蓝开关。`data_status` 加了个 `remember: Option<bool>`，只在 `Data::Open` 传值，卡片画在 `sign_cy + 168 dp`，点它走 `Action::Remember`（关掉直接关，开启弹风险窗）。

### 六、文件管理页是两行

图 4 的行一是「内部存储」（琥珀，当前路径）和「U 盘」（右侧），行二是「修改时间 / 降序 / 文件夹置顶 / 多选」。本机把两处位置和多选全挤在排序行右边，窄屏上会被挤掉。

`strip` 拆出 `strip_room(..., room)`，路径按给定宽度折中间；`strip_with_sort` 改成两行：行一是路径 + 另一侧位置（`away`），行二是排序三枚 + 页面自己的胶囊（`moves`，从左排，放不下就少画一枚）。

### 七、更新页与控制中心

图 2：「当前版本 0.0.30」卡片上面**没有「本机」分组**，「本次更新」在**停止按钮下面**。删掉 section，notes 从 `skyward_online` 里搬到页面尾部。

图 8：两个滑块是**并排的大胶囊**（约 180×44 dp），图标在左、白圆点在填充末端，不是本机上下两条 7 dp 细线。重画；面板高从 508 降到 464 dp（两行磁贴 128 + 224、胶囊 44、脚 30）。

磁贴颜色 `accent.with_alpha(72)` 跟着变蓝，正好是图 8 里 WLAN 开启那块的深蓝。

### 八、锁屏

图 9 是纯黑。本机原来铺的是 `rgb(6,6,8)` 加 alpha 226，页面的光晕会透出来。改成不透明黑。

### 没动的

- 状态栏的时间、网速、温度是系统的，不是 Skyward 画的；
- 首页图标配色（蓝 WiFi、绿安装、蓝备份、紫恢复、紫双系统、灰高级/设置、橙重启、红清除、蓝版本更新）逐条对过，本来就一致。


v23–v25 把功能搬过来了，界面还剩几处和官方 0.3.7 不一样。这一版把官方镜像里的 `system/bin/recovery`（9,637,952 字节）抽出来，和本机的按「中文文案集合 + 最长可匹配子串」比了一遍：官方 1412 条、本机 1432 条，真差异 28 块约 1000 字，逐块看过之后改的是下面这些。

### 粘贴栏原来是两条，官方是三条

官方 `files()` 的底栏字面量是连续的三个：`复制到此处` / `移动到此处` / `取消`。本机按 `clip.op` 只画一条 —— 也就是说，在文件信息页点了「复制到 U 盘…」，到了 U 盘只能复制，想改成移动得退回去重来。

现在两条都画，`Action::Paste(FileOp)` 带操作，`dock_bar_at` 把当初选的那个点亮。`Effect::Paste` 的 `op` 就用按钮上的，同名冲突（`Action::Resolve`）仍用冲突自己记下的那个，不会串。

### 其余文案

- 文件页右上胶囊：`选择` → `多选 / Select`。官方只有「多选」这一条，进入选择态后不变字，靠高亮表示，再按一下退出（`Action::ChooseMode` 本来就是取反）。
- 未插盘提示：`请通过 OTG 连接 U 盘` → `请先连接 U 盘 / Connect a USB drive first`。
- 盘里没东西：官方 U 盘页单独说 `U 盘为空`，内部存储才是 `暂无文件`。
- 记住密码风险窗：换成官方整段 ——「开启后，锁屏密码会保存在本机，每次开机自动解密 data 分区。任何拿到这台手机的人都能借此读取其中全部文件，丢失、被盗或送修时尤其危险。开启即表示你接受这一风险。」（繁中、英文同改）
- 设置页：0.3.7 把「解密」分组删了（用户清单第 8 条也是这么写的），开关只剩解密页键盘上方那颗胶囊。本机跟着删，`REMEMBER_LOCK` 常量一并去掉。
- 同名冲突页：`"{into}"中已有同名的"{name}"。` → `该文件夹中已有"{name}"。`
- 版本更新页的更新日志分组：`更新内容` → `本次更新`。
- 文件信息页的跨位置四行：官方是四条独立文案（`复制到 U 盘…` / `剪切到 U 盘…` / `复制到手机…` / `剪切到手机…`），本机原来是 `复制到{to}…` 模板拼的，中文拼出来是「复制到U 盘…」，`U` 前少了空格。改成四条字面量。
- AVB 页滑完写回后回一句 `已写入：逐块校验 开/关，启动校验 开/关`（官方 `已写入：逐块校验 {}，启动校验 {}`）。新增 `App::avb_written`，`Effect::AvbSet` 走它，读状态仍走 `avb_ready`，免得打开页面也冒这句。

### 顺手修的

`skyward-preview` 从 v23 起就编不过：`file_clash` 第二个参数变成 `Vec<String>`、`Newest` 多了 `notes`。本机只跑 `ccheck.py -p skyward -p skyward-ui` 所以一直没暴露，全工程检查才报出来。两处都补上了。

### 核过但没动的

- 首页三段（无线网络+文件管理 / 安装刷机+创建备份+恢复备份 / 第二系统+高级工具）、页脚（重启系统+清除数据+版本更新）与官方一致。
- 设置页分组 `语言 / 亮度 / 振动 / 通用 / 文件传输 / 调试 / 关于` 与官方一致。
- 官方 0.3.7 的 `MTP`、双系统、`payload.bin`、`avbctl`、`wpa_supplicant`、`sideload` 等能力字串计数与本机相同 —— 没有整块功能缺失，570 KB 的体积差是编译选项（本机 `opt-level` 更低）而不是少代码。

产物 `out/skyward-v26-popsicle-OS4.0.6.0.XPBCNXM.img`，104,857,600 字节，ramdisk 65,723,830 字节压缩 / 1279 条 / 15 项关键文件 / AVB 自洽 / ALL CHECKS PASSED，
sha256 `894f30ec2827cd9fd66f060b39e8e4ee2255b3543a16fa9711e7eaa1c4026af6`。提交 `213660d`、`340ea01`。

## v25：下拉面板跟手、脚角发灰、振动滑块的力度（2026-11-16 晚）

真机反馈三件事：滑块拉到哪都是一样的震动；控制中心下拉不跟手；面板脚角有两个灰色小三角。
三件都是代码里的实错，不是手感偏好。

### 一、下拉面板的比例搞错了单位

```rust
let pulled = from * SHADE_PULL_DP + (y - start);   // SHADE_PULL_DP = 110
```

`Touch` 的 `y` 是**屏幕像素**，`SHADE_PULL_DP` 却当成同一个单位直接加。440 dpi 下
110 px ≈ 40 dp，而面板有 508 dp 高 —— **手指动 40 dp，整块面板就「啪」地全出来**，
跟手比例 12.7 : 1，所以怎么拉都像跳出来的，松手又一下弹回。

改成 `t.m.dp(SHADE_PULL_DP)`，并把「拉满所需的距离」取面板高度的一半（254 dp），
约 2 : 1；`shade_settle` 里存的比例（0/1）而不是像素，`tick` 不再需要 dp。

顺手把松手动画从 1.8 s 的 smoothstep 换成 0.34 s 的 ease-out：起手快、收尾软，
是「把手指没拉完的那段接完」，不是一次慢吞吞的漂移。甩开的余速按行程的 8% 折算，
面板多高都跟手一致。

### 二、脚角那两个灰色三角

面板原来用 `rgb(20,20,26).with_alpha(238)` 画，拼法是「矩形 + 两个圆 + 底条」：

- `fill_rect` 是**覆盖写**，像素直接写 `[20,20,26,238]`；
- `fill_circle` 内部走 `span`，颜色带 alpha 时是**混合**，238/255 的黑混在白页面上 ≈ `#232328`。

于是面板主体是纯黑，只有圆的下半截（不与矩形重叠的那部分）是 `#232328` ——
正好就是左右脚角两个小三角。

改成不透明的 `#18181E`：`span` 走 `color.a == 255` 的直写快路径，整块一个颜色，
三角没了，混合也省了。fork 原来的 238 只透出十五分之一，肉眼看不出来。

### 三、振动滑块没有力度反馈

`shade_touch` 的 `Touch::Up` 里，滑块松手只做了 `self.drag = Drag::Idle` 就 `return fx` ——
**控制中心的振动滑块拉完根本不震**。用户听到的震动全来自别的按钮（都用默认 0.7），
当然「都是一个震动」。

现在：

- 拖动每跨一档（0..1 分 10 档）按**当前档的强度**回震一次，越往右越重；
- 松手再震一次，确认落在哪一档；
- 设置页的滑块共用 `set_level` 的返回值，行为一致；
- `lightest` 0.45 → 0.30，两端差 3.3 倍，0.7 处仍是 0.70 满强度，默认手感不变。

### 四、每帧一条 stderr

```rust
eprintln!("frame {:?} in {} ms", app.screen(), t0.elapsed().as_millis());
```

拉面板时每帧一次 `format!` + 一次写 stderr（`/dev/kmsg` 或串口），比画这一帧还贵。
改成只打超过 120 ms 的慢帧，诊断能力保留，快帧不再付出代价。

## v24：v23 出包后复查出来的四处（2026-11-16）

v23 的镜像做出来后，把新写的多选与互传逻辑逐条走了一遍**代码路径**（不是只看编译过），
发现四处错，其中一处让「复制到 U 盘」根本走不到第二步。

### 一、粘贴栏永远不出现（功能断链）

`Action::PickMarked` 把 `clip` 装好、位置也切到了对面，**但没有关掉 `state.choosing`**。
文件管理页的底栏是三选一：

```rust
if s.picking { … } else if s.choosing { 全选/复制/移动/删除 } else { clip 的粘贴栏 }
```

`choosing` 还是 `true`，所以切到 U 盘后底部仍然是多选栏，用户看不到「复制到此处」。
现在选中即退出选择模式，切过去就是粘贴栏。

### 二、批量删除一按就没了（无确认）

单条删除是「滑动以删除」（`Confirm::DeleteEntry`），批量删除原来直接
`Effect::Delete(paths)` —— 一顿多选、手指一滑，整个文件夹就没了，且没有回收站。
现在走同一套滑动确认：

- `remember_layout` 抽成通用的 `ask_layout(c, t, f, s, confirm, 标题, 代价, which)`，
  外面套 `ask_sheet(...)` 得到贴底窗口（记住密码与批量删除共用，`Action` 的「取消」由调用方给）。
- 新增 `Confirm::DeleteMarked`、`Action::KeepMarked`、`State::asking_marked`。
  文件管理页在 `asking_marked` 时把底栏换成这个窗口，标题「删除选中的文件？」，
  正文「将删除 N 项，删除后无法恢复。」。
- 返回键先关这个窗口，不会顺手退到上一层。

### 三、「全不选」标签下按的却是「全选」

底栏第一个胶囊在全部选中时显示「全不选」，但 `hits` 里写死 `Action::MarkAll` ——
按下去等于再全选一次，永远退不出来。现在标签与动作一起决定（`MarkAll` / `MarkNone`）。

### 四、排序行空间不够时先丢「内部存储」

`strip_with_sort` 的 extras 用 `extras.iter().rev()` 从右往左排，空间不够就 `break`：
先被丢掉的正好是数组第一个，也就是最该留的「内部存储 / U 盘」。改成先算总宽、
按传入顺序保留（多出来的从尾部丢），再整体右对齐绘制。同时底栏按钮加了「有没有事可做」：
未选中时复制/移动/删除三个胶囊变灰（`widgets::dock_bar_live`），点了提示「请先选中文件」，
不再是一按没反应。

## v23：把布丁 v0.3.7 的 U 盘与批量文件操作搬过来（2026-11-16）

用户拿来一份「小米 17 第三方 Recovery v0.3.7：相对上游 0.3.1 的 63 处改动」清单，要求把这些
改进做进 17 Pro Max 的镜像。**先核对了清单和本仓库，六成以上早就有**（明文 data 兜底、
`dm::status` 后 remove、跳过 999 分身、`weaver_first`、控制中心四磁贴与亮度/振动滑块、
`fileserve`、`portal`、`AVB` 两开关、界面优先启动……），真正缺的是用户看得见的三块。

### 一、文件管理与 U 盘（新增）

- 页面顶部「内部存储」「U 盘」两个胶囊并排，点一下切换位置。**两个位置各留各的面包屑**：
  `state.files` 与 `state.files_drive` 各存一份路径（从各自顶端算），
  绝对路径由 `app::place_path()` 现算。
- 「顶部」这个说法背后是一条紧箍咒：`/sdcard` 与 `/mnt/usb` 是两棵不同的树，
  跨位置粘贴必须换算。原来 `Action::Paste` 直接拿 `/sdcard/x` 往 `/mnt/usb/...` 里塞，
  就是布丁清单里说的「原实现会拼错」。现在一律走 `place_path` / `place_relative` / `place_of`。
- 多选：排序行右侧多一个「选择」胶囊，进入后行首画圈，勾上的是实心加白勾，
  底部换成「全选 / 复制 / 移动 / 删除」。选择模式下点行是勾选，不是打开——这条靠
  `listing()` 换 `action` 实现（`Action::Mark` 与 `Action::OpenEntry` 二选一）。
- 长按条目（文件信息页）多两行：「复制到 U 盘… / 剪切到 U 盘…」，在 U 盘上则是「到手机…」。
  两步式：先跳到目标位置，随便浏览到哪个文件夹，底部「复制到此处 / 移动到此处」落地。
  U 盘没插时不显示这两行（`State::place_open`）。
- 高级工具加「U 盘管理」，直接打开文件管理器的 U 盘页。
- 面包屑过深时保留首尾、中间折成一个「…」胶囊：原来是把整串往左顶出去，
  左边的胶囊直接画到屏幕外。

### 二、U 盘挂载（改）

`skyward-block/src/usb.rs` 的 `mount()` 原来按 `ro` 挂，**往里复制必然报
`Read-only file system`** —— 布丁清单第 9 条说的就是这个。改成 `rw`，
并在挂载前先 `umount_detach`：上一次进程崩溃留下的挂载点还在，直接往上挂就是
「显示的是上一个 U 盘的文件」。

### 三、记住密码（改）

原来解密页键盘上方只有一行小字「● 记住密码，下次自动解密」，点哪儿都行，也没有任何告知。
现在：

- 输入密码后出现胶囊，写着「已记住密码」或「未记住密码」，点一下切换。
- 从「未记住」切到「已记住」会弹悬浮窗（`Dock`，贴着屏幕底部），说明密码**原样**存在
  `metadata` 分区的 `skyward/credential`，能挂载该分区的电脑都读得到；**要滑动才生效**。
  每次重新开启都会再问一次，不记「提示过」。
- 关掉是立即生效，不弹窗。
- 设置页的同一个开关走同一套确认。窗口由 `pages::remember_sheet()` 画，
  先量一遍高度再按该高度画第二遍——贴底的窗口是由自己的高度决定位置的。

### 四、零碎

- 版本更新页解析 GitHub release 的 `body`，取最多 4 行、每行最多 64 字（超出加「…」），
  列在版本号下面。`skyward-net/src/github.rs` 的 `Newest` 多了 `notes`。
- panic 钩子把「the recovery ended on a panic at 文件:行:列: 原因」写进内核日志，
  重启后 `dmesg` 还能看到。装在 `ui::run()` 最前面。
- 文件操作写内核日志（`files: Copy 源 into 目标 (N carried)`），批量任务的进度页显示
  「X 等 N 项」。

### 没取的

- **更新检查改指向 `mariusarva/skyward-pudding`**（清单五.8）：那是小米 17 的仓库，
  本机是 17 Pro Max，指过去会推来装不上的包。仍指 `kirklin/skyward`。
- **v0.3.1 那套 `app/` 27 模块拆分**：本仓库结构停在 0.3.0（`app.rs` 单文件），
  整体搬的代价大于收益，仍按 [SYNC-UPSTREAM.md](SYNC-UPSTREAM.md) 第七、八节逐条挑。

## v22：深色主题里不再有蓝色（2026-11-15）

v21 只把默认值改成深色，页面黑是黑了，但凡是「强调」的地方还是那片蓝：按钮、开关打到开时的
轨道、解密页的锁图标、进度条。用户要的是「主题色改成黑的」，所以在深色调色板里把蓝色整个去掉。

做法不是逐处改色，而是把调色板里原本一个 `accent` 承担的两件事拆开：

| 字段 | 用途 | 浅色 | 深色 |
| --- | --- | --- | --- |
| `accent` | 墨：链接、选中的字、图标、勾、光标、聚焦光 | `#4A80F7` | `#EDEDED` |
| `fill` / `on_fill` | 会动作的控件填色与上面的字：按钮、下载键、键盘回车、Dock 末键、已连网络那一行 | `#4A80F7` / 白 | `#2E2E2E` / `#F2F2F2` |
| `meter` | 进度条填充、滑块已走的部分 | `#4A80F7` | `#EDEDED` |
| `switch_on` | 开关打开时的轨道 | `#4A80F7` | `#4C4C4C` |

`badge`（确认滑块的轨道、行尾的小标签底）由偏蓝的 `#1D2840` 改成中性 `#262626`；开关关时的
旋钮在深色转成灰色 `#7A7A7A`，滑块旋钮在深色转成黑色（它坐在白色填充里）。

浅色主题下四个新字段的取值和原来的 `accent` 完全一致，所以浅色模式一个像素都没变；深色下再也
找不到蓝色。危险操作（滑动以清除、恢复出厂）仍然是红色——那是语义，不是主题色。

## v21：默认深色（2026-11-15）

`theme.rs` 本来就有两套调色板：`LIGHT` 和 `DARK`（`bg` = `#000000`、`card` = `#1C1C1C`、
`text` = `#EDEDED`、`accent` = `#5488F8`），设置页顶部那两个预览块（`widgets::theme_picker`）
就是切换它们的，选过的值存在 `/metadata/skyward/settings.json`。只是默认落在浅色。

这一版把 `State::default()` 的 `dark` 改成 `true`：

- 新刷入、或 `/metadata` 里还没有 `settings.json` 时，第一次进 recovery 就是黑的；
- **已经存过设置的机器仍按存的值来**——刷完还是白的，去「恢复设置」顶部点深色那格，就地生效并记住，
  不用重启；
- 深浅两套都带配套资源：`DARK_HERO`、`glow::DARK`、键盘的深色键面、终端自己的 `DARK` 配色表、
  文件管理器的深色行色（`files.rs` 的 `t.dark` 分支）、状态栏浅色的电池与信号。

## v20：解密页为什么会有两条进度条（2026-11-15）

v19 认定「解密页没有转圈和进度条」，把 `pages::decrypt` 传给 `data_status` 的 `bar` 从 `None`
改成 `Some(state.data_progress)`。这是误判：进度一直有，只是**不由页面画**，而是 `app::paint_preparing`
每帧直接叠在页面上（布丁 0.2.49 引入，正因为「每帧动、页面不必重建」才放在那一层）。于是两套同时画：

| 来源 | 尺寸 | 位置 | 特征 |
| --- | --- | --- | --- |
| `pages::data_status`（v19 加） | 240 × 8 dp | `sign_cy + 126` | 纯色填充 |
| `app::paint_preparing`（原本就有） | 200 × 4 dp | `cy + 124` | 末端有个进度圆点 |

相差 2 dp，屏幕上就是两条错位横条；转圈同样成两个环（页面 r=48、叠加层 r=40）。布丁自己的
`Data::Closed | Data::Opening` 分支传的就是 `None`（见 `patches/skyward-pudding-v0.2.49.patch`），
这一版改回原样：环 r=40、条 200×4、带端点圆点。

排查过程留一笔：连续两次编译都断在 `os error 5`（写 `target` 下的 `.d` 被拒），落在
`shlex` / `procfs` / `critical-section` 等不同 crate 上；任务管理器里没有 cargo/rustc 残留，
D 盘剩 502 G，用 Python 手工往 `target` 里写文件却正常。把 `CARGO_TARGET_DIR` 指到
`D:\WorkBuddyAI\tmp\target-v20` 后第 1 步一次通过，但第 2 步仍写死 `--target-dir target/system-init`
又撞上。现在 `local_build.py` 的三条 cargo 命令和取产物路径都跟着 `CARGO_TARGET_DIR` 走，
以后遇到目录被锁直接换：

```bash
CARGO_TARGET_DIR=D:/WorkBuddyAI/tmp/target-v20 python scripts/local_build.py --name v20 --deliver
```

## v19：对着布丁 v0.3.9 的官方镜像做终检（2026-11-15）

这次不再比对源码，直接拆用户的镜像：`skyward-pudding-20261115-v0.3.9-OS4.0.6.0.XPCCNXM.img`
（header v4、ramdisk 1103 条、`system/bin/recovery` 9,428,536 字节）。发现：

- 布丁仓库的 v0.3.9 就是当天 12:28 的 `pudding/main`，而 v0.2.55 之后**没有任何源码提交**
  （`git log v0.2.55..v0.3.9` 只有 update.json 与补丁重生成），所以镜像的界面 = 0.2.55 的界面，
  我们这半年对的就是这份源码。
- 中文文案：我们源码里 1858 条含中文的字符串，**全部**能在镜像的二进制里找到；反向扫镜像的
  纯中文段，没有任何一条是我们缺的（只有相邻字面量粘连的假差异）。58 条功能英文短语（锁屏、
  图案解锁、控制中心、亮度/振动、无线传输、AVB、双启动、文件系统修复……）两边计数一致。
- 函数级：`pages.rs` 79 同 / 20 异（19 处只是注释、let-chain 写法、常量位置不同），
  `widgets.rs` 107 同，`files.rs` 16 同。首页、高级工具页、设置页布局**完全一致**。

真正缺的三处，都在这一版补齐：

1. **解密页没有转圈和进度条**：`state.data_progress` 一直在走（`DATA_RATE` 推进，测试也在断言），
   但 `pages::decrypt` 的 `Data::Closed | Data::Opening` 分支给 `data_status` 的 `bar` 传的是
   `None`，于是环不画、条不画。改成 `Some(cx.state.data_progress)`。
2. **文件管理排序行整行看不见**：v18 挪进 `files::strip_with_sort()` 后，路径条画布仍是
   `STRIP_DP`（51 dp），而三个胶囊从 `PILL_TOP_DP + PILL_DP + 8`（54 dp）起画，落在画布外被裁掉。
   布丁的画布是 `STRIP_DP + 44`，照它改。
3. **无线传输提示**：补「WLAN 需先连接。」（繁中「Wi-Fi 需先連線。」、英文 "Wi-Fi has to be joined first."）。

保留的差异（我们更全或属定制）：AVB 页多「备份当前 vbmeta / 写回备份的 vbmeta」两行与
`Confirm::AvbGive`；解密键盘页多「记住密码，下次自动解密」开关与隐藏返回箭头；无线传输选文件夹
时多一个「取消」；品牌字（`卑鄙坑Baby` / `叼着奶瓶逛酒吧` / 署名）。

另记一笔：布丁镜像里的 `skyward/device-popsicle.toml` 是**上游原版**（没有 `[haptics]`、
`[data] weaver_first`、`[backup]`），我们的 `devices/popsicle.toml` 才是这个 fork 的定制版。

## v18：把最后几处界面差异也补齐（2026-11-15）

v17 之后逐函数比对 `pages.rs`（79 个函数完全相同、20 个不同）、`widgets.rs`（106 同 / 2 不同）、
`theme.rs`（完全一致），把还能对上的都对了：

- **首页「WLAN」→「无线网络」**：常量 `WLAN` 一直是 `["WLAN","Wi-Fi","Wi-Fi"]`，首页那一行显示的是
  「WLAN」，而布丁截图是「无线网络」。这是最显眼的一处不一致。
- **文件管理器排序控件进顶部路径条**：原来画在页面 `dp(238)` 位置的三段文字（排序键 / 升降序 /
  文件夹置顶），滚动就看不见了；改成布丁的 `files::strip_with_sort()`，三个胶囊跟着路径条固定，
  随时可点。`listing()` 因此去掉 `sort` 参数。
- **设置页「WiFi」节 → 「解密」节**：首页第一行已有「无线网络」入口，设置页里那节是重复的；
  换成布丁的「记住解锁密码」开关（新增 `Switch::Remember`，与解密页的 `Action::Remember`
  共用 `state.remember` 和 `Effect::Remember`）。
- AVB 滑动确认文案「滑动以修改」→「滑动以写入」；网络下载英文文案 `Download from a link` → `Download`。

保留不动的差异（我们这边更全，不是漏同步）：AVB 页多「备份当前 vbmeta / 写回备份的 vbmeta」两行、
解密页多「记住密码，下次自动解密」开关、`Confirm::AvbGive` 单独确认；品牌字（首页大字、关于页
wordmark 与署名）是 v9–v12 的定制，未改。

顺带修了 `local_build.py` 的 `deliver()`：它只扫 `*.zip` 归档，换成裸 `.img` 后历史镜像一直
堆在交付目录里（14 个并排），现在 `.img` / `.zip` 一起归档，交付目录永远只有当前这一版。

## v17：首页按布丁重排（2026-11-15）

功能本来就不缺——`Screen` 枚举里 44 个页面和布丁一一对应，第二系统、高级工具、备份、恢复、传输、下载、AVB 全在。
缺的是**入口的分组方式**：v16 把文件管理、安装、备份、恢复、挂载、解密六行堆在同一个卡片里，
而布丁新版把它们拆成三组（文件管理提到最上面跟无线网络一起，安装/备份/恢复单独一组，
系统类的一组分在下面），并把「设置」改叫「恢复设置」——在 recovery 里它就是恢复相关的设置，
这个名字比「设置」准确。

同时把「挂载」「解密」从首页挪进高级工具（布丁就是这么放的），首页因此少两行、
高级工具页从 9 项变 10 项。文件管理不再在高级工具里重复出现。

只改 `crates/skyward-ui/src/pages.rs` 一个文件：`home()` 分组、`advanced()` 顺序、四个文案常量。

## v15：同步两个上游仓库（2026-11-15）

上游两个仓库都动了，但**性质完全不同**：

- **kirklin/skyward**：`v0.3.1` 之后 main 上又进了 2 个 fix，**还没打 tag**。只盯 tag 会当成「没有更新」——同步脚本已经改成跟分支头。
- **mariusarva/skyward-pudding**：从 `v0.2.52` 跳到 `v0.3.9`，实质是 **v0.2.55 整体搬到上游 0.3.1 上重做**（作者实测 `patch -p1` 零失败）。它面向 pudding（小米 17），本仓库是 popsicle（17 Pro Max）、结构停在 0.3.0，**整套 diff 不能套**。

取的是具体修复，不是整份补丁：

| 来源 | 改动 | 为什么 |
| --- | --- | --- |
| `c5e09f6` | `skyward_sys::wait_file()`，`hand_over` 里等文件出现（最多 5 秒） | 驱动建 `/dev` 节点晚于 `insmod` 返回，原来直接 `chown` 必失败 |
| `cc68936` | `/vendor/lib64/hw` 进 vendor / odm 的库搜索路径，系统库抽成 `SHARED_LIBRARIES` | Android 上 HAL 实现库放这个目录，找不到就起不来 |
| 布丁 v0.2.55 | `map()` 里映射前先探文件系统，能读就直接用原设备 | 未加密的 data 原来也要走 dm-crypt，直接报错 |
| 布丁 v0.2.55 | `unmap()` 与失败清理前先 `dm::status` | 映射没建成时 `remove` 再报一个错，把真正原因盖掉 |
| 布丁 v0.2.55 | 无 `/data/unencrypted/mode` 时跳过用户解锁 | 明文分区没这个文件，`open_shared` 会失败 |

`999` 分身跳过、`weaver_first` 的备用 blob、`gadget_state`（OTG）本仓库早就有，没重复取。

**没取的**：布丁的 `files.rs` 排序 UI（`strip_with_sort`，把三个排序 pill 画在路径条下面）——那是 0.3.1 布局下的重排，我们的 `ROWS_TOP_DP` 是 0.3.0 的值，直接搬会错位。等哪天整体上 0.3.1 再说。

## v14：为什么没有整个合到 0.3.1

`v0.3.1` 的 14 个提交里，8 个是纯重构（`app.rs` 11694 行拆成 27 个模块、`ui.rs` +1716/−1403、edition 2024、跨 15 个 crate 的 hex/worker 重构），没有任何功能差异，却正好落在布丁改得最多的两个文件上。所以只 `cherry-pick` 了 4 个真修复，四个都零冲突。

代价：上游后续版本如果继续在拆好的 `app/` 目录上开发，越晚整体合入越难。等布丁仓库适配了 0.3.1+ 再考虑整体跟上。做法见 [SYNC-UPSTREAM.md](SYNC-UPSTREAM.md) 第六节。

## 构建方式：Actions 不再是唯一的路（2026-11-14）

`python scripts/local_build.py` 在本机把 CI 那八步全跑完，产物过同一套 `check_build.py`
（ramdisk 1279 条、15 个关键条目、AVB 自洽）。Windows 实测一条命令 16 秒出包（增量编译）。

为此改了 5 处 Unix 专用代码，都按平台分开，`#[cfg(unix)]` 在 `aarch64-linux-android` 上
恒真，**手机端二进制不变**：`skyward-bootimg/ramdisk.rs`（解包落盘）、
`skyward-block/erase.rs`（页大小）、`skyward-programs`（omapi 依赖与安全元件服务名）、
`skyward-config/lib.rs`（设备路径的绝对路径判定）。清单与故障排查见
[LOCAL-BUILD.md](LOCAL-BUILD.md)。

与 CI 产物的差别只有压缩体积：本机 `lz4` v1.10 比 ubuntu-24.04 的 v1.9.4 压得松一点
（zip 约 58.3 MB vs 51.0 MB），差额是分区留白。

## v4 / v5 触控失灵的原因（重要）

两个独立问题叠在一起：

1. **v4**：布丁把 UI 提前到模块还没加载完时就启动。`let mut haptics = ...transpose()?` 在 `synaptics_tcm2` 正在 insmod（约 800 ms）时返回 `no input device takes force-feedback effects`，`?` 让 recovery 直接崩。模块加载器把「文件已存在」（`EEXIST`）当成加载成功，但驱动没 probe，于是主屏触控节点不存在 → 整台机器点不动。v5 把 `Haptics::open` 改成非致命才解决。
2. **v5**：`scan()` 取「第一个 DIRECT + 多点」的设备，而副屏（Goodix 976×596）的 input 节点比主屏（Synaptics 1200×2608）**早 60 ms** 注册，被当成主面板。v6 改成打分选屏后正常。

教训：**震动相关代码绝不能让它阻止输入设备初始化**；**有副屏的机器必须按分辨率选面板**。

## GitHub Actions 为什么曾经全红（2026-11-21）

`Build popsicle` 从 run #28（2026-11-20 13:51，`95b2db8`）起连红 20 次，全部卡在同一步：

```
$ cargo build --locked --release -p skyward-pack
   Compiling skyward-block v0.3.17 (crates/skyward-block)
error[E0308]: mismatched types
   --> crates/skyward-block/src/blkdev.rs:24:47
   24 |     if unsafe { libc::ioctl(file.as_raw_fd(), request as libc::c_int, arg) } != 0 {
      |                 -----------                   ^^^^^^^^^^^^^^^^^^^^^^ expected `u64`, found `i32`
      |                    arguments to this function are incorrect
```

`skyward-pack` 是唯一在 host 上编译的二进制（手机侧 `skyward` / `system-init` 都用
`cargo ndk` 交叉编）。CI 的 host 是 glibc，那里 `libc::ioctl` 的 request 参数是
`Ioctl = c_ulong`；`as libc::c_int` 在 bionic（`Ioctl = c_int`）上正确，在 host 上就是
类型错。`tools` job 在 `Compiling skyward-block` 停住，`Recovery for popsicle` 整块被
skip，所以列表里一片红——**不是二十个不同的毛病，是同一个**。

修法（`dd4ca38`）：请求常量与 helper 直接取 `libc::Ioctl`（bionic / musl 是 `c_int`，
glibc 是 `c_ulong`），调用点不再强转，同一个比特两种宽度都落对。`area.rs`、`dm.rs`、
`erase.rs`、`size_bytes()` 里被 `cfg(target_os = "android")` 挡住、CI 编不到的同类强转
一并对齐，免得下次在 host 上编译再踩一次。

同一列表里的 `cancelled` 不是失败：`concurrency.cancel-in-progress` 在连续 push 时会
把前一次的运行取消掉，被取消的 job 显示为灰而不是红。

`skipped`（灰减号）也不是失败。`Publish the release` 的触发条件是
`if: github.ref_type == 'tag'`（`.github/workflows/build.yml:126`），推 `main` 时这个 job
必然跳过——它只负责在 tag 上建 release 并传 CI 自编的 zip。裸镜像是我本地出包后手动
`POST` 到 `uploads.github.com` 传上去的，跟这个 job 无关。所以「构建成功 + 发布跳过」
是推 `main` 的正常形态；真正发布看 tag 那一次运行（例如 `v98` 对应 run #60）。

本机验证 host 编译的办法（Windows 也能跑）：

```powershell
rustup target add x86_64-unknown-linux-gnu
# unpayload 的 zstd / liblzma / lz4 要交叉 C 编译器，用一个同签名的空壳顶上
cargo check --target x86_64-unknown-linux-gnu -p skyward-pack `
  --config 'patch.crates-io.unpayload.path="D:/WorkBuddyAI/tmp/stub-unpayload"'
```

空壳只照抄 0.1.2 的公开签名（`Payload` / `ImageOptions` / `PartitionImage` / `ReadAt` /
`ExtractOptions` / `Event` / `Extracted`），函数体全 `unimplemented!()`；要验的是 packer
自己的 Rust，不是解包逻辑。`dlltool.exe` 得先进 PATH（`mingw64/bin`），否则 proc-macro
编不过。
