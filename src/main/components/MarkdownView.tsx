import Markdown, { type Components } from "react-markdown";
import remarkBreaks from "remark-breaks";
import remarkGfm from "remark-gfm";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * 译文 / 原文的 Markdown 渲染层。
 *
 * 只负责"显示"：不修改任何请求内容，也不改翻译结果本身。
 * - remarkBreaks：把单个换行渲染成 <br>，避免多行文本被 HTML 折成一行
 * - 段落之间用空行分隔（见 utils/textLayout.ts 的 restoreParagraphs）
 * - 代码块、表格横向可滚动；链接交给系统浏览器打开
 */

const HEADING_CLASS: Record<number, string> = {
    1: "text-[17px] font-semibold mt-3 mb-1.5",
    2: "text-[15px] font-semibold mt-3 mb-1.5",
    3: "text-[13.5px] font-semibold mt-2.5 mb-1",
    4: "text-[13px] font-semibold mt-2 mb-1",
    5: "text-[12.5px] font-semibold mt-2 mb-1",
    6: "text-[12px] font-semibold mt-2 mb-1 text-tagW",
};

const OpenableScheme = /^(https?:|mailto:)/i;

const components: Components = {
    p: ({ children }) => (
        <p className="my-1.5 first:mt-0 last:mb-0 leading-relaxed whitespace-pre-wrap break-words">
            {children}
        </p>
    ),
    h1: ({ children }) => (
        <h1 className={`${HEADING_CLASS[1]} text-mainTitleW`}>{children}</h1>
    ),
    h2: ({ children }) => (
        <h2 className={`${HEADING_CLASS[2]} text-mainTitleW`}>{children}</h2>
    ),
    h3: ({ children }) => (
        <h3 className={`${HEADING_CLASS[3]} text-mainTitleW`}>{children}</h3>
    ),
    h4: ({ children }) => (
        <h4 className={`${HEADING_CLASS[4]} text-mainTitleW`}>{children}</h4>
    ),
    h5: ({ children }) => (
        <h5 className={`${HEADING_CLASS[5]} text-mainTitleW`}>{children}</h5>
    ),
    h6: ({ children }) => (
        <h6 className={`${HEADING_CLASS[6]}`}>{children}</h6>
    ),
    ul: ({ children }) => (
        <ul className="my-1.5 pl-4 list-disc marker:text-tagW space-y-1">
            {children}
        </ul>
    ),
    ol: ({ children }) => (
        <ol className="my-1.5 pl-4 list-decimal marker:text-tagW space-y-1">
            {children}
        </ol>
    ),
    li: ({ children, className }) => (
        <li
            className={`leading-relaxed break-words [&>p]:my-0 ${
                className?.includes("task-list-item")
                    ? "list-none -ml-4 flex items-start gap-1"
                    : ""
            }`}
        >
            {children}
        </li>
    ),
    blockquote: ({ children }) => (
        <blockquote className="my-2 pl-2.5 border-l-2 border-borderMainW text-tagSecondW [&>p]:my-1">
            {children}
        </blockquote>
    ),
    hr: () => <hr className="my-3 border-t border-borderSubW" />,
    strong: ({ children }) => (
        <strong className="font-semibold text-mainTitleW">{children}</strong>
    ),
    em: ({ children }) => <em className="italic">{children}</em>,
    del: ({ children }) => (
        <del className="text-tagW line-through">{children}</del>
    ),
    a: ({ href, children }) => (
        <a
            href={href}
            className="text-mainBlueW underline break-all cursor-pointer"
            onClick={async (e) => {
                e.preventDefault();
                if (!href || !OpenableScheme.test(href)) return;
                try {
                    await openUrl(href);
                } catch (err) {
                    console.error("打开链接失败", err);
                }
            }}
        >
            {children}
        </a>
    ),
    pre: ({ children }) => (
        <pre className="my-2 p-2 rounded-lg bg-gray-50 border border-borderMainW overflow-x-auto text-[11.5px] leading-relaxed">
            {children}
        </pre>
    ),
    code: ({ className, children }) => {
        const isBlock = /language-/.test(className ?? "");
        if (isBlock) {
            return (
                <code className={`font-mono whitespace-pre ${className ?? ""}`}>
                    {children}
                </code>
            );
        }
        return (
            <code className="font-mono text-[11.5px] px-1 py-px rounded bg-gray-100 border border-gray-200 text-[#c7254e] break-words">
                {children}
            </code>
        );
    },
    table: ({ children }) => (
        <div className="my-2 overflow-x-auto rounded-lg border border-borderMainW">
            <table className="w-full text-[12px] border-collapse">
                {children}
            </table>
        </div>
    ),
    th: ({ children }) => (
        <th className="px-2 py-1 text-left font-semibold bg-subBgW border-b border-borderSubW whitespace-nowrap">
            {children}
        </th>
    ),
    td: ({ children }) => (
        <td className="px-2 py-1 align-top border-b border-borderSubW break-words">
            {children}
        </td>
    ),
    img: ({ src, alt }) => (
        <img
            src={typeof src === "string" ? src : undefined}
            alt={alt ?? ""}
            className="max-w-full h-auto rounded-md my-1.5"
        />
    ),
    input: ({ checked }) => (
        <input
            type="checkbox"
            checked={!!checked}
            readOnly
            className="mt-1 shrink-0 accent-[#4a90d9]"
        />
    ),
};

function MarkdownView({
    content,
    className = "",
}: {
    content: string;
    className?: string;
}) {
    return (
        <div
            className={`break-words select-text ${className}`}
            // 渲染层不允许出现可点击跳转的外部导航
            onClickCapture={(e) => {
                const el = e.target as HTMLElement;
                if (el.tagName === "A") e.preventDefault();
            }}
        >
            <Markdown
                remarkPlugins={[remarkGfm, remarkBreaks]}
                components={components}
            >
                {content ?? ""}
            </Markdown>
        </div>
    );
}

export default MarkdownView;
