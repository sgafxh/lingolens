import {
    currentMonitor,
    getCurrentWindow,
    LogicalSize,
} from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";

import useUiStore from "../store/useUiStore";

/**
 * 右下角窗口缩放手柄。
 *
 * 主窗口是 `decorations: false` 的无边框置顶窗口，没有原生拖拽边，
 * 所以自己画一个手柄，拖拽时调用 `window.setSize`。
 *
 * 三个容易踩的坑，这里都处理了：
 * 1. 窗口在鼠标移出时会自动隐藏（见 utils/useCustom.ts），且带 300ms 延迟。
 *    拖拽过程中指针很容易压到窗口边缘外，必须先锁住 hover 状态，松手后再交还给正常逻辑。
 * 2. **手柄放在窗口真正的右下角**（尖尖那个像素点）。之前往里收 10px、往上提 48px
 *    去避开页脚按钮，反而不符合"角上抓手"的习惯（而且斜着拖时手感对不上）。
 *    现在贴角，靠 ① 移出隐藏的容错范围 ② 进入手柄时的抑制，不会在仲手时误隐藏。
 * 3. 尺寸需要被记住：松手时调一次 `save_window_size`，后端还会监听窗口 Resized
 *    事件防抖落盘（前端的 pointerup 可能因为指针压出窗口而收不到）。
 *
 * 拖拽行为：默认**等比缩放**（整体放大缩小），按住 Shift 变成宽高各自跟随的
 * 自由缩放。
 */

/** 最小尺寸（逻辑像素），与 tauri.conf.json 的 minWidth / minHeight 保持一致 */
export const MIN_WIN_WIDTH = 260;
export const MIN_WIN_HEIGHT = 200;
/** 最大尺寸不超过当前显示器的比例，避免拖到屏幕外 */
const MAX_RATIO = 0.9;

interface SizeLimits {
    minWidth: number;
    minHeight: number;
    maxWidth: number;
    maxHeight: number;
}

async function readLimits(): Promise<SizeLimits> {
    let maxWidth = Number.POSITIVE_INFINITY;
    let maxHeight = Number.POSITIVE_INFINITY;

    try {
        const monitor = await currentMonitor();
        if (monitor) {
            const scale = monitor.scaleFactor || 1;
            // 用工作区而不是全屏尺寸，避免拖到任务栏 / Dock 底下
            const area = monitor.workArea?.size ?? monitor.size;
            maxWidth = (area.width / scale) * MAX_RATIO;
            maxHeight = (area.height / scale) * MAX_RATIO;
        }
    } catch (err) {
        console.warn("读取显示器尺寸失败，跳过上限约束", err);
    }

    return {
        minWidth: MIN_WIN_WIDTH,
        minHeight: MIN_WIN_HEIGHT,
        maxWidth: Math.max(MIN_WIN_WIDTH, maxWidth),
        maxHeight: Math.max(MIN_WIN_HEIGHT, maxHeight),
    };
}

const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(value, min), max);

