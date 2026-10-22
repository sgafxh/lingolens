//! 翻译后端：法衡-H1（OpenAI 兼容 /chat/completions）
//!
//! 说明：上游仓库里这个文件是私有的（`.gitignore` 标记为 Private files，走有道的
//! 非官方接口）。本构建改用法衡-H1，理由与取舍见下：
//!
//! * 速度：实测单词 0.14~0.21s、三段段落 0.25s（与有道 0.13~0.15s 基本持平）。
//!   **前提是必须显式带 `reasoning_effort: "none"`** —— 默认开着思维链时同样的
//!   请求要 1.0~1.45s（慢 7 倍），因为返回的 token 里 90% 以上是推理内容。
//! * 分段：法衡-H1 会原样保留原文的 `\n\n`，所以"划词多段被挤成一大段"这个问题
//!   在数据源头就消失了（前端仍保留启发式重排作为兜底）。
//! * 代价：拿不到有道词典的结构化字段（音标、真人发音、词形变化、同近义词），
//!   单词模式退化为"单词 + 释义"。因此 `wordCard.show` 恒为 false，不渲染词典卡片。

use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use crate::utils::debug_log;
use crate::utils::define_config::{AppConfig, CONFIG_FILE_NAME};

/// 单次翻译的超时时间
const REQUEST_TIMEOUT_SECS: u64 = 20;
/// 判定为"查词"而非"句子"的最大字符数（英文）
const WORD_MAX_CHARS: usize = 30;
/// 中文"单词"的字数上限。
///
/// 中文没有空格，一句中文（"图像显示一个人坐在卧室地板上打游戏"）也才 20 多个字，
/// 拿英文的 30 字符标准套就会把整句当成一个单词，于是走了词典提示词，
/// 输出变成 "n. / 图像 / 画面 / 影像" —— 看起来就是"没翻译"。
/// 所以中文按字数卡：词 / 词组（法律、有期徒刑）算查词，再长就当句子翻译。
const CJK_WORD_MAX_CHARS: usize = 6;
/// 汉字占比达到这个值就算"全中文"（原文里夹几个英文术语仍按中文算）
const CJK_RATIO_CHINESE: f64 = 0.6;
/// 汉字占比低于这个值就算"全英文"
const CJK_RATIO_ENGLISH: f64 = 0.3;

/// 翻译 API 返回的完整词典数据（前端 TransData 对应结构）
///
/// `voice` / `translate` / `word_card` 保持 `serde_json::Value`，与上游一致，
/// 这样前端类型定义不用改。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictResponse {
    /// 发音数据（音标、音频地址等）。本后端无音标，恒为空字符串。
    pub voice: Value,
    /// 翻译结果（译文、来源语言、目标语言等）
    pub translate: Value,
    /// 单词卡片（释义、词形变化、同近义词等）—— 注释：上游字段名 wordCard
    #[serde(rename = "wordCard")]
    pub word_card: Value,
}

/// 前端统一的翻译查询结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransResult {
    /// 状态码：200 成功，-1 失败
    pub status: i16,
    /// 错误消息（仅失败时存在）
    pub msg: Option<String>,
    /// 翻译结果数据（仅成功时存在）
    pub data: Option<DictResponse>,
}

impl TransResult {
    pub fn success(data: DictResponse) -> Self {
        Self {
            status: 200,
            msg: None,
            data: Some(data),
        }
    }

    pub fn fail(msg: String) -> Self {
        Self {
            status: -1,
            msg: Some(msg),
            data: None,
        }
    }
}

/// 原文的语言构成
#[derive(Debug, Clone, Copy, PartialEq)]
enum LangKind {
    /// 基本全是中文（夹少量英文词也算）
    Chinese,
    /// 基本全是英文（夹少量中文也算）
    English,
    /// 中英混排，两边都有实质内容
    Mixed,
}

/// 目标语言
#[derive(Debug, Clone, Copy, PartialEq)]
enum Target {
    Chinese,
    English,
    /// 原文已经是目标语言，原样返回（不浪费一次请求）
    KeepAsIs,
}

