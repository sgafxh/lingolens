import { useOnWindowChange } from "../utils/useCustom";
import { useEffect } from "react";
import { useLocation } from "react-router-dom";
import Header from "../components/Header";
import Content from "../components/Content";
import Footer from "../components/Footer";
import ApiError from "../components/ApiError";
import CustomToast from "../components/CustomToast";
import useTransStore from "../store/useTransStore";
import { runTranslate } from "../utils/translate";
import "../../assets/css/main/index.css";

function HomePage({
    isAutoPlay,
    isUs,
}: {
    isAutoPlay: boolean;
    isUs: boolean;
}) {
    const location = useLocation();

    // 结果来自全局 store（监听器在 App 里，HomePage 卸载后依然能收词）
    const result = useTransStore((state) => state.result);
    const error = useTransStore((state) => state.error);
    const reset = useTransStore((state) => state.reset);

    // 窗口隐藏时清空，下次唤起不显示旧内容
    useOnWindowChange(() => {
        reset();
    });

    useEffect(() => {
        // 从搜索结果页点某个词跳回来的情况
        if (location.state?.word) {
            runTranslate(location.state.word);
        }
    }, [location.state?.word]);

    return (
        <div className="pt-3 flex flex-col gap-3 h-screen overflow-hidden">
            <CustomToast />
            <Header />

            {error === null ? (
                <Content
                    transResult={result}
                    isAutoPlay={isAutoPlay}
                    isUs={isUs}
                />
            ) : (
                <ApiError message={error} />
            )}

            <Footer path={"home"} />
        </div>
    );
}

export default HomePage;
