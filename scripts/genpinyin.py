# -*- coding: utf-8 -*-
"""Generate crates/skyward-ui/src/pinyin.rs from Unihan readings + jieba word counts.

    python scripts/genpinyin.py

Reads `pinyin.txt` (mozillazg/pinyin-data) and `jieba_dict.txt` (fxsjy/jieba)
from `downloads/` beside the repository; rerun it after changing either.
"""

import collections
import pathlib

REPO = pathlib.Path(__file__).resolve().parent.parent
DOWN = REPO.parent / "downloads"
OUT = REPO / "crates" / "skyward-ui" / "src" / "pinyin.rs"

TONE = {
    "ā": "a", "á": "a", "ǎ": "a", "à": "a",
    "ē": "e", "é": "e", "ě": "e", "è": "e", "ê": "e", "ế": "e", "ề": "e",
    "ī": "i", "í": "i", "ǐ": "i", "ì": "i",
    "ō": "o", "ó": "o", "ǒ": "o", "ò": "o",
    "ū": "u", "ú": "u", "ǔ": "u", "ù": "u",
    "ǖ": "v", "ǘ": "v", "ǚ": "v", "ǜ": "v", "ü": "v",
    "ń": "n", "ň": "n", "ǹ": "n", "ḿ": "m",
    "\u0300": "", "\u0301": "", "\u030c": "", "\u0304": "", "\u0302": "",
}


def plain(py):
    return "".join(TONE.get(ch, ch) for ch in py).strip().lower()


def gb2312_ok(ch):
    try:
        ch.encode("gb2312")
        return True
    except UnicodeEncodeError:
        return False


def is_han(ch):
    return "\u4e00" <= ch <= "\u9fff"


readings = collections.defaultdict(set)
# 一个字的头一个读音就是它的常读：Unihan 把常用的排在最前。见读 jian 不读
# xian，号读 hao 不读 xiao，靠的就是这个顺序。
first = {}
for line in (DOWN / "pinyin.txt").read_text(encoding="utf-8").splitlines():
    if not line.startswith("U+"):
        continue
    code, rest = line.split(":", 1)
    ch = chr(int(code[2:], 16))
    if not gb2312_ok(ch):
        continue
    pys = [plain(part) for part in rest.split("#")[0].split(",")]
    pys = [py for py in pys if py and all("a" <= c <= "z" for c in py)]
    if not pys:
        continue
    first.setdefault(ch, pys[0])
    for py in pys:
        readings[py].add(ch)

weight = collections.Counter()
for line in (DOWN / "jieba_dict.txt").read_text(encoding="utf-8", errors="replace").splitlines():
    bits = line.split()
    if len(bits) < 2 or not bits[1].isdigit():
        continue
    word, freq = bits[0], int(bits[1])
    if not word:
        continue
    share = freq / len(word)
    for ch in word:
        if is_han(ch):
            weight[ch] += share

PER_SYLLABLE = 12
groups = {}
for py, chars in readings.items():
    # 一个字在这里只是它的次要读音（见在 xian、号在 xiao）时排到后面：
    # 打这个音的人多半要的是常读就是它的那个字。
    groups[py] = sorted(chars, key=lambda c: (first.get(c) != py, -weight[c], c))[:PER_SYLLABLE]

