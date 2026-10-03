/** 一条快捷键配置 */
export interface ShortcutItem {
    id: string;
    label: string;
    keys: string[];
    defaultKeys: string[];
    save: {
        id: string;
        index: number;
    }[];
}

/** 通用设置项 */
export interface GeneralSetting {
    id: string;
    label: string;
    description: string;
    value: boolean | string | number;
}

/** 真正的配置项，定义在Rust */
export interface RustAppConfig {
    theme: string;
    auto_start: boolean;
    auto_play: boolean;
    pronunciation: string;
    pronunciation_volume: number;
    point_key: string;
    pinned_key: string;
    hide_win_key: string;
    select_text_modifiers: string;
    select_text_code: string;
    search_win_modifiers: string;
    search_win_code: string;
    /** 主窗口尺寸（逻辑像素），由主窗口右下角拖拽手柄维护 */
    win_width: number;
    win_height: number;
    /** 翻译后端（法衡-H1，OpenAI 兼容接口）；由本机配置维护，设置页不编辑 */
    llm_base_url: string;
    llm_model: string;
    llm_api_key: string;
    first_setup_completed: boolean;
}
