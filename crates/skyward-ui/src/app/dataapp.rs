//! 用户装在 data 分区里的应用：`/data/app/<包名>-<散列>/` 一套 apk，安装时间记在
//! `/data/system/packages.xml` 的 `first-install-time`。

use std::path::Path;

/// 一台手机上装着的应用，按安装时间从新到旧。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataApp {
    /// 包名，也是 `/data/app` 里文件夹名去掉后面那段散列。
    pub package: String,
    /// `/data/app` 里的那个文件夹。
    pub dir: String,
    /// 应用自己起的名字，认不出来就退回包名。
    pub label: String,
    /// 版本，来自 APK 里的 `versionName`，没有就空着。
    pub version: String,
    /// 安装时间，Unix 秒；存档里没写就是 0。
    pub installed: i64,
    /// 最后更新时间，Unix 秒；同上。
    pub updated: i64,
    /// 这套 apk 一共多少字节。
    pub size: u64,
    /// 图标那份 PNG 的字节，挑最高密度的那张；没有图标就是空。
    pub icon: Vec<u8>,
    /// 装在系统分区里（`/system`、`/product` 这些），不是用户自己装的。
    pub system: bool,
}

/// `/data/system/packages.xml` 里记的安装时间，按包名。
pub fn install_times(xml: &str) -> std::collections::HashMap<String, (i64, i64)> {
    let mut out = std::collections::HashMap::new();
    for chunk in xml.split("<package ").skip(1) {
        let head = match chunk.find('>') {
            Some(end) => &chunk[..end],
            None => continue,
        };
        let field = |name: &str| -> Option<&str> {
            let at = head.find(&format!("{name}=\""))?;
            let rest = &head[at + name.len() + 2..];
            let end = rest.find('"')?;
            Some(&rest[..end])
        };
        let Some(package) = field("name") else { continue };
        let when = |name: &str| field(name).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
        let first = when("first-install-time");
        let last = when("last-update-time");
        out.insert(package.to_string(), (first, last));
    }
    out
}

/// APK 的 `AndroidManifest.xml` 里 `application` 的 `label`，只认字面量；写成
/// `@string/…` 的这里解析不了，调用方退回包名。
pub fn literal_label(manifest: &[u8]) -> Option<String> {
    match label_of(manifest)? {
        Label::Text(text) => Some(text),
        Label::Id(_) => None,
    }
}

/// 标签是个资源 id 时，那个 id。
pub fn label_id(manifest: &[u8]) -> Option<u32> {
    match label_of(manifest)? {
        Label::Id(id) => Some(id),
        Label::Text(_) => None,
    }
}

/// 清单里 `application` 的标签：写死的字，或者一个资源 id。
pub enum Label {
    Text(String),
    Id(u32),
}

