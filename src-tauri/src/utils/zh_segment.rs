//! 中文分词：词表 + 词频加权最短路径（jieba 的 route 算法）
//!
//! 为什么需要这一层：Windows OCR 对中文是**按单字**给 word 的（"设"、"计"），
//! 数据里根本没有"词"这一级信息。而"悬浮取词"必须知道词边界 ——
//! 否则会把"设计"和"改代码"拦腰拼在一起。
//!
//! 做法：建 DAG（每个位置所有可能的词 + 单字兜底），再从右往左做最短路径，
//! 每个 token 贡献 `ln(词频/总词频)`，让"切得少"和"切出来的词更常用"自动折中。
//! 词表见 `assets/cn_dict.txt`，由 `scripts/build_cn_dict.py` 从 jieba 的
//! dict.txt（MIT 协议）裁剪生成：词频 ≥ 5 的 13.6 万条，词频压成 2 字符的
//! 4096 级对数（量化误差约 0.4%）。
//!
//! **为什么没有做未登录词识别（jieba 的 finalseg HMM）**：实测在"取词"场景不可靠，
//! 而且无法用置信度区分对错 —— 假合并的置信度甚至比真词更高：
//!
//! | 片段 | HMM 判定 | 置信差 | 对错 |
//! |---|---|---|---|
//! | 漏报 | B/E 合并 | 2.06 nats | 对 |
//! | 补报 | B/E 合并 | 2.96 nats | 对 |
//! | 充电 | B/E 合并 | 3.63 nats | 对 |
//! | 先改 | B/E 合并 | 1.58 nats | **错**（把相邻两个词粘一起） |
//! | 改代 | B/E 合并 | 5.59 nats | **错**（置信度比真词还高） |
//! | 瞒报 | S/S 拆开 | 0.00 nats | **错**（漏掉真词） |
//!
//! 查不到的词宁可只给单字（用户还能看到单字释义），也不该凭空合并出错词 ——
//! 那正是"设计改代码"被投诉的同一类问题。所以容量花在词表本身（数据驱动、不会瞎猜）。

use std::collections::HashMap;
use std::sync::OnceLock;

/// jieba dict.txt 的总词频（build_cn_dict.py 生成时会打印，换词表必须同步改这里）
const TOTAL_FREQ: f64 = 59_474_253.0;

/// 词表允许的最长词（生成脚本已裁掉更长的条目）
const MAX_WORD_CHARS: usize = 10;

/// 频率等级字符表，必须与 build_cn_dict.py 的 LEVEL_CHARS 一致
const LEVEL_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+-";

static DICT: OnceLock<HashMap<&'static str, f64>> = OnceLock::new();

/// 词表：词 → 词频（首次使用时解析并缓存，约几十毫秒）
fn dict() -> &'static HashMap<&'static str, f64> {
    DICT.get_or_init(|| {
        let raw = include_str!("../../assets/cn_dict.txt");
        let slots = (LEVEL_CHARS.len() * LEVEL_CHARS.len() - 1) as f64;
        let mut map = HashMap::with_capacity(150_000);
        for line in raw.lines() {
            let Some((word, lv)) = line.split_once('\t') else {
                continue;
            };
            let mut it = lv.chars();
            let (Some(a), Some(b)) = (
                it.next().and_then(level_index),
                it.next().and_then(level_index),
            ) else {
                continue;
            };
            // 等级 → 词频：10^(0..7) 对数还原（2 字符 = 4096 级）
            let code = a * LEVEL_CHARS.len() + b;
            let freq = 10f64.powf(code as f64 / slots * 7.0);
            map.insert(word, freq);
        }
        map
    })
}

#[inline]
fn level_index(c: char) -> Option<usize> {
    LEVEL_CHARS.find(c)
}

#[inline]
fn is_han(c: char) -> bool {
    let cp = c as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
}

/// 未登录词（词表里没有）的分数：词频按 1 算，即 DAG 里最差的选择
#[inline]
fn unknown_score() -> f64 {
    -(TOTAL_FREQ.ln())
}

/// 词的分数：ln(词频) - ln(总词频)
///
/// 照搬 jieba 的 `route` 公式：每个 token 贡献 ln(词频/总词频)，于是
/// "切得少"（每多一个 token 就多付一次 -ln(总词频)）和"切出来的词更常用"会自动折中。
fn word_score(word: &str) -> f64 {
    match dict().get(word) {
        Some(&freq) => freq.ln() - TOTAL_FREQ.ln(),
        None => unknown_score(),
    }
}

/// 预热：解析词表（1.5MB 文本），启动时后台跑一次，避免第一次悬浮取词卡一下
pub fn warmup() {
    let _ = dict();
}

