"""从 jieba 词典生成 Pointra 用的中文分词词表。

为什么需要这个：Windows OCR 对中文是按"单字"给 box 的，没有"词"这一级信息；
而"悬浮取词"必须知道词边界（否则会把"设计"和"改代码"拦腰拼起来）。

做法：取 jieba 的 dict.txt（MIT 协议，词+词频+词性），按词频降序裁剪后，
把词频压成 1 个字符的对数等级（64 级，每级约 1.28 倍）存进 assets/cn_dict.txt。
运行侧用它跑"词频加权最短路径"分词（等价于 jieba 的 route 算法）。

用法：
    python scripts/build_cn_dict.py <jieba dict.txt> [输出路径] [最小词频]
"""

import sys
import os
import io

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

# 频率等级用的字符表（64 个，不含空白，方便按行解析）
# 用 2 个字符表示一个等级 → 4096 级，量化误差 ~0.4%，避免 64 级（±28% 误差）
# 把"漏报"和"漏+报"这种接近的 DP 判断翻转掉。
LEVEL_CHARS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+-"
LEVELS = len(LEVEL_CHARS)
SLOTS = LEVELS * LEVELS            # 4096
DECADES = 7.0                      # 频率范围 1 ~ 1e7


def is_han(c: str) -> bool:
    cp = ord(c)
    return (
        (0x4E00 <= cp <= 0x9FFF)
        or (0x3400 <= cp <= 0x4DBF)
        or (0x20000 <= cp <= 0x2A6DF)
    )


def freq_level(freq: int) -> str:
    """词频 → 2 字符等级（对数刻度，4096 级）"""
    import math

    if freq < 1:
        freq = 1
    lv = int(round(math.log10(freq) / DECADES * (SLOTS - 1)))
    lv = max(0, min(SLOTS - 1, lv))
    return LEVEL_CHARS[lv >> 6] + LEVEL_CHARS[lv & (LEVELS - 1)]


def main() -> None:
    src = sys.argv[1] if len(sys.argv) > 1 else "jieba_dict.txt"
    dst = sys.argv[2] if len(sys.argv) > 2 else "src-tauri/assets/cn_dict.txt"
    # 词频下限：实测 >= 5 才能覆盖"漏报"(10)、"瞒报"(23) 这类低频复合词；
    # 再往下（>= 3）要多花 2.5MB 换 21 万个几乎用不到的词，不划算。
    min_freq = int(sys.argv[3]) if len(sys.argv) > 3 else 5

    rows = []
    total = 0
    with open(src, encoding="utf-8") as f:
        for line in f:
            parts = line.split()
            if len(parts) < 2:
                continue
            try:
                freq = int(parts[1])
            except ValueError:
                continue
            word = parts[0]
            if not all(is_han(c) for c in word):
                continue          # 拉丁/数字词由分词器当成连续段整体处理，不进词表
            if freq < min_freq:
                continue
            if len(word) > 10:
                continue          # 超长条目基本是专名，裁掉以免覆盖过长
            rows.append((word, freq))
            total += freq

    seen = set()
    singles, multi = [], []
    for word, freq in sorted(rows, key=lambda x: -x[1]):
        if word in seen:
            continue
        seen.add(word)
        (singles if len(word) == 1 else multi).append((word, freq))

    kept = singles + multi
    kept.sort(key=lambda x: -x[1])

    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "w", encoding="utf-8", newline="\n") as out:
        for word, freq in kept:
            out.write(f"{word}\t{freq_level(freq)}\n")

    size = os.path.getsize(dst)
    print(f"总词频 {total}（TOTAL 常量用这个）")
    print(f"单字 {len(singles)} + 多字 {len(multi)} = {len(kept)} 条（词频 >= {min_freq}），{size / 1024:.0f} KB → {dst}")
    for probe in ["设计", "代码", "卧室", "地板", "图像", "公共层", "标点", "边界"]:
        print(f"  {'OK ' if probe in seen else '缺 '} {probe}", end="")
    print()


if __name__ == "__main__":
    main()