function ResizeHandle() {
    const resizing = useUiStore((state) => state.isResizing);
    const setResizing = useUiStore((state) => state.setIsResizing);
    const frameRef = useRef<number | null>(null);
    const sizeRef = useRef({ width: 0, height: 0 });

    useEffect(() => {
        // 让系统层也兜住最小值，避免 JS 层面失效时窗口被拖成一条缝
        getCurrentWindow()
            .setSizeConstraints({
                minWidth: MIN_WIN_WIDTH,
                minHeight: MIN_WIN_HEIGHT,
            })
            .catch((err) => console.warn("设置窗口尺寸约束失败", err));

        return () => {
            if (frameRef.current !== null)
                cancelAnimationFrame(frameRef.current);
        };
    }, []);

    const handlePointerDown = async (
        e: React.PointerEvent<HTMLDivElement>,
    ) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();

        const target = e.currentTarget;
        const appWindow = getCurrentWindow();

        try {
            target.setPointerCapture(e.pointerId);
        } catch {
            /* 某些环境下不支持指针捕获，退化为普通拖拽 */
        }

        let startWidth: number;
        let startHeight: number;
        try {
            const scale = await appWindow.scaleFactor();
            const size = await appWindow.innerSize();
            startWidth = size.width / scale;
            startHeight = size.height / scale;
        } catch (err) {
            console.error("读取窗口尺寸失败", err);
            return;
        }

        const limits = await readLimits();
        sizeRef.current = { width: startWidth, height: startHeight };

        const startX = e.screenX;
        const startY = e.screenY;

        // 拖拽期间锁住 window_locked，避免鼠标压出窗口触发自动隐藏；
        // 松手后的短暂时间内也继续抑制，因为刚改过尺寸（双击范围内浏览器可能误报 mouseleave）
        useUiStore.getState().suppressHide(1200);
        invoke("log_debug", {
            tag: "resize",
            msg: "pointerdown 命中缩放手柄",
        }).catch(() => {});
        invoke("update_hover_status", { hovered: true }).catch(() => {});
        setResizing(true);
        document.body.style.cursor = "nwse-resize";

        const applySize = (width: number, height: number) => {
            // 先记录最新目标值，再决定要不要合并到下一帧。
            //
            // 原来是先判断 `frameRef.current !== null` 就直接 return，导致合帧期间的
            // 移动量被整个丢掉 —— 最终尺寸偏小，落盘的还是旧值（"改了尺寸但没记住"）。
            sizeRef.current = { width, height };

            if (frameRef.current !== null) return;

            frameRef.current = requestAnimationFrame(() => {
                frameRef.current = null;
                const { width: w, height: h } = sizeRef.current;
                appWindow
                    .setSize(new LogicalSize(Math.round(w), Math.round(h)))
                    .catch((err) => console.error("调整窗口尺寸失败", err));
            });
        };

        const onPointerMove = (ev: PointerEvent) => {
            const dx = ev.screenX - startX;
            const dy = ev.screenY - startY;

            // 按住 Shift：宽高各自跟随
            if (ev.shiftKey) {
                applySize(
                    clamp(startWidth + dx, limits.minWidth, limits.maxWidth),
                    clamp(startHeight + dy, limits.minHeight, limits.maxHeight),
                );
                return;
            }

            // 默认：等比缩放。
            //
            // 用"变化更明显"的那根轴决定倍率，另一轴等比跟随。原来是两轴各自独立，
            // 横向拖动时高度一动不动，用起来就像"只能放大宽"。
            const scaleByWidth = (startWidth + dx) / startWidth;
            const scaleByHeight = (startHeight + dy) / startHeight;
            const scale =
                Math.abs(scaleByWidth - 1) >= Math.abs(scaleByHeight - 1)
                    ? scaleByWidth
                    : scaleByHeight;

            // 上下限按同一个倍率算，否则夹取会破坏等比
            const minScale = Math.max(
                limits.minWidth / startWidth,
                limits.minHeight / startHeight,
            );
            const maxScale = Math.min(
                limits.maxWidth / startWidth,
                limits.maxHeight / startHeight,
            );
            const finalScale = clamp(
                scale,
                minScale,
                Math.max(minScale, maxScale),
            );

            applySize(startWidth * finalScale, startHeight * finalScale);
        };

        const onPointerUp = (ev: PointerEvent) => {
            window.removeEventListener("pointermove", onPointerMove);
            window.removeEventListener("pointerup", onPointerUp);
            window.removeEventListener("pointercancel", onPointerUp);

            document.body.style.cursor = "";
            setResizing(false);
            try {
                target.releasePointerCapture(ev.pointerId);
            } catch {
                /* ignore */
            }

            const pointerInside =
                ev.clientX >= 0 &&
                ev.clientY >= 0 &&
                ev.clientX <= window.innerWidth &&
                ev.clientY <= window.innerHeight;

            // 指针还在窗口内 → 继续锁住；已经拖到窗口外 → 交还给移出隐藏逻辑
            if (pointerInside) useUiStore.getState().suppressHide(400);
            invoke("update_hover_status", { hovered: pointerInside }).catch(
                () => {},
            );

            appWindow
                .setSize(
                    new LogicalSize(
                        Math.round(sizeRef.current.width),
                        Math.round(sizeRef.current.height),
                    ),
                )
                .catch(() => {});

            // 后端自己读窗口真实尺寸，这里不必传值（前端合帧时手里的值可能偏旧）
            invoke("save_window_size").catch((err) =>
                console.error("保存窗口尺寸失败", err),
            );
        };

        window.addEventListener("pointermove", onPointerMove);
        window.addEventListener("pointerup", onPointerUp);
        window.addEventListener("pointercancel", onPointerUp);
    };

    return (
        <div
            onPointerDown={handlePointerDown}
            // 鼠标已经在手柄上（即已在窗口内），顺手抑制一小段隐藏，避免边缘抖动误伤
            onPointerEnter={() => {
                useUiStore.getState().suppressHide(500);
                invoke("log_debug", {
                    tag: "resize",
                    msg: "指针进入缩放手柄",
                }).catch(() => {});
            }}
            title="拖拽缩放窗口（等比；按住 Shift 自由调宽高）"
            // 贴在窗口真正的右下角：抓手就在你最自然去够的那个角上。
            // 命中区 24px，比图标大一圈，不用恰好命中图标也能拖。
            className={`fixed bottom-0 right-0 z-50 w-6 h-6 flex items-end justify-end rounded-sm
                        cursor-nwse-resize touch-none transition-opacity
                        hover:bg-subBgW ${resizing ? "opacity-100" : "opacity-60 hover:opacity-100"}`}
        >
            <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
                <g stroke="#bbbbbb" strokeWidth="1.4" strokeLinecap="round">
                    <line x1="11" y1="4" x2="4" y2="11" />
                    <line x1="11" y1="8" x2="8" y2="11" />
                </g>
                <circle cx="11.5" cy="11.5" r="0.9" fill="#bbbbbb" />
            </svg>
        </div>
    );
}

export default ResizeHandle;