BY_HAND = {
    "beifen": "备份", "huifu": "恢复", "fenzhi": "分区", "jingxiang": "镜像",
    "anzhuang": "安装", "gengxin": "更新", "shezhi": "设置", "mima": "密码",
    "wenjian": "文件", "mulu": "目录", "guazai": "挂载", "jiemi": "解密",
    "chongqi": "重启", "guanji": "关机", "mokuai": "模块", "yingyong": "应用",
    "xitong": "系统", "cunchu": "存储", "zhongduan": "终端", "rizhi": "日志",
    "daochu": "导出", "xiehui": "写回", "jiaoyan": "校验", "liangdu": "亮度",
    "zhendong": "振动", "shengji": "升级", "shuaxin": "刷新", "shuaiji": "刷机",
    "peizhi": "配置", "quanxian": "权限", "kaifa": "开发", "gaoji": "高级",
    "xuanxiang": "选项", "shuju": "数据", "neibu": "内部", "waibu": "外部",
    "fenqubiao": "分区表", "wenjianjia": "文件夹", "zhongwen": "中文",
    "yingwen": "英文", "shuru": "输入", "shuchu": "输出", "wangluo": "网络",
    "wuxian": "无线", "lanya": "蓝牙", "dianchi": "电池", "chongdian": "充电",
    "pingmu": "屏幕", "xiangji": "相机", "zhaopian": "照片", "shipin": "视频",
    "yinle": "音乐", "duanxin": "短信", "dianhua": "电话", "lianxi": "联系",
    "liulan": "浏览", "jiance": "检测", "xiufu": "修复", "shanchu": "删除",
    "fuzhi": "复制", "yidong": "移动", "chongmingming": "重命名",
    "xinjian": "新建", "dakai": "打开", "baocun": "保存", "fenxiang": "分享",
    "sousuo": "搜索", "chazhao": "查找", "tihuan": "替换", "xuanze": "选择",
    "paixu": "排序", "shijian": "时间", "riqi": "日期", "xingqi": "星期",
    "shangwu": "上午", "xiawu": "下午", "wanshang": "晚上", "zaoshang": "早上",
    "zhongwu": "中午", "jintian": "今天", "mingtian": "明天", "houtian": "后天",
    "zuotian": "昨天", "qiantian": "前天", "xiaoshi": "小时",
    "fenzhong": "分钟", "miaozhong": "秒钟", "quanxuan": "全选",
    "jiazai": "加载", "dengdai": "等待", "zanting": "暂停", "jixu": "继续",
    "queding": "确定", "quxiao": "取消", "fanhui": "返回", "kaishi": "开始",
    "jieshu": "结束", "shibai": "失败", "chenggong": "成功", "cuowu": "错误",
    "zhuangtai": "状态", "tongzhi": "通知", "banben": "版本", "guanyu": "关于",
    "wenti": "问题", "jindu": "进度", "sudu": "速度", "rongliang": "容量",
    "kongjian": "空间", "mingcheng": "名称", "leixing": "类型", "daxiao": "大小",
    "xiugai": "修改", "baocunwei": "保存为", "zhuti": "主题", "bizhi": "壁纸",
    "ziti": "字体", "yuyan": "语言", "shengyin": "声音", "jingyin": "静音",
    "feixing": "飞行", "redian": "热点", "zhuangtailan": "状态栏",
    "shoujihao": "手机号", "youxiang": "邮箱", "zhanghao": "账号",
    "shebei": "设备", "mingzi": "名字", "shuoming": "说明", "gongneng": "功能",
    "fenlei": "分类", "moren": "默认", "quanbu": "全部", "zidingyi": "自定义",
    # 天天要说的话：jieba 的频次表把它们排得太低，只好自己写。
    "nihao": "你好", "xiexie": "谢谢", "zaijian": "再见", "duibuqi": "对不起",
    "meiguanxi": "没关系", "qingwen": "请问", "keyi": "可以", "meiyou": "没有",
    "shenme": "什么", "zenme": "怎么", "weishenme": "为什么", "yinwei": "因为",
    "suoyi": "所以", "danshi": "但是", "ruguo": "如果", "xianzai": "现在",
    "yihou": "以后", "yiqian": "以前", "yijing": "已经", "haishi": "还是",
    "huozhe": "或者", "zhege": "这个", "nage": "那个", "zhexie": "这些",
    "naxie": "那些", "women": "我们", "nimen": "你们", "tamen": "他们",
    "ziji": "自己", "dajia": "大家", "pengyou": "朋友", "laoshi": "老师",
    "xuesheng": "学生", "gongzuo": "工作", "xuexi": "学习", "shenghuo": "生活",
    "shihou": "时候", "difang": "地方", "dongxi": "东西", "shiqing": "事情",
    "banfa": "办法", "fangfa": "方法", "yuanyin": "原因", "jieguo": "结果",
    "ranhou": "然后", "zuihou": "最后", "diyi": "第一",
}

AUTO = {}
for line in (DOWN / "jieba_dict.txt").read_text(encoding="utf-8", errors="replace").splitlines():
    bits = line.split()
    if len(bits) < 2 or not bits[1].isdigit():
        continue
    word, freq = bits[0], int(bits[1])
    if len(word) != 2 or freq < 3000:
        continue
    if not all(gb2312_ok(c) and is_han(c) for c in word):
        continue
    reading = "".join(first.get(c, "") for c in word)
    if not reading or len(reading) > 18 or reading in BY_HAND:
        continue
    best = AUTO.get(reading)
    if best is None or freq > best[1]:
        AUTO[reading] = (word, freq)

words = dict(BY_HAND)
for reading, (word, _freq) in AUTO.items():
    words.setdefault(reading, word)

print("syllables", len(groups), "chars", sum(len(v) for v in groups.values()))
print("words", len(words), "by hand", len(BY_HAND), "auto", len(words) - len(BY_HAND))

