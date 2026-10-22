// ── 数据结构 ──────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct WordBox {
    pub text: String,
    /// 归一化坐标（0.0 ~ 1.0，原点左上）
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// BBox 计算内部用的归一化矩形（与 WordBox 字段布局一致）
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalizedRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

// ── 字符分类 ──────────────────────────────────────────────

#[inline]
pub(crate) fn is_token_char(c: char) -> bool {
    c.is_ascii_alphabetic()
}

#[inline]
pub(crate) fn is_uppercase(c: char) -> bool {
    c.is_ascii_uppercase()
}

#[inline]
pub(crate) fn is_lowercase(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// CamelCase 拆分：HelloWorld → [Hello, World]
pub(crate) fn refined_latin_ranges(s: &str) -> Vec<std::ops::Range<usize>> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut prev: Option<char> = None;

    for i in 0..chars.len() {
        let (byte_idx, cur) = chars[i];
        let next = chars.get(i + 1).map(|&(_, c)| c);

        if is_token_char(cur) {
            if start.is_none() {
                start = Some(byte_idx);
            } else if let Some(p) = prev {
                let split = (is_lowercase(p) && is_uppercase(cur))
                    || (is_uppercase(p)
                        && is_uppercase(cur)
                        && next.map(is_lowercase).unwrap_or(false));
                if split {
                    ranges.push(start.unwrap()..byte_idx);
                    start = Some(byte_idx);
                }
            }
            prev = Some(cur);
        } else {
            if let Some(s) = start {
                ranges.push(s..byte_idx);
            }
            start = None;
            prev = None;
        }
    }
    if let Some(s) = start {
        ranges.push(s..s_byte_end(s, &chars));
    }
    ranges
}

fn s_byte_end(start: usize, chars: &[(usize, char)]) -> usize {
    chars
        .iter()
        .rev()
        .find(|(bi, _)| *bi >= start)
        .map(|(bi, c)| bi + c.len_utf8())
        .unwrap_or(start)
}

// ── 脚本类型 ──────────────────────────────────────────────

#[derive(PartialEq)]
pub(crate) enum TokenKind {
    Latin,
    Han,
    OtherLetter,
}

pub(crate) fn token_kind(c: char) -> Option<TokenKind> {
    if c.is_ascii_alphabetic() {
        return Some(TokenKind::Latin);
    }
    let cp = c as u32;
    if (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0x3040..=0x30FF).contains(&cp)
    {
        return Some(TokenKind::Han);
    }
    if c.is_alphabetic() {
        return Some(TokenKind::OtherLetter);
    }
    None
}

/// 脚本感知分词：拉丁段做 CamelCase 拆分，汉字段整体成一个 token。
/// 这样 "调用APIRequest时" 会产生 ["调用", "API", "Request", "时"] 四个 token，
/// 中英混排行里的英文单词能正确提取，汉字段也能整段拿到（不再逐字切开，
/// 否则中文取词只能拿到一个字）。
pub(crate) fn script_aware_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut ranges = Vec::new();
    let mut seg_start: Option<usize> = None;
    let mut cur_kind: Option<TokenKind> = None;

    let flush = |seg_start: usize, end: usize, kind: &TokenKind, ranges: &mut Vec<_>| {
        if seg_start >= end {
            return;
        }
        match kind {
            TokenKind::Latin => ranges.extend(
                refined_latin_ranges(&text[seg_start..end])
                    .into_iter()
                    .map(|r| (seg_start + r.start)..(seg_start + r.end)),
            ),
            _ => ranges.push(seg_start..end),
        }
    };

    for &(byte_idx, c) in &chars {
        match token_kind(c) {
            Some(k) => match &cur_kind {
                None => {
                    seg_start = Some(byte_idx);
                    cur_kind = Some(k);
                }
                Some(ck) if *ck != k => {
                    let ss = seg_start.unwrap();
                    flush(ss, byte_idx, ck, &mut ranges);
                    seg_start = Some(byte_idx);
                    cur_kind = Some(k);
                }
                _ => {}
            },
            None => {
                if let (Some(ss), Some(ck)) = (seg_start, &cur_kind) {
                    flush(ss, byte_idx, ck, &mut ranges);
                }
                seg_start = None;
                cur_kind = None;
            }
        }
    }
    if let (Some(ss), Some(ck)) = (seg_start, &cur_kind) {
        flush(ss, text.len(), ck, &mut ranges);
    }
    ranges
}