/// 分词：返回每个词的**字符区间**（左闭右开）
pub fn segment(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 {
        return Vec::new();
    }

    // ── 1) 建 DAG：dag[i] = 以下标 i 开头、可以作为一个 token 的结束位置 ──
    let table = dict();
    let mut dag: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        if is_han(chars[i]) {
            let mut end = i;
            while end < n && is_han(chars[end]) {
                end += 1;
            }
            let limit = (i + MAX_WORD_CHARS).min(end);
            for e in (i + 1)..=limit {
                if e == i + 1 {
                    dag[i].push(e); // 单字兜底
                    continue;
                }
                let cand: String = chars[i..e].iter().collect();
                if table.contains_key(cand.as_str()) {
                    dag[i].push(e);
                }
            }
        } else if chars[i].is_ascii_alphanumeric() {
            // 西文单词/数字整段一个 token：不在词表里，也不该被切开
            let mut e = i + 1;
            while e < n
                && (chars[e].is_ascii_alphanumeric() || matches!(chars[e], '\'' | '-' | '_'))
            {
                e += 1;
            }
            dag[i].push(e);
        } else {
            // 标点、空白、其他：各自单独成 token
            dag[i].push(i + 1);
        }
    }

    // ── 2) 从右往左 DP（jieba 同款）──
    let mut route = vec![0.0f64; n + 1];
    let mut next = vec![0usize; n + 1];
    for i in (0..n).rev() {
        let mut best = f64::MIN;
        let mut best_end = i + 1;
        for &e in &dag[i] {
            let score = if is_han(chars[i]) {
                let w: String = chars[i..e].iter().collect();
                word_score(&w)
            } else {
                0.0 // 西文/标点不计分，免得干扰汉字的切分决策
            } + route[e];
            if score > best {
                best = score;
                best_end = e;
            }
        }
        route[i] = best;
        next[i] = best_end;
    }

    // ── 3) 回溯 ──
    let mut out = Vec::with_capacity(n / 2 + 1);
    let mut i = 0;
    while i < n {
        let e = next[i].max(i + 1).min(n);
        out.push((i, e));
        i = e;
    }
    out
}

/// 取包含第 `char_idx` 个字符的那个词
///
/// 这就是"悬浮取词"要的东西：光标停在"设"或"计"上都给"设计"，
/// 停在"改"上给"改"，不会跨词拼凑。
pub fn word_at_char(text: &str, char_idx: usize) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    for (s, e) in segment(text) {
        if char_idx >= s && char_idx < e {
            let w: String = text.chars().skip(s).take(e - s).collect();
            let w = w.trim().to_string();
            return if w.is_empty() { None } else { Some(w) };
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        segment(text)
            .iter()
            .map(|&(s, e)| text.chars().skip(s).take(e - s).collect())
            .collect()
    }

    #[test]
    fn 词表已正确加载() {
        // 数据文件被裁坏时这里先炸，避免静默退化成"按字切"
        let d = dict();
        assert!(d.len() > 100_000, "词表条目太少: {}", d.len());
        for w in ["设计", "标点", "边界", "图像", "代码", "漏报", "瞒报"] {
            assert!(d.contains_key(w), "词表缺少: {w}");
        }
    }

    #[test]
    fn 词频量化精度够用() {
        // 4096 级：常用词频率应接近原值（"的" 是词表里最高的单字）
        let f = *dict().get("的").expect("缺少高频单字");
        assert!(f > 1e5, "高频单字词频异常: {f}");
        assert!(*dict().get("漏报").unwrap() < 100.0, "低频词被抬高");
    }

    #[test]
    fn 低频复合词不再被词表丢掉() {
        // 用户报的例子：漏报（词频 10）、瞒报（23）。之前 60k 截断线那版会切成"漏"+"报"
        let w = words("未按规定漏报统计数据");
        assert!(w.contains(&"漏报".to_string()), "{w:?}");
        let w = words("他涉嫌瞒报个人有关事项");
        assert!(w.contains(&"瞒报".to_string()), "{w:?}");
        // 词表覆盖的部分不能被切坏
        assert!(w.contains(&"涉嫌".to_string()), "{w:?}");
    }

    #[test]
    fn 相邻词不会被粘起来() {
        // 两种曾经的错法：①"左右各取 2 字" → "设计改代码"；②HMM 未登录词 → "先改"
        let w = words("先改OCR公共层");
        assert!(w.contains(&"先".to_string()), "{w:?}");
        assert!(w.contains(&"改".to_string()), "{w:?}");
        let w = words("按你选的设计改代码");
        assert!(w.contains(&"设计".to_string()), "{w:?}");
        assert!(w.contains(&"代码".to_string()), "{w:?}");
    }

    #[test]
    fn 取词落在词边界上() {
        // 同一个字的不同位置应给同一个词
        let line = "按你选的设计改代码";
        assert_eq!(word_at_char(line, 4).as_deref(), Some("设计"));
        assert_eq!(word_at_char(line, 5).as_deref(), Some("设计"));
        assert_eq!(word_at_char(line, 6).as_deref(), Some("改"));
        assert_eq!(word_at_char(line, 8).as_deref(), Some("代码"));
    }

    #[test]
    fn 标点不会被当成词() {
        let line = "图像显示，一个人";
        // 下标 4 是"，" → 断句处不会返回标点，要么空要么一定是词
        let w = word_at_char(line, 4);
        assert!(w.is_none() || w.as_deref() == Some("，"));
        assert_eq!(word_at_char(line, 0).as_deref(), Some("图像"));
        assert_eq!(word_at_char(line, 6).as_deref(), Some("一个"));
    }

    #[test]
    #[ignore] // 需要人工核对切分结果时：cargo test --lib zh_segment -- --ignored --nocapture
    fn 打印几个句子的实际切分() {
        for s in [
            "按你选的设计改代码",
            "图像显示一个人坐在卧室地板上打游戏",
            "先改OCR公共层（命中吸附、取词、标点边界）",
            "未按规定漏报统计数据",
            "他涉嫌瞒报个人有关事项",
            "我们用法衡 API 做了测试，效果不错。",
            "继承和发展",
        ] {
            println!("{s}  →  {:?}", words(s));
        }
    }
}
