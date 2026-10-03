import { create } from "zustand";
import { TransResultTypes } from "../types/transResult";

/**
 * 翻译结果状态。
 *
 * 之所以从 HomePage 的局部 state 上提到全局 store：
 * 划词/取词的 `from-cursor` 事件原先由 HomePage 监听，而句子翻译会
 * `navigate("/search")` 把 HomePage 卸载，监听器随之销毁，导致窗口不关闭时
 * 再也收不到新的取词事件（表现为"必须先关掉窗口才能翻译下一段"）。
 * 现在监听统一放在常驻的 App 里，结果通过本 store 分发。
 */
interface TransState {
    /** 单词模式的翻译结果；null 表示加载中或空闲 */
    result: TransResultTypes | null;
    /** 句子/段落模式的翻译结果，App 收到后跳转 /search */
    sentenceResult: TransResultTypes | null;
    /** 失败信息 */
    error: string | null;
    /** 发起新请求前调用：清空旧结果，让界面立刻回到 Loading（直接覆盖而不是留着上一条） */
    beginRequest: () => void;
    setResult: (result: TransResultTypes) => void;
    setSentenceResult: (result: TransResultTypes) => void;
    setError: (error: string) => void;
    reset: () => void;
}

const useTransStore = create<TransState>()((set) => ({
    result: null,
    sentenceResult: null,
    error: null,
    beginRequest: () =>
        set({ result: null, sentenceResult: null, error: null }),
    setResult: (result) => set({ result, error: null, sentenceResult: null }),
    setSentenceResult: (sentenceResult) =>
        set({ sentenceResult, result: null, error: null }),
    setError: (error) =>
        set({ error, result: null, sentenceResult: null }),
    reset: () => set({ result: null, sentenceResult: null, error: null }),
}));

export default useTransStore;
