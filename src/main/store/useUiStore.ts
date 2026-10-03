import { create } from "zustand";

interface UiState {
    isPinned: boolean;
    updateUrl: string;
    /** 正在拖拽右下角缩放手柄：期间禁止"鼠标移出即隐藏" */
    isResizing: boolean;
    /**
     * 在此之前的时间戳（Date.now() 毫秒）内忽略"鼠标移出即隐藏"。
     * 用于刚调整过窗口尺寸的场景：尺寸变化会让指针位置相对窗口变了，
     * 浏览器可能误报 mouseleave，此时不该把窗口关掉。
     */
    hideSuppressUntil: number;
    setIsPinned: (isPinned: boolean) => void;
    setUpdateUrl: (updateUrl: string) => void;
    setIsResizing: (isResizing: boolean) => void;
    /** 抑制移出隐藏 ms 毫秒（多次调用取最晚的那次） */
    suppressHide: (ms: number) => void;
}

const useUiStore = create<UiState>()((set, get) => ({
    isPinned: false,
    updateUrl: "",
    isResizing: false,
    hideSuppressUntil: 0,
    setIsPinned: (isPinned) => set({ isPinned }),
    setUpdateUrl: (updateUrl) => set({ updateUrl }),
    setIsResizing: (isResizing) => set({ isResizing }),
    suppressHide: (ms) => {
        const until = Date.now() + ms;
        if (until > get().hideSuppressUntil) set({ hideSuppressUntil: until });
    },
}));
export default useUiStore;