/// token 是否有可翻译的内容（字母或汉字；纯标点、空白不算）
///
/// 注意区分两件事：**保留**在词表里、和**可以被命中**。
/// 标点必须保留（小句边界、句尾判定都靠它），但不该被当成"要查的那个词"。
///
/// 历史：这里是 `is_ascii_alphabetic` 的时候，汉字 token 被整批丢掉，
/// 导致悬浮取词/取句在中文上完全没反应。
#[inline]
pub fn is_meaningful_token(s: &str) -> bool {
    s.chars().any(|c| c.is_alphanumeric())
}

/// 是否同一视觉行（垂直方向有足够重叠）
#[inline]
fn same_visual_line(a: &WordBox, b: &WordBox) -> bool {
    let overlap = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    overlap >= 0.5 * a.h.min(b.h)
}

/// 是否整段都是汉字/假名
#[inline]
fn is_han_text(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| matches!(token_kind(c), Some(TokenKind::Han)))
}

/// 拼句子时判断要不要补空格：只在两个西文词之间补
#[inline]
fn needs_space_between(prev: char, next: char) -> bool {
    prev.is_ascii_alphanumeric() && next.is_ascii_alphanumeric()
}

/// 把 token 序列拼成文本：汉字之间不补空格，西文词之间补
///
/// 原来一律 `join(" ")`，中文会变成"图像 显示 一个人"。
fn join_tokens<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let mut s = String::new();
    for t in tokens {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        if let (Some(prev), Some(next)) = (s.chars().last(), t.chars().next()) {
            if needs_space_between(prev, next) {
                s.push(' ');
            }
        }
        s.push_str(t);
    }
    s.trim().to_string()
}

// ── BBox 计算 ─────────────────────────────────────────────

pub(crate) fn fallback_box(
    text_box: NormalizedRect,
    text: &str,
    range: &std::ops::Range<usize>,
) -> Option<NormalizedRect> {
    let total = if text.is_ascii() {
        text.len()
    } else {
        text.chars().count()
    };
    if total == 0 {
        return None;
    }
    let sc = if text.is_ascii() {
        range.start
    } else {
        text[..range.start].chars().count()
    };
    let ec = if text.is_ascii() {
        range.end
    } else {
        text[..range.end].chars().count()
    };
    if ec <= sc {
        return None;
    }
    let sf = sc as f64 / total as f64;
    let ef = ec as f64 / total as f64;
    let x = text_box.x + text_box.w * sf;
    let w = text_box.w * (ef - sf);
    if w <= 0.0 {
        return None;
    }
    Some(NormalizedRect {
        x,
        y: text_box.y,
        w,
        h: text_box.h,
    })
}

pub(crate) fn is_valid_box(b: NormalizedRect, parent: NormalizedRect) -> bool {
    if b.w <= 0.0 || b.h <= 0.0 {
        return false;
    }
    let tol = 0.02;
    b.x >= parent.x - tol
        && b.y >= parent.y - tol
        && (b.x + b.w) <= (parent.x + parent.w + tol)
        && (b.y + b.h) <= (parent.y + parent.h + tol)
}

pub(crate) fn is_compatible_subrange(
    precise: NormalizedRect,
    fallback: NormalizedRect,
    parent: NormalizedRect,
) -> bool {
    if precise.w <= 0.0 || fallback.w <= 0.0 {
        return false;
    }
    if precise.w >= parent.w * 0.8 && precise.w > fallback.w * 1.8 {
        return false;
    }
    let tol = (parent.w * 0.01).max(fallback.w * 0.35);
    if precise.x < fallback.x - tol {
        return false;
    }
    if precise.x + precise.w > fallback.x + fallback.w + tol {
        return false;
    }
    let overlap = (precise.x + precise.w).min(fallback.x + fallback.w) - precise.x.max(fallback.x);
    overlap.max(0.0) / precise.w.min(fallback.w) >= 0.35
}