/// 查词提示词（英文单词 → 中文释义）
const SYSTEM_WORD_ZH: &str = "\
You are a compact bilingual dictionary. For the given word, output ONLY its part(s) of speech \
and concise Chinese meanings.
Format rules:
- Line 1: part of speech, e.g. `n.` or `adj.; n.`
- Following lines: one short Chinese meaning per line, each under 14 characters, most common first, at most 4 lines
- Output must be in Chinese; never output the original English word
- No pronunciation, no examples, no explanations, no bullet symbols
- Output plain text only, never wrap the answer in code fences";

/// 查词提示词（中文词 → 英文释义）
///
/// 单独写一份而不是靠"if the word is Chinese"一句带过：实测那句带过时，
/// 中文词 `有期徒刑` 会返回中文同义词（"有期监禁"），看起来像没翻译。
const SYSTEM_WORD_EN: &str = "\
You are a compact bilingual dictionary. For the given Chinese word, output ONLY its part \
of speech and concise English meanings.
Format rules:
- Line 1: part of speech, e.g. `n.` or `adj.; n.`
- Following lines: one short English meaning per line, most common first, at most 4 lines
- Output must be in English; never output the original Chinese word or Chinese synonyms
- No pronunciation, no examples, no explanations, no bullet symbols
- Output plain text only, never wrap the answer in code fences";

/// 句子/段落提示词的公共部分
const SENTENCE_RULES: &str = "\
Output rules (must follow strictly):
- Output ONLY the translation, nothing else
- Keep the original paragraph structure: every blank line and every line break in the source must \
appear in the same place in your output
- No explanations, no notes, no quotes around the whole answer, no code fences
- Preserve inline Markdown syntax (**bold**, `code`, lists, headings) instead of describing it
- Keep code identifiers, file paths, numbers and proper nouns that have no established Chinese \
equivalent unchanged";

/// 判断是不是中日韩字符（这里主要关心汉字）
fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF      // 汉字扩展 A
        | 0x4E00..=0x9FFF    // 汉字基本区
        | 0xF900..=0xFAFF    // 兼容汉字
        | 0x3040..=0x30FF    // 日文假名
        | 0xAC00..=0xD7AF // 韩文
    )
}

