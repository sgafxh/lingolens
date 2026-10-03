import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import useUiStore from "../store/useUiStore";

/**
 * 鼠标移出窗口后延迟多少毫秒才真的隐藏。
 *
 * 不能立即隐藏：鼠标只是擦出窗口一点点又马上回来（例如仲手去够右下角的缩放手柄），
 * 立即隐藏会让窗口"自己没了"。延迟期间只要 mouseenter 就取消。
 */
const HIDE_DELAY_MS = 300;

/**
 * 自定义 Hook：将最新值同步到 Ref 中，避免 useEffect 依赖频繁触发
 */
function useLatest<T>(value: T) {
    const ref = useRef(value);
    ref.current = value;
    return ref;
}

export function useWindowListener(
    isPinned: boolean,
    setIsPinned: (isPinned: boolean) => void,
    hideKey: string,
    pinKey: string,
) {
    // 将所有频繁改变的属性收纳到一个 ref 对象中
    const stateRef = useLatest({ isPinned, hideKey, pinKey });

    useEffect(() => {
        const appWindow = getCurrentWindow();
        let hideTimer: ReturnType<typeof setTimeout> | null = null;

        const clearHideTimer = () => {
            if (hideTimer !== null) {
                clearTimeout(hideTimer);
                hideTimer = null;
            }
        };

        /** 当前是否还存在不能隐藏的理由 */
        const hideBlocked = () => {
            const { isResizing, hideSuppressUntil } = useUiStore.getState();
            return (
                isResizing ||
                Date.now() < hideSuppressUntil ||
                stateRef.current.isPinned
            );
        };

        const handleMouseEnter = () => {
            clearHideTimer();
            invoke("update_hover_status", { hovered: true });
        };

        const hideWindow = (reason: string) => {
            invoke("update_hover_status", { hovered: false });
            invoke("log_debug", {
                tag: "fe_hide",
                msg: `${reason} → appWindow.hide()`,
            }).catch(() => {});
            appWindow.hide();
        };

        const handleMouseLeave = () => {
            if (hideBlocked()) return;

            clearHideTimer();
            hideTimer = setTimeout(async () => {
                hideTimer = null;
                if (hideBlocked()) return;

                // 指针只是擦出窗口（伸手去够右下角缩放手柄时几乎必然发生）：
                // 不算"离开"，重新排一次检查，等它真的走远了再隐藏。
                const near = await invoke<boolean>("is_cursor_near_window", {
                    tolerance: 30,
                }).catch(() => true);

                if (near) {
                    handleMouseLeave();
                    return;
                }

                hideWindow("mouseleave 且指针已远离窗口");
            }, HIDE_DELAY_MS);
        };

        const handleKeydown = (e: KeyboardEvent) => {
            const { hideKey, pinKey, isPinned } = stateRef.current;

            if (e.key === hideKey) {
                hideWindow(`${hideKey} 键`);
            }
            if (e.key === pinKey) {
                setIsPinned(!isPinned);
            }
        };

        const isMac =
            navigator.platform?.toLowerCase().includes("mac") ?? false;
        const docElment = isMac ? document.body : document;

        // 绑定事件
        docElment.addEventListener("mouseenter", handleMouseEnter);
        docElment.addEventListener("mouseleave", handleMouseLeave);
        document.addEventListener("keydown", handleKeydown);

        // 清理事件
        return () => {
            clearHideTimer();
            docElment.removeEventListener("mouseenter", handleMouseEnter);
            docElment.removeEventListener("mouseleave", handleMouseLeave);
            document.removeEventListener("keydown", handleKeydown);
        };
    }, []);
}

export function useOnWindowChange(onHide: () => void) {
    const savedOnHide = useRef(onHide);

    useEffect(() => {
        savedOnHide.current = onHide;
    }, [onHide]);

    useEffect(() => {
        async function setupListener() {
            const appWindow = getCurrentWindow();

            // 监听窗口隐藏/失焦事件
            const unlisten = await appWindow.listen("tauri://blur", () => {
                // 判断窗口是否为隐藏状态
                appWindow.isVisible().then((isShow) => {
                    if (!isShow) {
                        savedOnHide.current();
                    }
                });
            });
            return unlisten;
        }

        const listenerPromise = setupListener();
        return () => {
            listenerPromise.then((unlisten) => unlisten());
        };
    }, []);
}