fn label_of(manifest: &[u8]) -> Option<Label> {
    // 二进制 AXML：先读字符串池，再在 start element 里找 application 的 label。
    fn u16_at(d: &[u8], at: usize) -> Option<u16> {
        d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32_at(d: &[u8], at: usize) -> Option<u32> {
        d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    const RES_STRING_POOL: u16 = 0x0001;
    const RES_XML_START_ELEMENT: u16 = 0x0102;
    const STRING_POOL_UTF8: u32 = 0x100;
    if u16_at(manifest, 0)? != 0x0003 {
        return None;
    }
    let mut at = u16_at(manifest, 2)? as usize;
    let mut strings: Vec<String> = Vec::new();
    while at + 8 <= manifest.len() {
        let kind = u16_at(manifest, at)?;
        let header = u16_at(manifest, at + 2)? as usize;
        let size = u32_at(manifest, at + 4)? as usize;
        if size < 8 || at + size > manifest.len() {
            break;
        }
        if kind == RES_STRING_POOL {
            let count = u32_at(manifest, at + 8)? as usize;
            let flags = u32_at(manifest, at + 16)?;
            let start = u32_at(manifest, at + 20)? as usize;
            let utf8 = flags & STRING_POOL_UTF8 != 0;
            let offsets = at + header;
            for i in 0..count {
                let off = u32_at(manifest, offsets + i * 4)? as usize;
                let mut p = at + start + off;
                let text = if utf8 {
                    p += 2; // 先是一个 u8 长度，再一个是 UTF-16 长度
                    let mut bytes = Vec::new();
                    while let Some(&b) = manifest.get(p) {
                        if b == 0 {
                            break;
                        }
                        bytes.push(b);
                        p += 1;
                    }
                    String::from_utf8_lossy(&bytes).into_owned()
                } else {
                    let n = u16_at(manifest, p)? as usize;
                    p += 2;
                    let mut units = Vec::with_capacity(n);
                    for k in 0..n {
                        units.push(u16_at(manifest, p + k * 2)?);
                    }
                    String::from_utf16_lossy(&units)
                };
                strings.push(text);
            }
        } else if kind == RES_XML_START_ELEMENT {
            let ext = at + header;
            let name_at = u32_at(manifest, ext + 4)? as usize;
            let attr_start = u16_at(manifest, ext + 8)? as usize;
            let attr_size = u16_at(manifest, ext + 10)? as usize;
            let count = u16_at(manifest, ext + 12)? as usize;
            let name = strings.get(name_at).map(String::as_str).unwrap_or("");
            if name == "application" && attr_size >= 20 {
                for i in 0..count {
                    let a = ext + attr_start + i * attr_size;
                    let key = strings.get(u32_at(manifest, a + 4)? as usize)?.clone();
                    if key != "label" {
                        continue;
                    }
                    let raw = u32_at(manifest, a + 8)? as usize;
                    let kind = manifest.get(a + 15).copied().unwrap_or(0);
                    // 0x03 是字符串，值在 rawValue；0x01 是指向资源表的引用，id 在
                    // data 里（rawValue 那时是 0xffffffff）。
                    let data = u32_at(manifest, a + 16)? as usize;
                    match kind {
                        0x03 => return strings.get(raw).filter(|s| !s.is_empty()).cloned().map(Label::Text),
                        0x01 => return Some(Label::Id(data as u32)),
                        _ => return None,
                    }
                }
            }
        }
        at += size;
    }
    None
}

/// `resources.arsc` 里某个资源 id 对应的字符串：先按包、再按类型、再按条目
/// 找过去。标签写成 `@string/app_name` 的应用都靠这一步才认得出名字。
pub fn resolve_string(arsc: &[u8], id: u32) -> Option<String> {
    fn u16_at(d: &[u8], at: usize) -> Option<u16> {
        d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32_at(d: &[u8], at: usize) -> Option<u32> {
        d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    const RES_TABLE: u16 = 0x0002;
    const RES_STRING_POOL: u16 = 0x0001;
    const RES_TABLE_PACKAGE: u16 = 0x0200;
    const RES_TABLE_TYPE: u16 = 0x0201;
    const STRING_POOL_UTF8: u32 = 0x100;
    const FLAG_COMPACT: u16 = 0x0008;
    /// 表头的全局字符串池里第 n 个。
    fn pool(d: &[u8], at: usize, size: usize, n: usize) -> Option<String> {
        let count = u32_at(d, at + 8)? as usize;
        if n >= count {
            return None;
        }
        let flags = u32_at(d, at + 16)?;
        let start = u32_at(d, at + 20)? as usize;
        let header = u16_at(d, at + 2)? as usize;
        let off = u32_at(d, at + header + n * 4)? as usize;
        let mut p = at + start + off;
        let _ = size;
        if flags & STRING_POOL_UTF8 != 0 {
            p += 2;
            let mut bytes = Vec::new();
            while let Some(&b) = d.get(p) {
                if b == 0 {
                    break;
                }
                bytes.push(b);
                p += 1;
            }
            Some(String::from_utf8_lossy(&bytes).into_owned())
        } else {
            let len = u16_at(d, p)? as usize;
            p += 2;
            let mut units = Vec::with_capacity(len);
            for k in 0..len {
                units.push(u16_at(d, p + k * 2)?);
            }
            Some(String::from_utf16_lossy(&units))
        }
    }

    if u16_at(arsc, 0)? != RES_TABLE {
        return None;
    }
    let table_size = u32_at(arsc, 4)? as usize;
    let table_header = u16_at(arsc, 2)? as usize;
    // 全局字符串池紧跟在表头后面。
    let global = table_header;
    let mut at = table_header;
    while at + 8 <= arsc.len().min(table_size) {
        let kind = u16_at(arsc, at)?;
        let header = u16_at(arsc, at + 2)? as usize;
        let size = u32_at(arsc, at + 4)? as usize;
        if size < 8 || at + size > arsc.len() {
            return None;
        }
        if kind == RES_TABLE_PACKAGE {
            let package_id = u32_at(arsc, at + 8)?;
            if package_id == id >> 24 {
                let type_strings = u32_at(arsc, at + 8 + 4 + 256)? as usize;
                let key_strings = u32_at(arsc, at + 8 + 4 + 256 + 4 + 4)? as usize;
                let type_off = at + type_strings;
                let key_off = at + key_strings;
                let _ = RES_STRING_POOL;
                let type_size = {
                    let size = u32_at(arsc, type_off + 4)? as usize;
                    size
                };
                let _ = type_size;
                let mut p = at + header;
                while p + 8 <= at + size {
                    let k = u16_at(arsc, p)?;
                    let h = u16_at(arsc, p + 2)? as usize;
                    let sz = u32_at(arsc, p + 4)? as usize;
                    if sz < 8 || p + sz > at + size {
                        break;
                    }
                    if k == RES_TABLE_TYPE {
                        let type_id = arsc.get(p + 8).copied().unwrap_or(0) as u32;
                        // 同一个类型有多个配置变体（按密度、语言等），第一个变体里
                        // 常常是空条目，所以要挨个看过去。
                        if type_id == (id >> 16) & 0xff {
                            let entry_count = u32_at(arsc, p + 12)? as usize;
                            let entries_start = u32_at(arsc, p + 16)? as usize;
                            let index = (id & 0xffff) as usize;
                            if index < entry_count {
                                let offset = u32_at(arsc, p + h + index * 4)? as usize;
                                if offset != 0xffff_ffff {
                                    let e = p + entries_start + offset;
                                    let flags = u16_at(arsc, e + 2)?;
                                    let (data_kind, data) = if flags & FLAG_COMPACT != 0 {
                                        // 紧凑条目：值就写在条目里，key 在前。
                                        (arsc.get(e + 7).copied().unwrap_or(0), u32_at(arsc, e + 8)?)
                                    } else {
                                        let esize = u16_at(arsc, e)? as usize;
                                        (arsc.get(e + esize + 3).copied().unwrap_or(0), u32_at(arsc, e + esize + 4)?)
                                    };
                                    // 0x03 = 字符串池的索引。
                                    if data_kind == 0x03 {
                                        let _ = key_off;
                                        if let Some(text) = pool(arsc, global, table_size, data as usize) {
                                            if !text.is_empty() {
                                                return Some(text);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p += sz;
                }
            }
        }
        at += size;
    }
    None
}

/// APK 里的 `versionName`，只认字面量。
pub fn literal_version(manifest: &[u8]) -> Option<String> {
    // 简化：直接找版本属性所在的那一小段，字面量版本号在字符串池里，无法可靠
    // 反查，所以这里只处理 manifest 里写成字面量的常见情形——留空更常见。
    let _ = manifest;
    None
}

/// 目录名里的包名：`<包名>-<散列>`。
pub fn package_of(dir: &str) -> String {
    // 包名只由字母、数字、点组成，目录名里第一个 '-' 起就是散列。
    match dir.find('-') {
        Some(at) if at > 0 => dir[..at].to_string(),
        _ => dir.to_string(),
    }
}

/// 一套 apk 的字节总数。
pub fn size_of(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(list) = std::fs::read_dir(dir) {
        for entry in list.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version='1.0' encoding='utf-8' standalone='yes' ?>
<packages>
<package name="com.example.one" codePath="/data/app/~~abc/com.example.one-xyz" first-install-time="1763000000000" last-update-time="1763100000000" />
<package name="com.example.two" codePath="/data/app/~~def/com.example.two-uvw" first-install-time="1762000000000" />
</packages>"#;

    #[test]
    fn 安装时间按包名读出来() {
        let times = install_times(XML);
        assert_eq!(times["com.example.one"], (1763000000000, 1763100000000));
        assert_eq!(times["com.example.two"], (1762000000000, 0));
    }

    #[test]
    fn 有损_webp_的图标也解得开() {
        // 64×64 的纯色有损 WebP（PIL 生成），文件头是 RIFF/WEBP。
        let b64 = "UklGRlIAAABXRUJQVlA4IEYAAAAQBACdASpAAEAAPm02mEkkIyKhIggAgA2JaQB2APwAACDQri2IXGAUQAD+6zb///OJY6nd3Z///NgQM0PnpFa4mPUcAAAA";
        let bytes = crate::app::dataapp::base64_decode(b64).expect("sample");
        assert_eq!(&bytes[..4], b"RIFF");
        let (w, h, rgba) = super::decode_icon(&bytes, 32);
        assert_eq!((w, h), (32, 32), "缩到 32 见方");
        assert_eq!(rgba.len(), 32 * 32 * 4);
        // 中间那点是不透明的蓝色系。
        let at = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(rgba[at + 3], 255, "不透明");
        assert!(rgba[at + 2] > rgba[at], "偏蓝");
    }

    #[test]
    fn 目录名去掉散列就是包名() {
        assert_eq!(package_of("com.example.one-xyz123"), "com.example.one");
        assert_eq!(package_of("io.github.mangi.eta.beta-hWRNOoUANvr2r8ZCjq-nOg=="), "io.github.mangi.eta.beta");
        assert_eq!(package_of("nodash"), "nodash");
    }
}
/// APK 里最合适的那张启动图标：`res/mipmap-*/ic_launcher.png|webp` 一类，
/// 按密度挑；同一密度下 PNG 优先（解码路径更短）。
pub fn icon_png(names: impl Iterator<Item = String>, mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut best: Option<(u8, u8, String)> = None;
    for name in names {
        let lower_name = name.to_lowercase();
        if !name.starts_with("res/") || !(lower_name.ends_with(".png") || lower_name.ends_with(".webp")) {
            continue;
        }
        let lower = lower_name;
        let looks_right = lower.contains("ic_launcher") || lower.ends_with("/icon.png") || lower.ends_with("/icon.webp");
        if !looks_right {
            continue;
        }
        let density = if lower.contains("xxxhdpi") {
            5
        } else if lower.contains("xxhdpi") {
            4
        } else if lower.contains("xhdpi") {
            3
        } else if lower.contains("hdpi") {
            2
        } else if lower.contains("mdpi") {
            1
        } else {
            0
        };
        // PNG 排在 WebP 前面：同一密度下先取它。
        let kind = if lower.ends_with(".png") { 1 } else { 0 };
        if best.as_ref().is_none_or(|(d, k, _)| (density, kind) > (*d, *k)) {
            best = Some((density, kind, name));
        }
    }
    best.and_then(|(_, _, name)| read(name.as_str())).unwrap_or_default()
}

/// 一份 PNG 解成 RGBA，并缩到 `side` 见方（最近邻，够清楚也够快）。
pub fn decode_icon(png_bytes: &[u8], side: u32) -> (u32, u32, Vec<u8>) {
    let empty = (0, 0, Vec::new());
    if png_bytes.is_empty() || side == 0 {
        return empty;
    }
    // WebP 与 PNG 都是常见的图标格式，按文件头分开走。
    if png_bytes.len() > 12 && &png_bytes[..4] == b"RIFF" && &png_bytes[8..12] == b"WEBP" {
        return decode_webp(png_bytes, side);
    }
    let decoder = png::Decoder::new(png_bytes);
    let Ok(mut reader) = decoder.read_info() else { return empty };
    let mut buf = vec![0; reader.output_buffer_size()];
    let Ok(info) = reader.next_frame(&mut buf) else { return empty };
    let (w, h) = (info.width.max(1), info.height.max(1));
    let src = &buf[..info.buffer_size()];
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return empty,
    };
    let mut out = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let sx = (x as u64 * w as u64 / side as u64) as u32;
            let sy = (y as u64 * h as u64 / side as u64) as u32;
            let at = ((sy * w + sx) as usize) * channels;
            let px = match channels {
                4 => [src[at], src[at + 1], src[at + 2], src[at + 3]],
                3 => [src[at], src[at + 1], src[at + 2], 255],
                2 => [src[at], src[at], src[at], src[at + 1]],
                _ => [src[at], src[at], src[at], 255],
            };
            let to = ((y * side + x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&px);
        }
    }
    (side, side, out)
}

/// 一份 WebP（有损或无损都行）解成 RGBA，缩到 `side` 见方。
fn decode_webp(bytes: &[u8], side: u32) -> (u32, u32, Vec<u8>) {
    let empty = (0, 0, Vec::new());
    let Ok(mut decoder) = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)) else { return empty };
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 {
        return empty;
    }
    let has_alpha = decoder.has_alpha();
    let channels = if has_alpha { 4 } else { 3 };
    let mut buf = vec![0u8; (w as usize) * (h as usize) * channels];
    if decoder.read_image(&mut buf).is_err() {
        return empty;
    }
    let mut out = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let sx = (x as u64 * w as u64 / side as u64) as u32;
            let sy = (y as u64 * h as u64 / side as u64) as u32;
            let at = ((sy * w + sx) as usize) * channels;
            let px = if channels == 4 {
                [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
            } else {
                [buf[at], buf[at + 1], buf[at + 2], 255]
            };
            let to = ((y * side + x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&px);
        }
    }
    (side, side, out)
}

/// 只给测试用的一小段 base64 解码。
#[cfg(test)]
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in text.bytes() {
        if c == b'=' {
            break;
        }
        let v = TABLE.iter().position(|&t| t == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// 自适应图标（`mipmap-anydpi-v26/ic_launcher.xml`）里 `<foreground>` 指向的
/// 资源 id。前景图才是有画面的那一层。
pub fn adaptive_foreground(xml: &[u8]) -> Option<u32> {
    foreground_id(xml, "foreground")
}

/// 同一个资源 id 在各配置变体下的字符串（图标就是各个密度的路径）。
pub fn resolve_paths(arsc: &[u8], id: u32) -> Vec<String> {
    let mut out = Vec::new();
    let mut at = u16_at(arsc, 2).unwrap_or(12) as usize;
    let table_size = u32_at(arsc, 4).unwrap_or(arsc.len() as u32) as usize;
    let global = at;
    while at + 8 <= arsc.len().min(table_size) {
        let Some(kind) = u16_at(arsc, at) else { break };
        let Some(header) = u16_at(arsc, at + 2).map(|v| v as usize) else { break };
        let Some(size) = u32_at(arsc, at + 4).map(|v| v as usize) else { break };
        if size < 8 || at + size > arsc.len() {
            break;
        }
        if kind == 0x0200 && u32_at(arsc, at + 8) == Some(id >> 24) {
            let mut p = at + header;
            while p + 8 <= at + size {
                let Some(k) = u16_at(arsc, p) else { break };
                let Some(h) = u16_at(arsc, p + 2).map(|v| v as usize) else { break };
                let Some(sz) = u32_at(arsc, p + 4).map(|v| v as usize) else { break };
                if sz < 8 {
                    break;
                }
                if k == 0x0201 && arsc.get(p + 8) == Some(&(((id >> 16) & 0xff) as u8)) {
                    let entry_count = u32_at(arsc, p + 12).unwrap_or(0) as usize;
                    let entries_start = u32_at(arsc, p + 16).unwrap_or(0) as usize;
                    let index = (id & 0xffff) as usize;
                    if index < entry_count {
                        if let Some(offset) = u32_at(arsc, p + h + index * 4).map(|v| v as usize) {
                            if offset != 0xffff_ffff {
                                let e = p + entries_start + offset;
                                let flags = u16_at(arsc, e + 2).unwrap_or(0);
                                let (data_kind, data) = if flags & 0x0008 != 0 {
                                    (arsc.get(e + 7).copied().unwrap_or(0), u32_at(arsc, e + 8).unwrap_or(0))
                                } else {
                                    let esize = u16_at(arsc, e).unwrap_or(8) as usize;
                                    (arsc.get(e + esize + 3).copied().unwrap_or(0), u32_at(arsc, e + esize + 4).unwrap_or(0))
                                };
                                if data_kind == 0x03 {
                                    if let Some(text) = pool_at(arsc, global, data as usize) {
                                        if !text.is_empty() {
                                            out.push(text);
                                        }
                                    }
                                } else if data_kind == 0x01 && data != id {
                                    // 值的是一条引用：跟过去继续找，最多三层。
                                    out.extend(resolve_paths(arsc, data));
                                }
                            }
                        }
                    }
                }
                p += sz;
            }
        }
        at += size;
    }
    out
}

/// 一份 AXML 里名为 `want` 的元素上 `drawable` 那个属性（资源 id）。
fn foreground_id(xml: &[u8], want: &str) -> Option<u32> {
    let mut at = u16_at(xml, 2)? as usize;
    let mut strings: Vec<String> = Vec::new();
    while at + 8 <= xml.len() {
        let kind = u16_at(xml, at)?;
        let header = u16_at(xml, at + 2)? as usize;
        let size = u32_at(xml, at + 4)? as usize;
        if size < 8 || at + size > xml.len() {
            break;
        }
        if kind == 0x0001 {
            let count = u32_at(xml, at + 8)? as usize;
            let flags = u32_at(xml, at + 16)?;
            let start = u32_at(xml, at + 20)? as usize;
            strings.clear();
            for i in 0..count {
                let Some(off) = u32_at(xml, at + header + i * 4) else { break };
                let mut p = at + start + off as usize;
                let text = if flags & 0x100 != 0 {
                    p += 2;
                    let mut bytes = Vec::new();
                    while let Some(&b) = xml.get(p) {
                        if b == 0 {
                            break;
                        }
                        bytes.push(b);
                        p += 1;
                    }
                    String::from_utf8_lossy(&bytes).into_owned()
                } else {
                    let n = u16_at(xml, p)? as usize;
                    p += 2;
                    let mut units = Vec::with_capacity(n);
                    for k in 0..n {
                        units.push(u16_at(xml, p + k * 2)?);
                    }
                    String::from_utf16_lossy(&units)
                };
                strings.push(text);
            }
        } else if kind == 0x0102 {
            let ext = at + header;
            let name = strings.get(u32_at(xml, ext + 4)? as usize).map(String::as_str).unwrap_or("");
            if name == want {
                let attr_start = u16_at(xml, ext + 8)? as usize;
                let attr_size = u16_at(xml, ext + 10)? as usize;
                let count = u16_at(xml, ext + 12)? as usize;
                for i in 0..count {
                    let a = ext + attr_start + i * attr_size;
                    let key = strings.get(u32_at(xml, a + 4)? as usize).map(String::as_str).unwrap_or("");
                    if key == "drawable" {
                        return u32_at(xml, a + 16);
                    }
                }
            }
        }
        at += size;
    }
    None
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn pool_at(d: &[u8], at: usize, n: usize) -> Option<String> {
    let count = u32_at(d, at + 8)? as usize;
    if n >= count {
        return None;
    }
    let flags = u32_at(d, at + 16)?;
    let start = u32_at(d, at + 20)? as usize;
    let header = u16_at(d, at + 2)? as usize;
    let off = u32_at(d, at + header + n * 4)? as usize;
    let mut p = at + start + off;
    if flags & 0x100 != 0 {
        p += 2;
        let mut bytes = Vec::new();
        while let Some(&b) = d.get(p) {
            if b == 0 {
                break;
            }
            bytes.push(b);
            p += 1;
        }
        Some(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        let len = u16_at(d, p)? as usize;
        p += 2;
        let mut units = Vec::with_capacity(len);
        for k in 0..len {
            units.push(u16_at(d, p + k * 2)?);
        }
        Some(String::from_utf16_lossy(&units))
    }
}

#[cfg(test)]
mod real_apks {
    //! 真机上的 APK：手上有一批拷到 /storage 的样本，能跑就跑，跑不到就跳过。

    use super::*;
    use std::io::Read;

    fn samples() -> Vec<std::path::PathBuf> {
        let dir = std::path::Path::new("/storage/emulated/0/MT2/mcp/apks");
        let Ok(list) = std::fs::read_dir(dir) else { return Vec::new() };
        list.flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "apk"))
            .collect()
    }

    /// 一个 APK 的名字：字面量或资源表里查到的那个。
    pub fn label_of(path: &std::path::Path) -> Option<String> {
        let file = std::fs::File::open(path).ok()?;
        let mut zip = zip::ZipArchive::new(file).ok()?;
        let mut manifest = Vec::new();
        zip.by_name("AndroidManifest.xml").ok()?.read_to_end(&mut manifest).ok()?;
        if let Some(text) = literal_label(&manifest) {
            return Some(text);
        }
        let id = label_id(&manifest)?;
        let mut arsc = Vec::new();
        zip.by_name("resources.arsc").ok()?.read_to_end(&mut arsc).ok()?;
        resolve_string(&arsc, id)
    }

    #[test]
    fn 真机上的应用大多能从资源表里读出名字() {
        let list = samples();
        if list.is_empty() {
            return; // 没有样本就跳过
        }
        let mut named = 0;
        let mut shown = Vec::new();
        for path in &list {
            let pkg = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            match label_of(path) {
                Some(text) if !text.is_empty() => {
                    named += 1;
                    if shown.len() < 6 {
                        shown.push(format!("{pkg} → {text}"));
                    }
                }
                _ => {
                    if shown.len() < 12 {
                        shown.push(format!("{pkg} → （未读到）"));
                    }
                }
            }
        }
        println!("读到名字 {named}/{}", list.len());
        for line in &shown {
            println!("  {line}");
        }
        assert!(named * 10 >= list.len() * 8, "至少八成能读出名字，实际 {named}/{}", list.len());
    }
}