/// 数出"汉字字符数"和"英文单词数"
///
/// 两边都用各自的"内容单位"计数：汉字按字，英文按词。
/// 不能直接比字符数 —— `用 Transformer 做翻译` 里 11 个英文字母对 4 个汉字，
/// 按字符数算会得出"英文为主"的错误结论。
fn lang_units(text: &str) -> (usize, usize) {
    let mut cjk = 0usize;
    let mut words = 0usize;
    let mut in_word = false;

    for ch in text.chars() {
        if is_cjk(ch) {
            cjk += 1;
            in_word = false;
        } else if ch.is_alphabetic() {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }

    (cjk, words)
}

/// 判断原文的语言构成。
///
/// 以前是把"含中文"直接当成"中文原文"，于是英文段落里只要夹一个中文词（人名、术语）
/// 就会被判定成中文→英文方向，输出看起来像"没翻译"。现在按两边内容单位的占比判断：
/// * 汉字占 60% 以上 —— 全中文（夹几个英文术语也算）→ 反向译成英文
/// * 汉字占 30% 以下 —— 全英文（夹少量汉字也算）→ 译成中文
/// * 中间地带 —— 真·中英混排 → 把英文部分译成中文
fn lang_kind(text: &str) -> LangKind {
    let (cjk_chars, latin_words) = lang_units(text);
    let total = cjk_chars + latin_words;

    if total == 0 {
        // 纯数字 / 符号：按英文处理（译成中文）更常见
        return LangKind::English;
    }

    let ratio = cjk_chars as f64 / total as f64;
    if ratio >= CJK_RATIO_CHINESE {
        LangKind::Chinese
    } else if ratio <= CJK_RATIO_ENGLISH {
        LangKind::English
    } else {
        LangKind::Mixed
    }
}

/// 根据配置项 `llm_target_lang` 和原文语言决定目标语言
///
/// * `auto`（默认）—— 全中文 → 英文；全英文 / 中英混排 → 中文
/// * `zh` —— 一律中文（原文已是中文时原样返回）
/// * `en` —— 一律英文（原文已是英文时原样返回）
fn resolve_target(mode: &str, kind: LangKind) -> Target {
    match mode {
        "zh" => {
            if kind == LangKind::Chinese {
                Target::KeepAsIs
            } else {
                Target::Chinese
            }
        }
        "en" => {
            if kind == LangKind::English {
                Target::KeepAsIs
            } else {
                Target::English
            }
        }
        _ => match kind {
            LangKind::Chinese => Target::English,
            LangKind::English | LangKind::Mixed => Target::Chinese,
        },
    }
}

/// 叠加"触发方式"的影响：鼠标悬浮取词 / 取句时，中文原文固定译成英文。
///
/// 悬浮的语义是"看看这段话说的是什么（另一种语言）"，所以不受 `llm_target_lang` 影响。
fn resolve_target_with_origin(mode: &str, kind: LangKind, origin: &str) -> Target {
    if origin == "hover" && kind == LangKind::Chinese {
        return Target::English;
    }
    resolve_target(mode, kind)
}

/// 组装句子/段落的 system 提示词
fn build_sentence_prompt(target: Target, kind: LangKind) -> String {
    let mut prompt = String::from("You are a professional translator.\n");

    match target {
        Target::Chinese => prompt.push_str("- Translate the user's text into simplified Chinese.\n"),
        Target::English => prompt.push_str("- Translate the user's text into natural English.\n"),
        Target::KeepAsIs => {
            prompt.push_str("- The text is already in the target language; output it unchanged.\n")
        }
    }

    if target == Target::Chinese && kind == LangKind::Mixed {
        prompt.push_str(
            "- The source mixes Chinese and English: translate the English parts into simplified \
             Chinese, and keep the parts that are already Chinese as they are.\n",
        );
    }

    prompt.push_str(SENTENCE_RULES);
    prompt
}

/// 判断是查词还是句子
///
/// 英文：不含空白、长度够短的单个词（ephemeral / prison）
/// 中文：按**字数**卡（中文没有空格，不能拿英文字符数标准套）
fn is_single_word(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.chars().any(|c| c.is_whitespace()) {
        return false;
    }

    let count = t.chars().count();

    if t.chars().any(is_cjk) {
        // 法律 / 有期徒刑 这类词或词组算查词；整句走翻译
        return count <= CJK_WORD_MAX_CHARS;
    }

    // 至少包含一个字母，排除纯符号
    count <= WORD_MAX_CHARS && t.chars().any(|c| c.is_alphabetic())
}

/// 查词提示词按目标语言选：英文单词→中文释义，中文词→英文释义
fn word_prompt(target: Target) -> &'static str {
    match target {
        Target::English => SYSTEM_WORD_EN,
        _ => SYSTEM_WORD_ZH,
    }
}

/// 清掉模型可能自作主张加的代码围栏
fn strip_code_fence(text: &str) -> String {
    let t = text.trim();
    if !t.starts_with("```") {
        return t.to_string();
    }
    let without_open = match t.find('\n') {
        Some(idx) => &t[idx + 1..],
        None => return t.to_string(),
    };
    match without_open.rfind("```") {
        Some(idx) => without_open[..idx].trim().to_string(),
        None => without_open.trim().to_string(),
    }
}

/// 从 OpenAI 兼容响应里取出正文
fn extract_content(body: &Value) -> Result<String, String> {
    let choice = body
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| "翻译服务返回异常：没有 choices 字段".to_string())?;

    let message = choice.get("message").unwrap_or(&Value::Null);
    let content = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim();

    if content.is_empty() {
        // 有些实现会返回 finish_reason=length 且正文为空
        let reason = choice
            .get("finish_reason")
            .and_then(|r| r.as_str())
            .unwrap_or("unknown");
        return Err(format!("翻译服务返回空内容（finish_reason={reason}）"));
    }

    Ok(strip_code_fence(content))
}

/// 日志里回显文本时截断，避免日志被长文撑满
fn brief(text: &str) -> String {
    let cleaned = text.replace('\n', " ⏎ ");
    let mut out: String = cleaned.chars().take(80).collect();
    if cleaned.chars().count() > 80 {
        out.push('…');
    }
    out
}