pub(crate) fn resolved_box(
    precise: Option<NormalizedRect>,
    fallback: Option<NormalizedRect>,
    parent: NormalizedRect,
    is_sub: bool,
) -> Option<NormalizedRect> {
    if let Some(p) = precise {
        if is_valid_box(p, parent) {
            if is_sub {
                if let Some(f) = fallback {
                    if !is_compatible_subrange(p, f, parent) {
                        return fallback;
                    }
                }
            }
            return Some(p);
        }
    }
    fallback
}

// ── 命中检测 ──────────────────────────────────────

/// 命中光标的 token 下标
///
/// 三级退让：完整命中 → 容差命中 → 残向最近（吸附）。
/// 第三级是必要的：中文 OCR 是单字 box，而标点单独占一个 box，
/// 光标很容易正好落在字间空隙或标点上，只靠前两级会直接返回 None（窗口出不来）。
pub fn select_word_index(words: &[WordBox]) -> Option<usize> {
    if words.is_empty() {
        return None;
    }
    // 只有一个结果时直接返回，无需命中检测
    if words.len() == 1 {
        return Some(0);
    }

    const CX: f64 = 0.5;
    const CY: f64 = 0.5;
    const TOL: f64 = 0.004;
    /// 吸附半径（归一化）：约等于捕获宽度的 20%
    const SNAP: f64 = 0.2;

    let mut best_exact: Option<(f64, usize)> = None;
    let mut best_fuzzy: Option<(f64, usize)> = None;
    let mut best_near: Option<(f64, usize)> = None;

    for (i, w) in words.iter().enumerate() {
        let cx = w.x + w.w * 0.5;
        let cy = w.y + w.h * 0.5;
        let dist2 = (cx - CX).powi(2) + (cy - CY).powi(2);

        let in_exact = CX >= w.x && CX <= w.x + w.w && CY >= w.y && CY <= w.y + w.h;
        let in_fuzzy = !in_exact
            && CX >= w.x - TOL
            && CX <= w.x + w.w + TOL
            && CY >= w.y - TOL
            && CY <= w.y + w.h + TOL;

        if in_exact {
            if best_exact.map_or(true, |(d, _)| dist2 < d) {
                best_exact = Some((dist2, i));
            }
        } else if in_fuzzy {
            if best_fuzzy.map_or(true, |(d, _)| dist2 < d) {
                best_fuzzy = Some((dist2, i));
            }
        }

        if best_near.map_or(true, |(d, _)| dist2 < d) {
            best_near = Some((dist2, i));
        }
    }

    best_exact
        .or(best_fuzzy)
        .or_else(|| best_near.filter(|(d, _)| d.sqrt() <= SNAP))
        .map(|(_, i)| i)
}

/// 命中光标、且有意义（字母/汉字）的 token 下标：跳过标点
fn select_meaningful_index(words: &[WordBox]) -> Option<usize> {
    let candidates: Vec<usize> = (0..words.len())
        .filter(|&i| is_meaningful_token(&words[i].text))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let subset: Vec<WordBox> = candidates.iter().map(|&i| words[i].clone()).collect();
    let hit = select_word_index(&subset)?;
    Some(candidates[hit])
}

