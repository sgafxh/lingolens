import "../assets/css/App.css";
import { useWindowListener } from "./utils/useCustom";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { Routes, useNavigate, Route, useLocation } from "react-router-dom";
import useUiStore from "./store/useUiStore";
import useTransStore from "./store/useTransStore";
import SearchPage from "./pages/SearchPage";
import HomePage from "./pages/HomePage";
import ResizeHandle from "./components/ResizeHandle";
import useConfigStore from "./store/useConfigStore";
import { invoke } from "@tauri-apps/api/core";
import { RustAppConfig } from "../setting/types";
import { useShallow } from "zustand/react/shallow";
import { usePlatform } from "../hooks/usePlatform";
import { runTranslate } from "./utils/translate";

function App() {
    const navigate = useNavigate();
    const location = useLocation();
    const { isMac } = usePlatform();

    const isPinned = useUiStore((state) => state.isPinned);
    const setIsPinned = useUiStore((state) => state.setIsPinned);
    const setUpdateUrl = useUiStore((state) => state.setUpdateUrl);

    const sentenceResult = useTransStore((state) => state.sentenceResult);
    const wordResult = useTransStore((state) => state.result);

    const { isAutoPlay, pronunciation, pinnedKey, hideWinKey } = useConfigStore(
        useShallow((state) => ({
            isAutoPlay: state.auto_play,
            pronunciation: state.pronunciation,
            pinnedKey: state.pinned_key,
            hideWinKey: state.hide_win_key,
        })),
    );

    const updateConfig = useConfigStore((state) => state.update);

    // 监听鼠标移入/出窗口
    useWindowListener(isPinned, setIsPinned, hideWinKey, pinnedKey);

    // 取词/划词结果：句子跳搜索页，单词留在主页
    useEffect(() => {
        if (!sentenceResult) return;
        navigate("/search", { state: { data: sentenceResult } });
    }, [sentenceResult]);

    // 人在搜索页时按了取词键：回到主页把新单词显示出来（覆盖当前内容）
    useEffect(() => {
        if (!wordResult) return;
        if (location.pathname !== "/") navigate("/");
    }, [wordResult]);

    useEffect(() => {
        const setupListener = async () => {
            const unlisten = await listen<{ type: string; data: any }>(
                "win-event",
                (event) => {
                    if (event.payload.type === "search") {
                        navigate("/search");
                    } else if (event.payload.type === "config") {
                        updateConfig(event.payload.data);
                    }
                },
            );

            // 返回清理函数
            return unlisten;
        };

        const listenerPromise = setupListener();

        /**
         * 取词 / 划词事件必须注册在常驻的 App 上。
         * 之前它注册在 HomePage，而句子翻译会 navigate("/search") 把 HomePage 卸载，
         * 监听器随之失效 —— 不关掉窗口就再也翻译不了新内容。
         */
        const cursorPromise = listen<string>("from-cursor", (event) => {
            runTranslate(event.payload);
        });

        // 初始化配置
        invoke<RustAppConfig>("get_config").then((config) => {
            updateConfig(config);
        });

        // 检测更新
        // 本构建的 update.rs 恒返回"无更新"（见该文件注释），所以这里会走到 catch，
        // 属正常情况；加 catch 只为避免未处理的 Promise rejection 报错。
        invoke<any>("trigger_update_check", { version: __APP_VERSION__ })
            .then((data) => {
                setUpdateUrl(data.url);
            })
            .catch(() => {});

        return () => {
            listenerPromise.then((unlisten) => unlisten());
            cursorPromise.then((unlisten) => unlisten());
        };
    }, []);

    return (
        <>
            <Routes>
                <Route
                    path="/"
                    element={
                        <HomePage
                            isAutoPlay={isAutoPlay}
                            isUs={pronunciation === "us"}
                        />
                    }
                />
                <Route path="/search" element={<SearchPage isMac={isMac} />} />
            </Routes>
            <ResizeHandle />
        </>
    );
}

export default App;