/// 判断"该翻译却看起来没翻译"
///
/// 实测过：同一提示词、同一输入，模型偶尔会把原文原样吐回来（temperature=0 也会），
/// 表现出来就是用户看到的"没翻译 / 怎么还是原文"。这里做一次体检，命中就重试。
fn looks_untranslated(source: &str, output: &str, target: Target) -> bool {
    let source = source.trim();
    let output = output.trim();

    if output.is_empty() || output == source {
        return true;
    }

    match target {
        // 要英文却还是中文（≥30% 汉字就算中文）
        Target::English => lang_kind(output) == LangKind::Chinese,
        // 要中文却还是英文
        Target::Chinese => lang_kind(output) == LangKind::English,
        Target::KeepAsIs => false,
    }
}

/// 单次请求：发出去、取回正文
async fn request_once(
    client: &reqwest::Client,
    url: &str,
    model: &str,
    api_key: &str,
    text: &str,
    system_prompt: &str,
) -> Result<String, String> {
    let payload = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": text }
        ],
        "temperature": 0,
        // 关键：不开思维链。实测开与不开是 1.0s 与 0.14s 的差别。
        "reasoning_effort": "none",
        "stream": false
    });

    let resp = client
        .post(url)
        .bearer_auth(api_key.trim())
        .json(&payload)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("翻译请求超时（>{REQUEST_TIMEOUT_SECS}s）")
            } else if e.is_connect() {
                "无法连接翻译服务，请检查网络或 base_url".to_string()
            } else {
                format!("翻译请求失败：{e}")
            }
        })?;

    let status = resp.status();
    let raw = resp
        .text()
        .await
        .map_err(|e| format!("读取翻译响应失败：{e}"))?;

    if !status.is_success() {
        let brief: String = raw.chars().take(200).collect();
        return Err(format!("翻译服务返回 {status}：{brief}"));
    }

    let body: Value =
        serde_json::from_str(&raw).map_err(|e| format!("翻译响应不是合法 JSON：{e}"))?;

    extract_content(&body)
}

/// 调用法衡-H1 完成一次翻译（带"看起来没翻译"的兜底重试）
#[allow(clippy::too_many_arguments)]
async fn translate_with_faheng(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
    is_word: bool,
    target: Target,
    kind: LangKind,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("未配置法衡 API Key：请在配置文件里填入 llm_api_key".to_string());
    }

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let system_prompt = if is_word {
        word_prompt(target).to_string()
    } else {
        build_sentence_prompt(target, kind)
    };

    let first = request_once(client, &url, model, api_key, text, &system_prompt).await?;

    if !looks_untranslated(text, &first, target) {
        return Ok(first);
    }

    // 重试一次：换一句更硬的提示词（同提示词重试可能还是同一个输出）
    debug_log::log(
        "translate_retry",
        &format!("首次输出疑似未翻译（target={target:?}），换个提示词重试一次"),
    );

    let retry_prompt = format!(
        "{system_prompt}\n- The previous attempt returned the text in its original language. \
         Translate it properly this time; do not copy the original text."
    );

    match request_once(client, &url, model, api_key, text, &retry_prompt).await {
        Ok(second) => Ok(second),
        // 重试请求本身失败，就先用首次结果，不要因为重试把能用的结果丢了
        Err(_) => Ok(first),
    }
}

/// 读翻译后端配置：**直接读配置文件**，不走 store 插件的内存缓存。
///
/// 原因：store 插件启动后就在内存里缓存了一份，外部编辑 config.json（比如改
/// `llm_target_lang` 切目标语言）在重启前不会生效。而改这个文件正是我们给用户的
/// 切换方式，所以要每次现读（文件才几百字节）。读不到就用内存里的值兜底。
fn read_llm_settings(app: &AppHandle, fallback: &AppConfig) -> (String, String, String, String) {
    let from_file = app
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(CONFIG_FILE_NAME))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value.get("config").cloned());

    let pick = |key: &str, default: &str| -> String {
        from_file
            .as_ref()
            .and_then(|config| config.get(key))
            .and_then(|value| value.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| default.to_string())
    };

    (
        pick("llm_base_url", &fallback.llm_base_url),
        pick("llm_model", &fallback.llm_model),
        pick("llm_target_lang", &fallback.llm_target_lang),
        pick("llm_api_key", &fallback.llm_api_key),
    )
}

