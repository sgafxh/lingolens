import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Settings = {
    base_url: string;
    model: string;
    target_lang: string;
    has_api_key: boolean;
};

export default function TranslationSection() {
    const [settings, setSettings] = useState<Settings>({ base_url: "", model: "", target_lang: "auto", has_api_key: false });
    const [apiKey, setApiKey] = useState("");
    const [showKey, setShowKey] = useState(false);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [dirty, setDirty] = useState(false);
    const [message, setMessage] = useState("");
    const [failed, setFailed] = useState(false);

    useEffect(() => {
        let active = true;
        invoke<Settings>("get_translation_settings")
            .then(value => { if (active) { setSettings(value); setLoading(false); } })
            .catch(() => { if (active) { setFailed(true); setMessage("读取设置失败，请重新打开设置窗口。"); } });
        return () => { active = false; };
    }, []);

    const update = (field: "base_url" | "model" | "target_lang", value: string) => {
        setSettings(current => ({ ...current, [field]: value }));
        setDirty(true);
        setMessage("");
    };

    const save = async (event: React.FormEvent) => {
        event.preventDefault();
        setSaving(true);
        setMessage("");
        try {
            await invoke("save_translation_settings", { settings: {
                base_url: settings.base_url,
                model: settings.model,
                target_lang: settings.target_lang,
                api_key: apiKey.trim() || null,
            } });
            setSettings(current => ({ ...current, base_url: current.base_url.trim().replace(/\/+$/, ""), model: current.model.trim(), has_api_key: true }));
            setApiKey("");
            setShowKey(false);
            setDirty(false);
            setFailed(false);
            setMessage("已保存，下一次翻译生效。");
        } catch (error) {
            setFailed(true);
            setMessage(typeof error === "string" ? error : "保存失败，请重试。");
        } finally {
            setSaving(false);
        }
    };
    const inputClass = "w-full rounded-lg border border-borderMainW bg-white px-3 py-2 text-sm text-mainTitleW outline-none focus:border-mainBlueW select-text disabled:opacity-50";

    return (
        <section aria-labelledby="translation-settings-title">
            <h2 id="translation-settings-title" className="text-sm font-medium text-mainTitleW mb-3">翻译服务</h2>
            <form onSubmit={save} className="rounded-xl border border-borderMainW bg-white p-3 space-y-3">
                <p className="text-xs text-tagSecondW">添加你使用的模型服务。API Key 保存在本机。</p>
                <fieldset disabled={loading || saving} className="space-y-3">
                    <label className="block text-xs text-mainTitleW space-y-1">
                        <span>服务地址</span>
                        <input type="url" required className={inputClass} value={settings.base_url} onChange={e => update("base_url", e.target.value)} placeholder="https://api.example.com/v1" spellCheck={false} autoCapitalize="none" />
                    </label>
                    <label className="block text-xs text-mainTitleW space-y-1">
                        <span>模型名称</span>
                        <input required className={inputClass} value={settings.model} onChange={e => update("model", e.target.value)} placeholder="填写服务商提供的模型名称" spellCheck={false} autoCapitalize="none" />
                    </label>
                    <div className="space-y-1">
                        <label htmlFor="translation-api-key" className="text-xs text-mainTitleW">API Key</label>
                        <div className="flex gap-2">
                            <input id="translation-api-key" type={showKey ? "text" : "password"} required={!settings.has_api_key} className={inputClass} value={apiKey} onChange={e => { setApiKey(e.target.value); setDirty(true); setMessage(""); }} placeholder={settings.has_api_key ? "已保存，留空保留现有 Key" : "填写 API Key"} autoComplete="off" spellCheck={false} autoCapitalize="none" />
                            <button type="button" className="shrink-0 text-xs text-mainBlueW px-2" aria-pressed={showKey} onClick={() => setShowKey(value => !value)}>{showKey ? "隐藏" : "显示"}</button>
                        </div>
                    </div>
                    <label className="block text-xs text-mainTitleW space-y-1">
                        <span>目标语言</span>
                        <select className={inputClass} value={settings.target_lang} onChange={e => update("target_lang", e.target.value)}>
                            <option value="auto">自动判断</option>
                            <option value="zh">中文</option>
                            <option value="en">英文</option>
                        </select>
                    </label>
                    <button type="submit" disabled={!dirty || loading || saving} className="w-full rounded-lg bg-mainBlueW py-2 text-sm text-white disabled:opacity-40 cursor-pointer">{saving ? "保存中…" : "保存翻译服务"}</button>
                </fieldset>
                {message && <p role={failed ? "alert" : "status"} className={failed ? "text-xs text-red-600" : "text-xs text-green-700"}>{message}</p>}
            </form>
        </section>
    );
}