head = '''//! 内置拼音：一个音节到常用字的表，和一小张词的表。
//!
//! 字表来自 Unihan 的读音，只留 GB2312 里的字（屏幕上的 MiSans 都画得出）；
//! 同一音节里按字频排序，字频是在 jieba 的词频表上按字数摊出来的。词表一半
//! 是本仓库自己页面上的用语（手工写死，免得多音字选错音），一半是 jieba 里
//! 频次够高的两三个字。
//!
//! 生成脚本是 `scripts/genpinyin.py`，改词典就重跑它，不要手改这个文件。

/// 一条候选：写进输入框的字，和它从拼音串里去掉的字节数。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub text: String,
    /// 这一条用掉了拼音串开头的几个字节。
    pub used: usize,
}

/// 一个音节能出的候选字，常用的排在最前。
pub const DICT: &[(&str, &str)] = &[
'''

body = [f'    ("{py}", "{"".join(groups[py])}"),' for py in sorted(groups)]
body.append("];")
body.append("")
body.append("/// 整串正好是这些词时，词比自己拼出来的字先出。")
body.append("pub const WORDS: &[(&str, &str)] = &[")
body += [f'    ("{py}", "{words[py]}"),' for py in sorted(words)]
body.append("];")

tail = '''
/// 拼音串开头的那个音节：整串正好是一个音节就是它，否则取能配上的最长音节。
/// `None` 说明这串只是若干音节的开头（比如只打了一个 `n`），再打一个字母才
/// 分得清，这时不给候选。
fn syllable(input: &str) -> Option<(&'static str, usize)> {
    if let Some(&(py, _)) = DICT.iter().find(|(py, _)| *py == input) {
        return Some((py, input.len()));
    }
    if DICT.iter().any(|(py, _)| py.starts_with(input)) {
        return None;
    }
    let mut best: Option<&'static str> = None;
    for &(py, _) in DICT {
        if input.starts_with(py) && best.map_or(true, |b| py.len() > b.len()) {
            best = Some(py);
        }
    }
    best.map(|py| (py, py.len()))
}

/// 一串小写字母能写出的候选：整串正好是一个词的，词排最前；然后是开头那个
/// 音节的字，常用的在前。空表示还看不出要打什么，再打一个字母。
pub fn options(input: &str) -> Vec<Pick> {
    if input.is_empty() || !input.bytes().all(|b| b.is_ascii_lowercase()) {
        return Vec::new();
    }
    let mut out: Vec<Pick> = Vec::new();
    if let Some(&(_, word)) = WORDS.iter().find(|(py, _)| *py == input) {
        out.push(Pick { text: word.to_string(), used: input.len() });
    }
    let Some((py, used)) = syllable(input) else { return out };
    if let Some(&(_, chars)) = DICT.iter().find(|(candidate, _)| *candidate == py) {
        for ch in chars.chars() {
            let text = ch.to_string();
            if out.iter().all(|p| p.text != text) {
                out.push(Pick { text, used });
            }
        }
    }
    out
}

/// 拼音条一页放几个候选。
pub const PER_PAGE: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(input: &str) -> Vec<String> {
        options(input).into_iter().map(|p| p.text).collect()
    }

    #[test]
    fn a_finished_syllable_offers_its_characters_with_the_commonest_first() {
        let ni = texts("ni");
        assert_eq!(ni.first().map(String::as_str), Some("你"));
        assert!(ni.len() > 3, "a syllable gives a few: {ni:?}");
    }

    #[test]
    fn a_word_comes_before_the_characters_it_is_made_of() {
        let picks = options("nihao");
        assert_eq!(picks[0].text, "你好");
        assert_eq!(picks[0].used, "nihao".len());
    }

    #[test]
    fn a_run_that_is_only_the_start_of_syllables_waits() {
        assert!(options("zh").is_empty(), "zh begins zhi, zhang and more");
        assert!(options("s").is_empty(), "s begins shi, sa, si and more");
    }

    #[test]
    fn what_is_left_after_the_syllable_is_offered_next() {
        let picks = options("nihaoma");
        assert_eq!(picks[0].text, "你");
        assert_eq!(picks[0].used, 2);
    }

    #[test]
    fn a_syllable_fills_more_than_one_page() {
        let picks = options("shi");
        assert!(picks.len() > PER_PAGE);
        assert_ne!(picks[PER_PAGE].text, picks[0].text);
    }
}
'''

OUT.write_text(head + "\n".join(body) + "\n" + tail, encoding="utf-8", newline="\n")
print("wrote", OUT, OUT.stat().st_size, "bytes")
print("sample ni:", "".join(groups.get("ni", [])))
print("sample hao:", "".join(groups.get("hao", [])))
print("sample shi:", "".join(groups.get("shi", [])))