/// 供 `fetch_trans_res` 命令调用
pub async fn fetch_translation(
    word: &str,
    state: &State<'_, AppState>,
    app: &AppHandle,
) -> Result<DictResponse, String> {
    let text = word.trim();
    if text.is_empty() {
        return Err("没有取到要翻译的内容".to_string());
    }

    // 配置在 Mutex 里，先克隆出来再 await，避免把锁带过 await 点
    let (base_url, model, api_key, target_mode, origin, client) = {
        let fallback = state
            .config
            .lock()
            .map_err(|_| "读取配置失败（配置锁已损坏）".to_string())?
            .clone();

        // 每次现读配置文件，外部改动立即生效
        let (base_url, model, target_mode, api_key) = read_llm_settings(app, &fallback);

        // 本次是怎么触发的：selection（划词）/ hover（悬浮取词、取句）/ input（搜索框）
        let origin = state
            .last_origin
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();

        (
            base_url,
            model,
            api_key,
            target_mode,
            origin,
            state.client.clone(),
        )
    };

    let is_word = is_single_word(text);
    let kind = lang_kind(text);
    let target = resolve_target_with_origin(&target_mode, kind, &origin);

    debug_log::log(
        "translate",
        &format!(
            "origin={origin} mode={target_mode} source={kind:?} target={target:?} is_word={is_word} chars={}",
            text.chars().count()
        ),
    );
    debug_log::log("translate_in", &brief(text));

    let translation = if target == Target::KeepAsIs {
        // 原文已经是目标语言：直接返回，省一次请求
        text.to_string()
    } else {
        translate_with_faheng(
            &client,
            &base_url,
            &model,
            &api_key,
            text,
            is_word,
            target,
            kind,
        )
        .await?
    };

    debug_log::log("translate_out", &brief(&translation));

    // 保持与上游一致的字段形状，前端不需要改类型定义
    let qc_type = if is_word { "word" } else { "sentence" };
    let translate = json!({
        "zly": "faheng",
        "errorCode": "0",
        "qc_type": qc_type,
        "index": "0",
        "from": "auto",
        "source": "faheng",
        "text": text,
        "to": target_mode,
        "id": uuid::Uuid::new_v4().to_string(),
        "dit": translation,
        "orig_text": text,
        "md5": format!("{:x}", md5::compute(text.as_bytes())),
    });

    // show=false：本后端没有音标/词形/同近义词这类结构化词典数据，
    // 不渲染词典卡片，避免出现空壳 UI。
    let word_card = json!({
        "title": text,
        "show": false,
        "usualDict": [],
        "secondQuery": "",
        "exchange": {},
        "levelList": [],
    });

    Ok(DictResponse {
        voice: json!(""),
        translate,
        word_card,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 纯语言判定() {
        assert_eq!(lang_kind("这个模型没有正确回答问题。"), LangKind::Chinese);
        assert_eq!(
            lang_kind("The model failed to answer the question."),
            LangKind::English
        );
    }

    #[test]
    fn 英文夹一个中文词还算英文() {
        // 这是用户报的问题原型：含一个汉字就被当成中文→英文，输出看着像没翻译
        assert_eq!(
            lang_kind("The model failed to answer 法衡 correctly."),
            LangKind::English
        );
        assert_eq!(
            lang_kind("Call the api.faheng.chat 接口 to translate text."),
            LangKind::English
        );
    }

    #[test]
    fn 中文夹英文术语还算中文() {
        assert_eq!(
            lang_kind("我们用法衡 API 做了测试，效果不错。"),
            LangKind::Chinese
        );
        assert_eq!(
            lang_kind("用 Transformer 做翻译效果一般。"),
            LangKind::Chinese
        );
    }

    #[test]
    fn 真混排() {
        assert_eq!(
            lang_kind("这个 model very good 很好用 and fast 快"),
            LangKind::Mixed
        );
    }

    #[test]
    fn 纯数字符号不报错() {
        assert_eq!(lang_kind("1234567890 ==="), LangKind::English);
    }

    #[test]
    fn 方向矩阵_按用户要求() {
        // 全中文 → 英文
        assert_eq!(resolve_target("auto", LangKind::Chinese), Target::English);
        // 全英文 → 中文
        assert_eq!(resolve_target("auto", LangKind::English), Target::Chinese);
        // 中英混排 → 中文（英文部分译成中文）
        assert_eq!(resolve_target("auto", LangKind::Mixed), Target::Chinese);

        assert_eq!(resolve_target("zh", LangKind::English), Target::Chinese);
        assert_eq!(resolve_target("zh", LangKind::Mixed), Target::Chinese);
        assert_eq!(resolve_target("zh", LangKind::Chinese), Target::KeepAsIs);

        assert_eq!(resolve_target("en", LangKind::Chinese), Target::English);
        assert_eq!(resolve_target("en", LangKind::Mixed), Target::English);
        assert_eq!(resolve_target("en", LangKind::English), Target::KeepAsIs);
    }

    #[test]
    fn 识别未翻译并触发重试() {
        // 原样返回
        assert!(looks_untranslated(
            "我们用法衡 API 做了测试。",
            "我们用法衡 API 做了测试。",
            Target::English
        ));
        // 要英文，结果还是中文
        assert!(looks_untranslated(
            "我们用法衡 API 做了测试。",
            "我们使用法衡 API 进行了测试。",
            Target::English
        ));
        // 要中文，结果还是英文
        assert!(looks_untranslated(
            "We tested the API.",
            "We tested the API.",
            Target::Chinese
        ));
        // 正常英文输出不该误报
        assert!(!looks_untranslated(
            "我们用法衡 API 做了测试。",
            "We tested the Faheng API.",
            Target::English
        ));
        // 中英混排译成中文时，译文里保留中文部分是正常的，不能误报
        assert!(!looks_untranslated(
            "### Visual Analysis\nThe image depicts a person.",
            "### 视觉分析\n图像描绘了一个人。",
            Target::Chinese
        ));
    }

    #[test]
    fn 中文句子不能被当成单词() {
        // 用户报的问题原型：整句中文走了词典提示词，输出成了 "n. 图像 / 画面 / 影像"
        assert!(!is_single_word("图像显示一个人坐在卧室地板上打游戏"));
        assert!(!is_single_word("这个模型没有正确回答问题说明提示词有歧义"));
        // 词 / 词组仍算查词
        assert!(is_single_word("有期徒刑"));
        assert!(is_single_word("法律"));
    }

    #[test]
    fn 英文单词判定不受影响() {
        assert!(is_single_word("ephemeral"));
        assert!(is_single_word("prison"));
        assert!(!is_single_word("Hello world"));
        // 超过 30 字符的单词不再当查词（长长的驼峰标识符）
        assert!(!is_single_word("AuthnTokenValidationFailureRetryPolicy"));
    }

    #[test]
    fn 查词提示词按目标语言选() {
        assert!(word_prompt(Target::English).contains("concise English meanings"));
        assert!(word_prompt(Target::Chinese).contains("concise Chinese meanings"));
        // 中文词 → 英文释义的提示词不能出现"Chinese meanings"这种反方向要求
        assert!(!word_prompt(Target::English).contains("concise Chinese meanings"));
    }

    #[test]
    fn 悬浮取词遇中文固定译英文() {
        // 用户加的规则：鼠标悬浮取词 / 取句，原文是中文 → 英文
        assert_eq!(
            resolve_target_with_origin("auto", LangKind::Chinese, "hover"),
            Target::English
        );
        // 即使配置成一律中文，悬浮路径也要出英文
        assert_eq!(
            resolve_target_with_origin("zh", LangKind::Chinese, "hover"),
            Target::English
        );
        // 悬浮取词遇到英文原文：还是译成中文
        assert_eq!(
            resolve_target_with_origin("auto", LangKind::English, "hover"),
            Target::Chinese
        );
        // 划词路径不受影响
        assert_eq!(
            resolve_target_with_origin("zh", LangKind::Chinese, "selection"),
            Target::KeepAsIs
        );
        assert_eq!(
            resolve_target_with_origin("auto", LangKind::Chinese, "selection"),
            Target::English
        );
    }

    #[test]
    fn 混排提示词会说明保留中文部分() {
        let prompt = build_sentence_prompt(Target::Chinese, LangKind::Mixed);
        assert!(prompt.contains("into simplified Chinese"));
        assert!(prompt.contains("keep the parts that are already Chinese"));

        let prompt = build_sentence_prompt(Target::Chinese, LangKind::English);
        assert!(!prompt.contains("keep the parts that are already Chinese"));
    }
}
