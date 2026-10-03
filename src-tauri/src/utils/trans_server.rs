//! 翻译后端：法衡-H1（OpenAI 兼容 /chat/completions）
//!
//! 说明：上游仓库里这个文件是私有的（`.gitignore` 标记为 Private files，走有道的
//! 非官方接口）。本构建改用法衡-H1，理由与取舍见下：
//!
//! * 速度：实测单词 0.14s、三段段落 0.25s（与有道 0.13~0.15s 基本持平）。
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
use tauri::{AppHandle, State};

/// 单次翻译的超时时间
const REQUEST_TIMEOUT_SECS: u64 = 20;
/// 判定为"查词"而非"句子"的最大字符数
const WORD_MAX_CHARS: usize = 30;

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
    /// 单词卡片（释义、词形变化、同近义词等）
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

/// 查词提示词：只要词性和简明释义，不要整句话
const SYSTEM_WORD: &str = "\
You are a compact bilingual dictionary. For the given word, output ONLY its part(s) of speech \
and concise Chinese meanings (or English meanings if the word is Chinese).
Format rules:
- Line 1: part of speech, e.g. `n.` or `adj.; n.`
- Following lines: one short meaning per line, each under 14 characters, most common first, at most 4 lines
- Do not repeat the word, no pronunciation, no examples, no explanations, no bullet symbols
- Output plain text only, never wrap the answer in code fences";

/// 句子/段落提示词：只输出译文，并且必须保留原文的分段
const SYSTEM_SENTENCE: &str = "\
You are a professional translator. Translate the user's text:
- If the text contains Chinese, translate it into natural English; otherwise translate it into \
simplified Chinese.
Output rules (must follow strictly):
- Output ONLY the translation, nothing else
- Keep the original paragraph structure: every blank line and every line break in the source must \
appear in the same place in your output
- No explanations, no notes, no quotes around the whole answer, no code fences
- Preserve inline Markdown syntax (**bold**, `code`, lists, headings) instead of describing it";

/// 判断是查词还是句子：单个英文词（不含空白、长度够短）走词典提示词
fn is_single_word(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.chars().count() > WORD_MAX_CHARS {
        return false;
    }
    if t.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    // 至少包含一个字母或汉字，排除纯符号
    t.chars().any(|c| c.is_alphabetic())
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

/// 调用法衡-H1 完成一次翻译
async fn translate_with_faheng(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    api_key: &str,
    text: &str,
    is_word: bool,
) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("未配置法衡 API Key：请在配置文件里填入 llm_api_key".to_string());
    }

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let payload = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": if is_word { SYSTEM_WORD } else { SYSTEM_SENTENCE } },
            { "role": "user", "content": text }
        ],
        "temperature": 0,
        // 关键：不开思维链。实测开与不开是 1.0s 与 0.14s 的差别。
        "reasoning_effort": "none",
        "stream": false
    });

    let resp = client
        .post(&url)
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

/// 供 `fetch_trans_res` 命令调用
pub async fn fetch_translation(
    word: &str,
    state: &State<'_, AppState>,
    _app: &AppHandle,
) -> Result<DictResponse, String> {
    let text = word.trim();
    if text.is_empty() {
        return Err("没有取到要翻译的内容".to_string());
    }

    // 配置在 Mutex 里，先克隆出来再 await，避免把锁带过 await 点
    let (base_url, model, api_key, client) = {
        let config = state
            .config
            .lock()
            .map_err(|_| "读取配置失败（配置锁已损坏）".to_string())?;
        (
            config.llm_base_url.clone(),
            config.llm_model.clone(),
            config.llm_api_key.clone(),
            state.client.clone(),
        )
    };

    let is_word = is_single_word(text);
    let translation = translate_with_faheng(
        &client,
        &base_url,
        &model,
        &api_key,
        text,
        is_word,
    )
    .await?;

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
        "to": "auto",
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
