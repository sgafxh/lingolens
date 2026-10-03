/**
 * 译文分段还原。
 *
 * 背景：划词选 2-3 段文字时，OCR/剪贴板拿到的原文是带换行的，但翻译接口返回的
 * `dit` 有可能把换行吃掉了、只剩一整串，于是窗口里呈现成"一大段"。
 *
 * 策略：
 * 1. 译文本身带换行 → 原样返回（渲染层用 whitespace-pre-wrap / remark-breaks 保留）
 * 2. 原文只有一段 → 不动
 * 3. 原文多段但译文无换行 → 按原文各段长度比例，把译文按句号边界切成对应段数
 *
 * 第 3 条是启发式对齐，只影响显示，不改动请求内容，也不写回任何数据。
 */

/** 拆段：连续空行优先分段，单个换行也视为分段（划词复制常见形态） */
export function splitParagraphs(text: string): string[] {
    if (!text) return [];
    return text
        .split(/\r?\n\s*\r?\n|\r?\n/)
        .map((p) => p.trim())
        .filter(Boolean);
}

const SENTENCE_END = new Set([
    "。",
    "！",
    "？",
    "；",
    "…",
    ".",
    "!",
    "?",
    ";",
    "\n",
]);

/**
 * 按句末标点切句（手写扫描，避免正则 lookbehind 在旧版 WKWebView 上不支持）。
 * 句末的引号/括号会跟在前一句尾部，不单独成句。
 */
export function splitSentences(text: string): string[] {
    const out: string[] = [];
    let buf = "";

    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        buf += ch;

        if (!SENTENCE_END.has(ch)) continue;

        // 连续标点（如 "?!" "…" ）一起收尾
        while (i + 1 < text.length && SENTENCE_END.has(text[i + 1])) {
            buf += text[i + 1];
            i++;
        }
        // 收尾标点后紧跟的引号、括号归属本句
        while (i + 1 < text.length && /["'”’）)\]】」』]/.test(text[i + 1])) {
            buf += text[i + 1];
            i++;
        }

        const s = buf.trim();
        if (s) out.push(s);
        buf = "";
    }

    const rest = buf.trim();
    if (rest) out.push(rest);
    return out;
}

/** 原文是否像 Markdown / 结构化文本：这类内容交给 Markdown 渲染器，不做重排 */
function looksStructured(text: string): boolean {
    return (
        /```/.test(text) ||
        /^\s{0,3}#{1,6}\s/m.test(text) ||
        /^\s{0,3}[-*+]\s/m.test(text) ||
        /^\s{0,3}\d+[.)]\s/m.test(text) ||
        /^\s{0,3}>\s/m.test(text) ||
        /^\s{0,3}\|/m.test(text)
    );
}

/**
 * 把译文按原文段落结构重新分段。
 * @param translation 译文（dit）
 * @param source      原文（translate.text）
 */
export function restoreParagraphs(
    translation: string,
    source: string,
): string {
    const dit = translation ?? "";
    const src = source ?? "";
    if (!dit || !src) return dit;

    // 译文自带换行，已经是分好的段
    if (/\r?\n/.test(dit)) return dit;

    // 结构化文本不做启发式切分
    if (looksStructured(src) || looksStructured(dit)) return dit;

    const srcParagraphs = splitParagraphs(src);
    if (srcParagraphs.length < 2) return dit;

    const sentences = splitSentences(dit);
    // 句数少于段数，切不出对应的段数，保持原样
    if (sentences.length < srcParagraphs.length) return dit;

    const weights = srcParagraphs.map((p) => Math.max(p.length, 1));
    const totalWeight = weights.reduce((a, b) => a + b, 0);

    // 按权重分配句数，每段至少 1 句
    const counts = weights.map((w) => (sentences.length * w) / totalWeight);
    const assigned = counts.map((c) => Math.max(1, Math.round(c)));

    // 修正总和（多了从尾部减，少了补到最长的一段）
    let diff = assigned.reduce((a, b) => a + b, 0) - sentences.length;
    let guard = 0;
    while (diff !== 0 && guard++ < 1000) {
        if (diff > 0) {
            // 找当前句数 > 1 且最长的一段减一
            let idx = -1;
            for (let i = 0; i < assigned.length; i++) {
                if (assigned[i] > 1 && (idx === -1 || assigned[i] > assigned[idx]))
                    idx = i;
            }
            if (idx === -1) break;
            assigned[idx]--;
            diff--;
        } else {
            let idx = 0;
            for (let i = 1; i < assigned.length; i++) {
                if (weights[i] > weights[idx]) idx = i;
            }
            assigned[idx]++;
            diff++;
        }
    }

    const paragraphs: string[] = [];
    let cursor = 0;
    for (let i = 0; i < assigned.length; i++) {
        const chunk = sentences.slice(cursor, cursor + assigned[i]);
        cursor += assigned[i];
        if (chunk.length) paragraphs.push(chunk.join(""));
    }
    // 余下的句子并入最后一段，避免丢内容
    if (cursor < sentences.length) {
        paragraphs[paragraphs.length - 1] =
            (paragraphs[paragraphs.length - 1] ?? "") +
            sentences.slice(cursor).join("");
    }

    return paragraphs.length > 1 ? paragraphs.join("\n\n") : dit;
}
