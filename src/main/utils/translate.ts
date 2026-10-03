import { invoke } from "@tauri-apps/api/core";
import useTransStore from "../store/useTransStore";
import { TransResultTypes } from "../types/transResult";
import { judgeSentence } from "./tool";

/**
 * 统一的翻译入口：划词、取词、搜索框、结果页跳词都走这里。
 *
 * - 会先清空上一条结果（避免旧内容残留，实现"直接覆盖"）
 * - 单词结果写入 store.result，句子结果写入 store.sentenceResult 由 App 负责跳转
 * - 同一时刻只保留最新一次请求的结果，早到的旧响应会被丢弃
 */
let requestSeq = 0;

export async function runTranslate(word: string): Promise<void> {
    const text = word?.trim();
    if (!text) return;

    const seq = ++requestSeq;
    const store = useTransStore.getState();
    store.beginRequest();

    try {
        const res = await invoke<TransResultTypes>("fetch_trans_res", {
            word: text,
        });

        // 已经有更新的请求了，丢弃这条迟到的响应
        if (seq !== requestSeq) return;

        if (res?.status === 200 && res.data?.translate) {
            if (judgeSentence(res.data.translate.text ?? text)) {
                useTransStore.getState().setSentenceResult(res);
            } else {
                useTransStore.getState().setResult(res);
            }
        } else {
            useTransStore.getState().setError(res?.msg || "翻译失败");
        }
    } catch (err) {
        if (seq !== requestSeq) return;
        useTransStore.getState().setError(String(err));
    }
}