/// 悬浮取词（语言自适应）
///
/// - 英文：OCR 本身就有单词边界（"question"），直接取命中的那个 token
/// - 中文：Windows OCR 按**单字**给 box，数据里没有词边界 → 先把光标所在**整行**拼出来，
///   再交给分词器（jieba 词表 + 词频加权最短路径）切词，取包含光标那个字的词
///
/// 所以光标停在"设"或"计"上都得到"设计"，停在"改"上得到"改" ——
/// 不会再出现"设计改代码"这种跨词拼凑（那是上一版"左右各取 2 字"的毛病）。
pub fn select_word_smart(words: &[WordBox]) -> Option<String> {
    let hit = select_meaningful_index(words)?;
    let hit_text = words[hit].text.clone();

    if !is_han_text(&hit_text) {
        return Some(hit_text);
    }

    // 同一视觉行内、按 x 排好
    let mut line: Vec<usize> = (0..words.len())
        .filter(|&i| same_visual_line(&words[i], &words[hit]))
        .collect();
    line.sort_by(|&a, &b| {
        words[a]
            .x
            .partial_cmp(&words[b].x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 拼成整行文本，同时记下光标那个字在行内的字符下标
    let mut text = String::new();
    let mut hit_idx = 0usize;
    for &i in &line {
        let t = words[i].text.as_str();
        if t.trim().is_empty() {
            continue;
        }
        // 西文词之间补空格，避免 "AI" + "model" 粘成一个词
        if let (Some(prev), Some(next)) = (text.chars().last(), t.chars().next()) {
            if needs_space_between(prev, next) {
                text.push(' ');
            }
        }
        if i == hit {
            hit_idx = text.chars().count();
            // 命中的 token 里有多个字时（macOS Vision 会给整段汉字）：
            // 用光标在 box 内的相对位置估算具体是第几个字
            let count = t.chars().count();
            if count > 1 {
                let rel = ((CURSOR_NX - words[i].x) / words[i].w.max(f64::EPSILON))
                    .clamp(0.0, 1.0);
                hit_idx += ((rel * count as f64) as usize).min(count - 1);
            }
        }
        text.push_str(t);
    }

    // 分词后取光标所在的词；万一切不出来就退回命中的那个 token
    crate::utils::zh_segment::word_at_char(&text, hit_idx).or(Some(hit_text))
}

pub const CURSOR_NX: f64 = 0.5;
pub const WORD_GAP_FACTOR: f64 = 1.8;

fn ends_sentence(w: &WordBox) -> bool {
    matches!(
        w.text.trim_end().chars().last(),
        Some('.' | '!' | '?' | '。' | '！' | '？' | '…')
    )
}

pub fn select_sentence(words: &[WordBox]) -> Option<String> {
    if words.is_empty() {
        return None;
    }

    let mut sorted: Vec<&WordBox> = words.iter().collect();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    let mut widths: Vec<f64> = sorted.iter().map(|w| w.w).collect();
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_w = widths[widths.len() / 2];
    let gap_threshold = median_w * WORD_GAP_FACTOR;

    let pivot = sorted
        .iter()
        .position(|w| CURSOR_NX >= w.x && CURSOR_NX <= w.x + w.w)
        .unwrap_or_else(|| {
            sorted
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let da = (a.x + a.w * 0.5 - CURSOR_NX).abs();
                    let db = (b.x + b.w * 0.5 - CURSOR_NX).abs();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0)
        });

    let start = {
        let mut i = pivot;
        while i > 0 {
            let gap = sorted[i].x - (sorted[i - 1].x + sorted[i - 1].w);
            if gap > gap_threshold {
                break;
            }
            if ends_sentence(sorted[i - 1]) {
                break;
            }
            i -= 1;
        }
        i
    };

    let end = {
        let mut i = pivot;
        loop {
            if ends_sentence(sorted[i]) {
                i += 1;
                break;
            }
            if i + 1 >= sorted.len() {
                i += 1;
                break;
            }
            let gap = sorted[i + 1].x - (sorted[i].x + sorted[i].w);
            if gap > gap_threshold {
                i += 1;
                break;
            }
            i += 1;
        }
        i
    };

    // 拼接：只在两个西文词之间补空格（中文不能变成"图像 显示 一个人"）
    let s = join_tokens(sorted[start..end].iter().map(|w| w.text.as_str()));
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个 box：y 取在捕获中央附近（悬浮取词会以 (0.5,0.5) 为中心命中）
    fn wb(text: &str, x: f64, width: f64) -> WordBox {
        WordBox {
            text: text.to_string(),
            x,
            y: 0.48,
            w: width,
            h: 0.04,
        }
    }

    #[test]
    fn 汉字token不再被过滤() {
        // 用户报的问题原型：悬浮在中文上"完全没反应"，就是过滤器把汉字 token 丢掉了
        assert!(is_meaningful_token("图像"));
        assert!(is_meaningful_token("一个人"));
        assert!(is_meaningful_token("ephemeral"));
        assert!(is_meaningful_token("调用API"));
        // 标点保留在词表里（用来断句），但不该被当成"要查的那个词"
        assert!(!is_meaningful_token("，"));
        assert!(!is_meaningful_token(" "));
    }

    #[test]
    fn 中文取词按词边界切() {
        // 用户报的问题原型："按你选的设计改代码"，光标停在"设/计"上必须得到"设计"，
        // 不能给出"设计改代码"这种跨词拼凑
        let line = "按你选的设计改代码";
        let mut words = Vec::new();
        let mut x = 0.30;
        for ch in line.chars() {
            words.push(wb(&ch.to_string(), x, 0.04));
            x += 0.04;
        }
        // 捕获中心 0.5 落在"设"(0.46~0.50) / "计"(0.50~0.54) 交界
        assert_eq!(select_word_smart(&words).as_deref(), Some("设计"));
    }

    #[test]
    fn 中文取词跳过标点() {
        let words = vec![
            wb("图", 0.40, 0.04),
            wb("像", 0.44, 0.04),
            wb("，", 0.48, 0.04),
            wb("显", 0.52, 0.04),
        ];
        // 光标 0.5 正好落在标点上：跳过标点吸附到隔壁汉字，且结果一定不是标点
        assert_eq!(select_word_smart(&words).as_deref(), Some("图像"));
    }

    #[test]
    fn 英文取词仍然只取一个单词() {
        let words = vec![
            wb("the", 0.40, 0.06),
            wb("image", 0.48, 0.08),
            wb("shows", 0.58, 0.08),
        ];
        // image 命中 → 英文有自己的单词边界，不送去分词
        assert_eq!(select_word_smart(&words).as_deref(), Some("image"));
    }

    #[test]
    fn 中文取句能拿到带标点的整句() {
        let words = vec![
            wb("图像", 0.40, 0.06),
            wb("显示", 0.48, 0.06),
            wb("一个人", 0.56, 0.08),
            wb("。", 0.66, 0.03),
        ];
        assert_eq!(
            select_sentence(&words).as_deref(),
            Some("图像显示一个人。")
        );
    }

    #[test]
    fn 汉字段整体成一个token_英文段做驼峰拆分() {
        let src = "调用APIRequest时";
        let toks: Vec<&str> = script_aware_ranges(src)
            .iter()
            .map(|r| &src[r.clone()])
            .collect();
        assert_eq!(toks, vec!["调用", "API", "Request", "时"]);
    }

    #[test]
    fn 中文选句不插空格() {
        let words = vec![
            wb("图像", 0.44, 0.04),
            wb("显示", 0.49, 0.04),
            wb("一个人", 0.54, 0.05),
            wb("坐在", 0.60, 0.04),
        ];
        assert_eq!(
            select_sentence(&words).as_deref(),
            Some("图像显示一个人坐在")
        );
    }

    #[test]
    fn 中英混排选句只在西文词间插空格() {
        let words = vec![
            wb("如图", 0.44, 0.04),
            wb("AI", 0.49, 0.03),
            wb("model", 0.53, 0.05),
            wb("所示", 0.59, 0.04),
        ];
        assert_eq!(
            select_sentence(&words).as_deref(),
            Some("如图AI model所示")
        );
    }

    #[test]
    fn 英文选句仍然插空格() {
        let words = vec![
            wb("the", 0.44, 0.03),
            wb("image", 0.48, 0.05),
            wb("shows", 0.54, 0.05),
        ];
        assert_eq!(select_sentence(&words).as_deref(), Some("the image shows"));
    }
}
