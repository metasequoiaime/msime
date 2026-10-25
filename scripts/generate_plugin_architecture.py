#!/usr/bin/env python3
"""生成 docs/plugin-architecture.html：插件系统原理图，给不读代码的人看懂两种插件、生命周期和安全模型，底部折叠区是给开发者的模块分层、调用时序和代码地图。

页面是单个自带样式的 HTML，图都是这里按坐标算出的内联 SVG，不引用任何外部资源。改图请改这个脚本再重新生成，不要手改 HTML：

    python3 scripts/generate_plugin_architecture.py

内容取自 docs/plugins.md 和 crates/host-api、crates/script-runtime、crates/client-services/src/plugins 的代码，两者不一致时以代码为准，改了生命周期顺序、钩子、能力或限制要同步改这里。
"""

import html
import os
import sys

E = html.escape

# ---------- SVG 辅助函数 ----------

def box(x, y, w, h, cls, title, lines=(), rx=12, title_cls="t", line_cls="s"):
    out = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" class="{cls}"/>']
    ty = y + 24
    out.append(f'<text x="{x + 14}" y="{ty}" class="{title_cls}">{E(title)}</text>')
    ly = ty + 21
    for line in lines:
        out.append(f'<text x="{x + 14}" y="{ly}" class="{line_cls}">{E(line)}</text>')
        ly += 18
    return "\n".join(out)


def marker(mid, color):
    return (
        f'<marker id="{mid}" viewBox="0 0 10 10" refX="8.5" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        f'<path d="M0,1 L9,5 L0,9 z" fill="{color}"/></marker>'
    )


def arrow(x1, y1, x2, y2, mid, cls):
    return f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="{cls}" marker-end="url(#{mid})"/>'


def label(x, y, text, anchor="start", cls="lbl"):
    return f'<text x="{x}" y="{y}" class="{cls}" text-anchor="{anchor}">{E(text)}</text>'


GRAY = "#64748b"
ORANGE = "#ea580c"
TEAL = "#0d9488"
INDIGO = "#4f46e5"
RED = "#dc2626"


# ---------- 1. 一张图看懂 ----------

def overview():
    o = ['<svg viewBox="0 0 1100 420" class="diagram hero-svg" role="img" aria-label="插件在输入法里的位置">', "<defs>",
         marker("h1", GRAY), marker("h1o", ORANGE),
         '<linearGradient id="imeg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="1" stop-color="#f5f7ff"/></linearGradient>',
         '<filter id="sh" x="-10%" y="-10%" width="120%" height="130%"><feDropShadow dx="0" dy="6" stdDeviation="10" flood-color="#1e293b" flood-opacity="0.08"/></filter>',
         "</defs>"]
    # keyboard
    o.append('<g filter="url(#sh)"><rect x="30" y="150" width="210" height="110" rx="16" fill="#ffffff" stroke="#cbd5e1" stroke-width="1.5"/></g>')
    for r, (y, n, off) in enumerate([(166, 9, 0), (192, 8, 10), (218, 7, 20)]):
        for i in range(n):
            o.append(f'<rect x="{46 + off + i * 20.5}" y="{y}" width="16" height="18" rx="4" fill="#eef2f7" stroke="#cbd5e1"/>')
    o.append('<rect x="88" y="242" width="94" height="10" rx="4" fill="#eef2f7" stroke="#cbd5e1"/>')
    o.append(label(135, 292, "你按下按键", "middle", "big"))
    o.append(label(135, 314, "n · i · 空格 …", "middle", "sub"))
    o.append(arrow(248, 205, 296, 205, "h1", "ag"))
    # input method
    o.append('<g filter="url(#sh)"><rect x="302" y="24" width="520" height="372" rx="22" fill="url(#imeg)" stroke="#c7d2fe" stroke-width="2"/></g>')
    o.append(label(326, 60, "输入法", "start", "h"))
    o.append(label(414, 60, "把拼音变成候选词，再把你选的字写进输入框", "start", "sub"))
    # data packs
    o.append('<rect x="326" y="80" width="472" height="112" rx="16" fill="#f0fdfa" stroke="#5eead4" stroke-width="1.5"/>')
    o.append('<circle cx="356" cy="112" r="16" fill="#ccfbf1"/><path d="M348,106 h16 v14 h-16 z M348,106 l8,-5 l8,5" fill="none" stroke="#0d9488" stroke-width="2" stroke-linejoin="round"/>')
    o.append(label(382, 118, "素材包", "start", "t2 teal"))
    o.append(label(452, 118, "只是数据，输入法自己读来用", "start", "sub"))
    o.append(label(346, 148, "声音、背景音乐、/ 指令、打字特效、快捷短语、", "start", "body"))
    o.append(label(346, 172, "辅助码、单词本、符号——里面没有任何会运行的东西", "start", "body"))
    # sandbox
    o.append('<rect x="326" y="208" width="472" height="166" rx="16" fill="#fff7ed" stroke="#fb923c" stroke-width="2" stroke-dasharray="8 6"/>')
    o.append('<circle cx="356" cy="240" r="16" fill="#ffedd5"/><rect x="348" y="239" width="16" height="12" rx="2" fill="#ea580c"/><path d="M351,239 v-4 a5,5 0 0 1 10,0 v4" fill="none" stroke="#ea580c" stroke-width="2"/>')
    o.append(label(382, 246, "脚本插件", "start", "t2 orange"))
    o.append(label(470, 246, "关在沙箱里运行的小程序", "start", "sub"))
    o.append(label(346, 278, "· 只在几个固定时机被叫醒", "start", "body"))
    o.append(label(346, 302, "· 只拿得到你勾选允许的内容", "start", "body"))
    o.append(label(346, 326, "· 能做：调整候选顺序、改写要写进去的文字、", "start", "body"))
    o.append(label(346, 350, "  给 / 指令出结果", "start", "body"))
    # consent
    o.append('<g filter="url(#sh)"><rect x="30" y="336" width="210" height="60" rx="14" fill="#ffffff" stroke="#fdba74" stroke-width="1.5"/></g>')
    o.append('<path d="M58,350 l12,-4 l12,4 v10 c0,8 -6,12 -12,14 c-6,-2 -12,-6 -12,-14 z" fill="#ffedd5" stroke="#ea580c" stroke-width="1.8"/><path d="M64,361 l4,4 l8,-8" fill="none" stroke="#ea580c" stroke-width="2"/>')
    o.append(label(94, 362, "你的授权", "start", "t2"))
    o.append(label(94, 382, "每项权限单独勾选", "start", "sub"))
    o.append('<path d="M240,366 C 280,366 290,320 322,312" fill="none" class="ao" marker-end="url(#h1o)"/>')
    # output
    o.append(arrow(828, 205, 876, 205, "h1", "ag"))
    o.append('<g filter="url(#sh)"><rect x="882" y="140" width="190" height="130" rx="16" fill="#ffffff" stroke="#cbd5e1" stroke-width="1.5"/></g>')
    o.append('<rect x="882" y="140" width="190" height="28" rx="16" fill="#f1f5f9"/><rect x="882" y="156" width="190" height="12" fill="#f1f5f9"/>')
    for i, c in enumerate(["#f87171", "#fbbf24", "#34d399"]):
        o.append(f'<circle cx="{900 + i * 14}" cy="154" r="4.5" fill="{c}"/>')
    o.append('<rect x="900" y="186" width="154" height="40" rx="8" fill="#ffffff" stroke="#a5b4fc" stroke-width="1.5"/>')
    o.append(label(912, 212, "用 Rust 写", "start", "type"))
    o.append('<line x1="996" y1="194" x2="996" y2="218" stroke="#4f46e5" stroke-width="2"/>')
    o.append('<rect x="900" y="236" width="110" height="8" rx="4" fill="#e2e8f0"/><rect x="900" y="252" width="80" height="8" rx="4" fill="#e2e8f0"/>')
    o.append(label(977, 292, "文字出现在输入框", "middle", "big"))
    o.append(label(977, 314, "这一步叫「上屏」", "middle", "sub"))
    o.append("</svg>")
    return "\n".join(o)


# ---------- 插件实例的生命周期 ----------

def instance_lifecycle():
    o = ['<svg viewBox="0 0 1100 320" class="diagram" role="img" aria-label="插件实例的生命周期">', "<defs>",
         marker("l1", GRAY), marker("l1o", ORANGE), marker("l1r", RED), marker("l1g", TEAL), "</defs>"]
    nodes = [
        (20, 150, "mob", "① 装好", ["导入后默认停用"]),
        (195, 165, "scr", "② 你授权", ["逐项勾选权限", "和插件内容绑定"]),
        (385, 160, "core", "③ 准备好", ["后台编译，", "不占打字时间"]),
        (570, 180, "scr", "④ 创建实例", ["进入输入框后第一次", "要用时，运行初始化"]),
        (775, 140, "scr", "⑤ 收事件", ["按授权投递"]),
        (940, 140, "core", "⑥ 丢弃", ["离开或切换输入框，", "状态全部清空"]),
    ]
    for x, w, cls, t, ls in nodes:
        o.append(box(x, 70, w, 80, cls, t, ls))
    for (x, w, *_), (nx, *_) in zip(nodes, nodes[1:]):
        o.append(arrow(x + w + 2, 110, nx - 3, 110, "l1", "ag"))
    o.append('<path d="M1010,70 C1010,34 660,34 660,68" class="agr" fill="none" marker-end="url(#l1g)"/>')
    o.append(label(835, 26, "下一个输入框：重新创建，什么都不记得", "middle", "lblg"))
    o.append(box(700, 222, 240, 80, "bad", "这一次出错", ["改动作废，文字照原样上屏", "严重错误：本输入框里不再创建"]))
    o.append(arrow(845, 150, 845, 220, "l1r", "ar"))
    o.append(box(380, 222, 260, 80, "bad", "停用（熔断）", ["经常出错或太慢就自动停用，", "直到你改授权或重启输入法"]))
    o.append(arrow(698, 262, 643, 262, "l1r", "ar"))
    o.append(label(670, 254, "反复", "middle"))
    o.append(arrow(440, 220, 300, 153, "l1g", "agr"))
    o.append(label(392, 200, "恢复", "start", "lblg"))
    o.append(box(20, 222, 260, 80, "warn", "插件更新了", ["内容哪怕变了一个字节，", "之前的授权自动作废"]))
    o.append(arrow(200, 220, 250, 153, "l1o", "ao"))
    o.append(label(232, 196, "需重新授权", "start", "lblo"))
    o.append("</svg>")
    return "\n".join(o)


# ---------- 技术区：分层架构 ----------

def architecture():
    m, mo, mg = "a1", "a1o", "a1g"
    o = ['<svg viewBox="0 0 1120 650" class="diagram" role="img" aria-label="模块分层">', "<defs>", marker(m, GRAY), marker(mo, ORANGE), marker(mg, TEAL), "</defs>"]
    hosts = [
        ("host", "macOS", ["IMK 输入法进程"]),
        ("host", "Windows", ["MetasequoiaImeServer（TIP 不跑脚本）"]),
        ("host", "Linux", ["IBus 引擎进程 · Fcitx5 addon"]),
        ("mob", "iOS · Android · 鸿蒙", ["不选择加入生命周期，没有脚本"]),
    ]
    for x, (cls, t, ls) in zip([20, 294, 568, 842], hosts):
        o.append(box(x, 20, 258, 62, cls, t, ls))
        o.append(arrow(x + 129, 82, x + 129, 99, m, "ag"))
    o.append('<rect x="20" y="100" width="1080" height="34" rx="10" class="ffi"/>')
    o.append(label(560, 122, "FFI：msime_client_key · focus · set_lifecycle · set_field_traits · scripts_enable · msime_client_plugins", "middle", "ffit"))
    o.append(arrow(560, 134, 560, 147, m, "ag"))
    o.append('<rect x="20" y="148" width="1080" height="160" rx="14" class="cont"/>')
    o.append(label(34, 167, "crates/host-api", "start", "cap"))
    inner = [
        (36, "core", "lifecycle.rs 生命周期核心", ["每次 FFI 调用后 derive 事件", "纯函数，按键路径不分配内存", "事件本身不带文本", "未选择加入的会话：零事件"]),
        (302, "scr", "scripts/set.rs 会话级", ["按授权顺序组成 plan", "实例延迟创建，按轮计 fuel", "focus / destroy 丢弃全部实例", "读 script-grants.json"]),
        (568, "scr", "scripts/process.rs 进程级", ["模块缓存（按内容哈希）", "后台编译线程", "熔断 + script-health.json", "panic 总开关"]),
        (834, "dat", "数据包消费方", ["plugin_tables：指令/短语/辅助码", "key_sound：音效与背景音乐", "按 mtime+长度+inode 重读", "特效：宿主内置画法 + 参数"]),
    ]
    for x, cls, t, ls in inner:
        o.append(box(x, 185, 240, 110, cls, t, ls))
    o.append(arrow(276, 238, 300, 238, mo, "ao"))
    o.append(label(289, 230, "事件", "middle"))
    o.append(arrow(542, 238, 566, 238, mo, "ao"))
    o.append(label(555, 230, "模块", "middle"))
    o.append(f'<path d="M180,185 Q560,172 950,185" class="agr" fill="none" marker-end="url(#{mg})"/>')
    o.append(label(560, 168, "FEEDBACK：同一批事件也驱动内置按键音 / 特效 / 音乐", "middle", "lblg"))
    o.append(box(20, 352, 250, 118, "rt", "crates/input-runtime", ["transform.rs 两个注入点：", "· CandidateTransform", "· CommandProvider", "runtime 本身不知道脚本存在"]))
    o.append(box(290, 352, 400, 118, "scr", "crates/script-runtime（沙箱）", ["wasmi 2.0.0 解释器 · core wasm · 无 WASI / JIT", "abi.rs：能力、钩子、字段编号、返回码（只追加）", "module.rs：compile / validate，双向最小权限", "instance.rs：fuel 配额 · 内存上限 · 快照复位"]))
    o.append(box(710, 352, 390, 118, "dat", "client-services/src/plugins", ["9 种 kind 的清单解析与校验", "import：staging → 校验 → rename 整包替换", "script_pack / script_grants / script_commands", "community：社区分享，复用同一套校验"]))
    o.append(arrow(200, 352, 372, 297, mo, "ao"))
    o.append(label(214, 330, "经 trait 回调", "start", "lblo"))
    o.append(arrow(440, 295, 440, 350, mo, "ao"))
    o.append(label(446, 326, "实例化 · 投递", "start", "lblo"))
    o.append(arrow(620, 295, 620, 350, mo, "ao"))
    o.append(label(626, 326, "后台编译", "start", "lblo"))
    o.append(arrow(770, 295, 790, 350, mo, "ao"))
    o.append(label(790, 318, "读包 · 核哈希", "start", "lblo"))
    o.append(arrow(954, 295, 954, 350, mg, "agr"))
    o.append(label(960, 326, "读数据包", "start", "lblg"))
    o.append('<rect x="20" y="515" width="760" height="118" rx="14" class="disk"/>')
    o.append(label(34, 540, "磁盘", "start", "t"))
    files = [
        (40, 566, "<state>/plugins/<kind>/<id>/", "已安装的包"),
        (40, 602, "<plugins>/script-grants.json", "授权：顺序即执行顺序，绑定内容哈希"),
        (410, 566, "<user_data>/script-health.json", "只记停用与类别"),
        (410, 602, "安装包内只读 plugin-packs/", "官方包 uuid · latex-symbols"),
    ]
    for x, y, path, note in files:
        o.append(f'<text x="{x}" y="{y}" class="mono">{E(path)}</text>')
        o.append(f'<text x="{x}" y="{y + 16}" class="s">{E(note)}</text>')
    o.append(arrow(740, 470, 740, 513, m, "ag"))
    o.append(label(746, 496, "读写（持锁、原子替换）"))
    o.append(box(800, 515, 300, 118, "tool", "crates/pack-tool：msime-pack", ["validate · hash · run", "和输入法同一份校验与执行代码", "作者在本地按事件夹具试跑脚本"]))
    o.append(arrow(1040, 515, 1040, 472, m, "ag"))
    o.append(label(1046, 498, "复用"))
    o.append("</svg>")
    return "\n".join(o)


# ---------- 技术区：时序 ----------

def sequence():
    lanes = [
        ("宿主外壳", "platforms/*", "host"),
        ("host-api", "lib.rs / lifecycle.rs", "core"),
        ("input-runtime", "Runtime", "rt"),
        ("ScriptSet", "scripts/set.rs", "scr"),
        ("wasm 实例", "script-runtime", "scr"),
    ]
    lx = [100, 320, 545, 775, 1005]
    msgs = [
        (0, 1, "msime_client_key：开新一轮", "call", "① 按键"),
        (0, 1, "派发调用（字符 / 选词…）", "call", "② 派发前"),
        (1, 3, "input（先采样组字，再发事件）", "call", None),
        (3, 4, "input 钩子（订阅了才投递）", "call", None),
        (1, 2, "runtime.dispatch(Action)", "call", "③ 派发"),
        (2, 3, "CommandProvider：refresh 开头，刚打完 /触发词", "call", None),
        (3, 4, "command（快照实例 + 64B 随机数）", "call", None),
        (3, 2, "插件行，引擎按预编辑串缓存", "ret", None),
        (2, 3, "CandidateTransform：refresh 之后，前 20 行", "call", None),
        (3, 4, "candidates 钩子", "call", None),
        (3, 2, "重排 / 隐藏 / 注释，按授权顺序合成", "ret", None),
        (2, 1, "Transition + DispatchFacts", "ret", None),
        (1, 1, "complete_transition：全角转换、中英文加空格", "self", "④ 上屏前"),
        (1, 3, "selection_retracted / candidate_selected", "call", None),
        (1, 3, "before_commit（可改写时才让脚本改）", "call", None),
        (3, 4, "快照实例改写 body", "call", None),
        (3, 1, "新 body 接回原 tail，写回 transition.commit", "ret", None),
        (1, 3, "commit → composition_* → candidates_shown / hidden", "call", "⑤ 上屏后"),
        (1, 1, "同一批事件 → 内置音效 / 特效 / 音乐（FEEDBACK）", "self", None),
        (1, 0, "transition JSON：上屏 / 候选 / 预编辑", "ret", "⑥ 返回"),
    ]
    top, step = 96, 40
    height = top + step * len(msgs) + 24
    o = [f'<svg viewBox="0 0 1120 {height}" class="diagram" role="img" aria-label="一次按键的时序">', "<defs>", marker("s1", GRAY), marker("s1o", ORANGE), "</defs>"]
    bands = []
    for i, msg in enumerate(msgs):
        if msg[4]:
            bands.append([msg[4], i, i])
        else:
            bands[-1][2] = i
    for n, (name, a, b) in enumerate(bands):
        y0 = top + a * step - 24
        y1 = top + b * step + 16
        o.append(f'<rect x="8" y="{y0}" width="1104" height="{y1 - y0}" rx="8" class="{"band0" if n % 2 == 0 else "band1"}"/>')
        o.append(label(16, y0 + 16, name, "start", "bandt"))
    for x, (t, sub, cls) in zip(lx, lanes):
        o.append(f'<line x1="{x}" y1="64" x2="{x}" y2="{height - 10}" class="life"/>')
        o.append(f'<rect x="{x - 88}" y="14" width="176" height="48" rx="10" class="{cls}"/>')
        o.append(label(x, 34, t, "middle", "t"))
        o.append(label(x, 52, sub, "middle", "s"))
    for i, (a, b, text, kind, _) in enumerate(msgs):
        y = top + i * step
        script = a >= 3 or b >= 3
        mid = "s1o" if script else "s1"
        if kind == "self":
            x = lx[a]
            o.append(f'<path d="M{x},{y - 6} h34 v14 h-32" class="ag" fill="none" marker-end="url(#s1)"/>')
            o.append(label(x + 42, y + 5, text, "start", "msg"))
            continue
        x1, x2 = lx[a], lx[b]
        cls = ("ao" if script else "ag") + (" dash" if kind == "ret" else "")
        end = x2 - 3 if x2 > x1 else x2 + 3
        o.append(f'<line x1="{x1}" y1="{y}" x2="{end}" y2="{y}" class="{cls}" marker-end="url(#{mid})"/>')
        o.append(label((x1 + x2) / 2, y - 6, text, "middle", "msg"))
    o.append("</svg>")
    return "\n".join(o)


CSS = r"""
:root{--ink:#0f172a;--ink2:#334155;--muted:#64748b;--line:#e2e8f0;--bg:#f6f7fb;--card:#ffffff;--teal:#0d9488;--teal-bg:#f0fdfa;--teal-line:#99f6e4;--orange:#ea580c;--orange-bg:#fff7ed;--orange-line:#fed7aa;--indigo:#4f46e5;--indigo-bg:#eef2ff}
html,body{background-color:#f6f7fb;color:#0f172a;margin:0}
body{font-family:-apple-system,BlinkMacSystemFont,"PingFang SC","Hiragino Sans GB","Microsoft YaHei","Noto Sans CJK SC",sans-serif;font-size:16px;line-height:1.7;-webkit-font-smoothing:antialiased}
main{max-width:1160px;margin:0 auto;padding:0 24px 80px}
code,.mono{font-family:"SF Mono",Menlo,Consolas,monospace}
code{font-size:.86em;background:#eef1f6;padding:1px 6px;border-radius:5px;color:#1e293b}
.hero{background:linear-gradient(135deg,#eef2ff 0%,#f0fdfa 55%,#fff7ed 100%);border-bottom:1px solid var(--line)}
.hero-inner{max-width:1160px;margin:0 auto;padding:56px 24px 40px}
.eyebrow{display:inline-block;font-size:13px;font-weight:600;letter-spacing:.06em;color:var(--indigo);background:#ffffff;border:1px solid #c7d2fe;border-radius:999px;padding:3px 12px}
h1{font-size:40px;line-height:1.25;margin:16px 0 12px;letter-spacing:-.01em}
.hero p{font-size:18px;color:var(--ink2);max-width:760px;margin:0}
section{margin-top:64px}
.kicker{font-size:13px;font-weight:700;color:var(--muted);letter-spacing:.08em}
h2{font-size:28px;line-height:1.3;margin:4px 0 8px;letter-spacing:-.01em}
.intro{color:var(--ink2);max-width:780px;margin:0 0 22px}
.card{background:var(--card);border:1px solid var(--line);border-radius:18px;box-shadow:0 1px 2px rgba(15,23,42,.04),0 10px 30px rgba(15,23,42,.05)}
svg.diagram{width:100%;height:auto;display:block}
.svgwrap{padding:18px}
.glossary{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;margin-top:18px}
.term{padding:14px 18px}
.term b{display:block;font-size:16px}
.term span{color:var(--ink2);font-size:14.5px}
.two{display:grid;grid-template-columns:1fr 1fr;gap:20px}
.kind{padding:24px 24px 20px;border-top:5px solid}
.kind.data{border-top-color:var(--teal)}
.kind.script{border-top-color:var(--orange)}
.kind h3{font-size:22px;margin:0 0 4px}
.kind .tag{color:var(--muted);font-size:14.5px;margin-bottom:16px}
.grid8{display:grid;grid-template-columns:1fr 1fr;gap:10px}
.item{display:flex;gap:12px;align-items:flex-start;background:var(--teal-bg);border:1px solid var(--teal-line);border-radius:12px;padding:10px 12px}
.item .ic{flex:0 0 34px;height:34px;border-radius:10px;background:#ffffff;border:1px solid var(--teal-line);display:flex;align-items:center;justify-content:center;font-weight:700;color:var(--teal);font-size:15px}
.item b{display:block;font-size:15px;line-height:1.4}
.item span{font-size:13.5px;color:var(--ink2);line-height:1.5}
.ex{background:var(--orange-bg);border:1px solid var(--orange-line);border-radius:12px;padding:12px 14px;margin-bottom:10px}
.ex b{font-size:15.5px}
.ex .demo{display:flex;align-items:center;flex-wrap:wrap;gap:8px;margin-top:6px;font-size:14px;color:var(--ink2)}
.pill{display:inline-block;background:#ffffff;border:1px solid #e2e8f0;border-radius:8px;padding:2px 10px;font-size:15px;color:var(--ink)}
.pill.good{border-color:#fdba74;background:#ffffff}
.to{color:var(--orange);font-weight:700}
.foot{display:flex;flex-wrap:wrap;gap:8px;margin-top:14px}
.badge{font-size:13px;font-weight:600;border-radius:999px;padding:3px 10px;border:1px solid}
.b-teal{color:#0f766e;background:#ccfbf1;border-color:#99f6e4}
.b-orange{color:#c2410c;background:#ffedd5;border-color:#fed7aa}
.b-gray{color:#475569;background:#f1f5f9;border-color:#e2e8f0}
.b-red{color:#b91c1c;background:#fee2e2;border-color:#fecaca}
.steps{display:grid;grid-template-columns:repeat(3,1fr) 1.35fr;gap:14px;align-items:stretch}
.st{padding:20px;position:relative}
.num{width:34px;height:34px;border-radius:50%;background:var(--indigo);color:#ffffff;font-weight:700;display:flex;align-items:center;justify-content:center;margin-bottom:10px}
.st b{font-size:17px;display:block;margin-bottom:4px}
.st p{margin:0;color:var(--ink2);font-size:14.5px}
.st:not(:last-child)::after{content:"";position:absolute;right:-12px;top:36px;width:10px;height:10px;border-top:2.5px solid #a5b4fc;border-right:2.5px solid #a5b4fc;transform:rotate(45deg)}
.fork{display:flex;flex-direction:column;gap:10px}
.fork .branch{border-radius:12px;padding:10px 12px;font-size:14.5px;color:var(--ink2)}
.fork .branch b{font-size:15.5px;color:var(--ink)}
.branch.data{background:var(--teal-bg);border:1px solid var(--teal-line)}
.branch.script{background:var(--orange-bg);border:1px solid var(--orange-line)}
.timeline{display:grid;grid-template-columns:repeat(5,1fr);gap:14px;position:relative}
.timeline::before{content:"";position:absolute;left:4%;right:4%;top:22px;height:3px;background:linear-gradient(90deg,#c7d2fe,#fed7aa);border-radius:3px}
.stage{position:relative;padding-top:52px}
.dot{position:absolute;top:8px;left:50%;transform:translateX(-50%);width:30px;height:30px;border-radius:50%;background:#ffffff;border:3px solid var(--indigo);display:flex;align-items:center;justify-content:center;font-weight:700;color:var(--indigo);font-size:14px}
.stage.can .dot{border-color:var(--orange);color:var(--orange)}
.stage .card{padding:14px 14px 16px;height:100%;box-sizing:border-box}
.stage h4{margin:0 0 8px;font-size:16.5px}
.stage .what{font-size:13.5px;color:var(--ink2);margin-top:10px;line-height:1.55}
.mock{background:#f8fafc;border:1px solid var(--line);border-radius:10px;padding:10px;min-height:66px;display:flex;flex-direction:column;justify-content:center;gap:6px;font-size:14px}
.keys{display:flex;gap:6px;justify-content:center}
.key{min-width:30px;height:30px;padding:0 6px;border-radius:7px;background:#ffffff;border:1px solid #cbd5e1;box-shadow:0 2px 0 #cbd5e1;display:flex;align-items:center;justify-content:center;font-family:"SF Mono",Menlo,monospace;font-size:14px}
.cands{display:flex;gap:4px;flex-wrap:wrap;font-size:14px}
.cands span{background:#ffffff;border:1px solid #e2e8f0;border-radius:6px;padding:1px 6px}
.cands span.rare{color:#94a3b8;border-style:dashed}
.cands span.hl{background:var(--indigo);color:#ffffff;border-color:var(--indigo)}
.mini{font-size:12px;color:var(--muted)}
.field{background:#ffffff;border:1.5px solid #a5b4fc;border-radius:8px;padding:4px 10px;font-size:15px}
.cmd{margin-top:18px;display:grid;grid-template-columns:1.1fr 2fr;gap:18px;padding:18px 20px;align-items:center}
.cmd .rows{font-family:"SF Mono",Menlo,monospace;font-size:13.5px;display:flex;flex-direction:column;gap:4px}
.cmd .rows em{font-style:normal;color:#94a3b8;font-family:-apple-system,"PingFang SC",sans-serif;font-size:12px;margin-left:6px}
.cmd .rows span{background:#f8fafc;border:1px solid var(--line);border-radius:6px;padding:2px 8px}
.legend{display:flex;gap:18px;flex-wrap:wrap;margin:-6px 0 18px;font-size:14px;color:var(--ink2)}
.legend i{display:inline-block;width:12px;height:12px;border-radius:50%;margin-right:6px;vertical-align:-1px;border:3px solid}
.safe{display:grid;grid-template-columns:repeat(3,1fr);gap:16px}
.sf{padding:20px}
.sf .ic{width:42px;height:42px;border-radius:12px;display:flex;align-items:center;justify-content:center;margin-bottom:10px}
.sf b{display:block;font-size:17px;margin-bottom:4px}
.sf p{margin:0;color:var(--ink2);font-size:14.5px}
.honest{margin-top:16px;padding:16px 20px;border-left:5px solid #f59e0b;background:#fffbeb;border-radius:12px;color:#78350f;font-size:15px}
.flow{display:grid;grid-template-columns:1fr 1fr 1fr 1fr;gap:16px}
.fs{padding:18px;border-top:5px solid}
.fs b{display:block;font-size:17px;margin-bottom:4px}
.fs p{margin:0;font-size:14.5px;color:var(--ink2)}
.fs.ok{border-top-color:#10b981}.fs.warn{border-top-color:#f59e0b}.fs.bad{border-top-color:#ef4444}.fs.back{border-top-color:var(--indigo)}
details.dev{margin-top:64px}
details.dev>summary{cursor:pointer;list-style:none;padding:20px 24px;font-size:20px;font-weight:700}
details.dev>summary::-webkit-details-marker{display:none}
details.dev>summary::before{content:"+";display:inline-flex;width:28px;height:28px;border-radius:8px;background:var(--indigo-bg);color:var(--indigo);align-items:center;justify-content:center;margin-right:12px;font-weight:700}
details.dev[open]>summary::before{content:"−"}
details.dev>summary span{font-size:14.5px;font-weight:400;color:var(--muted);margin-left:8px}
.devbody{padding:0 24px 28px}
.devbody h3{font-size:19px;margin:28px 0 8px}
.devbody .note{color:var(--muted);font-size:14px;margin:0 0 12px}
.devbody .svgbox{border:1px solid var(--line);border-radius:14px;padding:10px;background:#ffffff}
table{border-collapse:collapse;width:100%;font-size:14px;background:#ffffff;border-radius:12px;overflow:hidden}
th,td{border-bottom:1px solid var(--line);padding:9px 12px;text-align:left;vertical-align:top}
th{background:#f8fafc;font-weight:600;color:var(--ink2)}
svg text{font-family:-apple-system,BlinkMacSystemFont,"PingFang SC","Microsoft YaHei","Noto Sans CJK SC",sans-serif}
svg .h{font-size:24px;font-weight:700;fill:#0f172a}
svg .big{font-size:17px;font-weight:600;fill:#0f172a}
svg .t2{font-size:18px;font-weight:700;fill:#0f172a}
svg .t2.teal{fill:#0f766e}svg .t2.orange{fill:#c2410c}
svg .sub{font-size:14px;fill:#64748b}
svg .body{font-size:15px;fill:#334155}
svg .type{font-size:18px;fill:#0f172a}
svg rect{stroke-width:1.5}
.host{fill:#eff6ff;stroke:#60a5fa}.mob{fill:#f8fafc;stroke:#cbd5e1}.core{fill:#f8fafc;stroke:#64748b}
.scr{fill:#fff7ed;stroke:#fb923c}.dat{fill:#f0fdfa;stroke:#2dd4bf}.rt{fill:#eef2ff;stroke:#818cf8}
.tool{fill:#f8fafc;stroke:#94a3b8}.bad{fill:#fef2f2;stroke:#f87171}.warn{fill:#fffbeb;stroke:#fbbf24}.disk{fill:#fcfcfd;stroke:#cbd5e1;stroke-dasharray:6 5}
.cont{fill:#ffffff;stroke:#cbd5e1;stroke-dasharray:7 5}.ffi{fill:#1e293b;stroke:#1e293b}
.band0{fill:#f8fafc;stroke:none}.band1{fill:#ffffff;stroke:none}
svg .t{font-size:14px;font-weight:600;fill:#0f172a}
svg .s{font-size:12px;fill:#475569}
svg .mono{font-size:12.5px;fill:#1e293b;font-family:"SF Mono",Menlo,Consolas,monospace}
svg .cap{font-size:12px;fill:#64748b;font-weight:600}
svg .ffit{font-size:12.5px;fill:#ffffff;font-family:"SF Mono",Menlo,Consolas,monospace}
svg .lbl,svg .lblo,svg .lblg,svg .msg{font-size:11.5px;paint-order:stroke;stroke:#ffffff;stroke-width:4px;stroke-linejoin:round}
svg .lbl,svg .msg{fill:#475569}svg .lblo{fill:#c2410c}svg .lblg{fill:#0f766e}
svg .bandt{font-size:12px;font-weight:700;fill:#94a3b8}
svg .ag{stroke:#64748b;stroke-width:1.6}svg .ar{stroke:#dc2626;stroke-width:1.7}svg .ao{stroke:#ea580c;stroke-width:1.7}svg .agr{stroke:#0d9488;stroke-width:1.7}
svg .dash{stroke-dasharray:5 4}
svg .life{stroke:#cbd5e1;stroke-width:1.2;stroke-dasharray:4 4}

.b-indigo{color:#4338ca;background:#eef2ff;border-color:#c7d2fe}
.lc{display:flex;flex-direction:column;align-items:stretch;gap:0}
.lc-end{display:flex;gap:14px;align-items:center;background:#ffffff;border:1px solid var(--line);border-radius:16px;padding:14px 18px;box-shadow:0 1px 2px rgba(15,23,42,.04);max-width:640px}
.lc-end.out{align-self:flex-end}
.lc-end b{font-size:17px;margin-right:8px}
.lc-end span{display:block;color:var(--ink2);font-size:14.5px}
.lc-ic{flex:0 0 40px;height:40px;border-radius:12px;background:var(--indigo);color:#ffffff;display:flex!important;align-items:center;justify-content:center;font-size:20px;font-weight:700}
.lc-end.out .lc-ic{background:#64748b}
.lc-loop{margin:14px 0;border:2px dashed #c7d2fe;border-radius:22px;padding:16px 14px 14px;background:linear-gradient(180deg,#f8faff,#ffffff)}
.lc-head b{font-size:17px;margin-right:10px}
.lc-head span{color:var(--ink2);font-size:14.5px}
.lc-grid{display:grid;grid-template-columns:repeat(7,minmax(0,1fr));gap:8px;margin-top:14px}
.ev{background:#ffffff;border:1.5px solid #c7d2fe;border-radius:14px;padding:12px 9px;display:flex;flex-direction:column;gap:5px;position:relative}
.ev.can{border-color:#fdba74;background:#fffaf5}
.ev:not(:last-child)::after{content:"";position:absolute;right:-9px;top:22px;width:8px;height:8px;border-top:2px solid #a5b4fc;border-right:2px solid #a5b4fc;transform:rotate(45deg)}
.evn{width:26px;height:26px;border-radius:50%;background:#eef2ff;color:var(--indigo);font-weight:700;font-size:13px;display:flex;align-items:center;justify-content:center}
.ev.can .evn{background:#ffedd5;color:var(--orange)}
.ev b{font-size:16px}
.ev code{font-size:10.5px;align-self:flex-start;white-space:nowrap;padding:1px 4px}
.ev .badge{align-self:flex-start;font-size:12px;padding:1px 8px}
.ev p{margin:2px 0 0;font-size:13px;color:var(--ink2);line-height:1.55}
.lc-back{margin-top:12px;text-align:center;font-size:14px;font-weight:600;color:var(--indigo);background:#eef2ff;border-radius:999px;padding:6px}
.lc-notes{display:grid;grid-template-columns:repeat(3,1fr);gap:14px;margin-top:20px}
.lc-note{padding:16px 18px}
.lc-note b{font-size:16px;margin-right:8px}
.lc-note p{margin:6px 0 0;font-size:14.5px;color:var(--ink2)}
.sub-h{font-size:21px;margin:40px 0 6px}
@media (max-width:1000px){.lc-grid{grid-template-columns:repeat(2,1fr)}.ev::after{display:none}.lc-notes{grid-template-columns:1fr}}
@media (max-width:900px){.two,.steps,.timeline,.safe,.flow,.glossary,.cmd{grid-template-columns:1fr}.timeline::before{display:none}.st::after{display:none}h1{font-size:30px}}
"""

ICON_LOCK = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#ea580c" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></svg>'
ICON_EYE = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#4f46e5" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></svg>'
ICON_KEY = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#0d9488" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10"/><path d="M3 3l18 18" stroke="#dc2626"/></svg>'
ICON_RESET = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#7c3aed" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v5h5"/></svg>'
ICON_TIMER = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#d97706" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2.5M9 2h6"/></svg>'
ICON_SHIELD = '<svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#059669" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M9 12l2 2 4-4"/></svg>'

BODY = r"""
<div class="hero"><div class="hero-inner">
<span class="eyebrow">水杉输入法 · 插件系统</span>
<h1>插件是怎么工作的</h1>
<p>插件让输入法多出新本事：换按键音、加快捷短语、让生僻字靠后、在中英文之间自动加空格……这一页用几张图讲清：插件有哪两种，它们的生命周期是怎样的、在你打字时什么时候出手，以及为什么它们没法偷看你打的字。</p>
</div></div>
<main>

<section>
<div class="kicker">一张图看懂</div>
<h2>插件住在输入法里，但分两个房间</h2>
<p class="intro">「素材包」只是文件，输入法读出来自己用；「脚本插件」是真正会运行的小程序，被关在一个隔离的沙箱里，只能在固定时机、拿着你允许它看的内容做事。</p>
<div class="card svgwrap">@@OVERVIEW@@</div>
<div class="glossary">
<div class="card term"><b>候选</b><span>打拼音时弹出的那排备选词，按数字或空格选择。</span></div>
<div class="card term"><b>上屏</b><span>选好的字真正写进输入框的那一刻。</span></div>
<div class="card term"><b>沙箱</b><span>一个封闭的运行环境：程序在里面跑，碰不到外面的文件、网络和其他程序。</span></div>
</div>
</section>

<section>
<div class="kicker">两种插件</div>
<h2>素材包和脚本插件，各管什么</h2>
<p class="intro">绝大多数插件是素材包，安全性来自它根本不能运行。脚本插件更强大，所以多了好几道锁。</p>
<div class="two">
<div class="card kind data">
<h3>素材包</h3>
<div class="tag">只有数据，没有代码 · 共 8 种</div>
<div class="grid8">
<div class="item"><div class="ic">音</div><div><b>按键音</b><span>每次按键的声音，或一串旋律</span></div></div>
<div class="item"><div class="ic">乐</div><div><b>背景音乐</b><span>打字时循环播放的曲子</span></div></div>
<div class="item"><div class="ic">/</div><div><b>指令表</b><span>打 /sig 出你的签名和日期</span></div></div>
<div class="item"><div class="ic">✦</div><div><b>打字特效</b><span>按键时的火花或闪光</span></div></div>
<div class="item"><div class="ic">K</div><div><b>短语表</b><span>快捷短语，如 Kalpha 打出 α</span></div></div>
<div class="item"><div class="ic">码</div><div><b>辅助码表</b><span>换一套辅助码方案</span></div></div>
<div class="item"><div class="ic">词</div><div><b>单词本</b><span>背单词用的词书</span></div></div>
<div class="item"><div class="ic">※</div><div><b>符号集</b><span>符号面板里多一组符号或颜文字</span></div></div>
</div>
<div class="foot"><span class="badge b-teal">电脑上 8 种都能导入</span><span class="badge b-teal">手机上可导入辅助码表</span><span class="badge b-teal">可以分享到社区</span></div>
</div>
<div class="card kind script">
<h3>脚本插件</h3>
<div class="tag">会运行的小程序 · 实验功能 · 仅 macOS、Windows、Linux</div>
<div class="ex"><b>中英文之间加空格</b><div class="demo"><span class="pill">用Rust写</span><span class="to">→</span><span class="pill good">用 Rust 写</span><span>在上屏之前改写文字</span></div></div>
<div class="ex"><b>生僻字靠后</b><div class="demo">打 yi 时 <span class="pill">乚</span><span class="pill">伇</span> 不再夹在 <span class="pill">一 以 已</span> 中间，挪到最后</div></div>
<div class="ex"><b>隐藏生僻字</b><div class="demo">打 kao 时不再出现 <span class="pill">丂</span>，常用字往前补位</div></div>
<div class="ex"><b>UUID</b><div class="demo">打 <span class="pill">/uuid</span> 给出一个随机 UUID 的几种写法（随安装包附带）</div></div>
<div class="foot"><span class="badge b-orange">装好后默认关闭</span><span class="badge b-orange">逐项授权才运行</span><span class="badge b-gray">不能分享到社区</span></div>
</div>
</div>
</section>

<section>
<div class="kicker">从安装到生效</div>
<h2>一个插件要走过的四步</h2>
<p class="intro">每个包都要先过同一道检查。检查不过就整包拒绝，不会出现「装了一半」的情况。</p>
<div class="steps">
<div class="card st"><div class="num">1</div><b>选一个插件包</b><p>一个文件夹、一个 .zip，或者从社区下载。</p></div>
<div class="card st"><div class="num">2</div><b>严格检查</b><p>文件名、大小、格式、每一项内容都按固定规则核对；有一处不合规就整包拒绝。</p></div>
<div class="card st"><div class="num">3</div><b>安装</b><p>检查通过才放进插件目录；同名的旧包整包换成新包。</p></div>
<div class="card st"><div class="num">4</div><b>打开它</b>
<div class="fork">
<div class="branch data"><b>素材包：</b>在「插件 → 我的插件」里选用或打开「启用」，马上生效。</div>
<div class="branch script"><b>脚本插件：</b>在「脚本插件（实验）」里逐项勾选它申请的权限。授权和插件内容绑在一起，插件哪怕改了一个字节，也要重新授权。</div>
</div></div>
</div>
</section>

<section>
<div class="kicker">打一个字的过程</div>
<h2>脚本插件在哪几个时机出手</h2>
<p class="intro">插件不是一直在跑，而是只在下面这几个时机被叫醒，而且只在它声明过、你授权过的时机被叫醒。橙色的时机插件能改结果，蓝色的只能看。</p>
<div class="legend"><span><i style="border-color:#ea580c"></i>能改结果</span><span><i style="border-color:#4f46e5"></i>只能看，改不了</span></div>
<div class="timeline">
<div class="stage"><div class="dot">1</div><div class="card"><h4>你按下按键</h4><div class="mock"><div class="keys"><span class="key">y</span><span class="key">i</span></div></div><div class="what">插件只知道「开始了一次输入」。想知道按的是哪个字母，要你授权「读取正在输入的内容」。</div></div></div>
<div class="stage can"><div class="dot">2</div><div class="card"><h4>候选出来之前</h4><div class="mock"><div class="cands"><span>一</span><span>以</span><span>已</span><span class="rare">乚</span><span class="rare">伇</span></div><div class="mini">生僻字被挪到最后</div></div><div class="what">插件可以调整顺序、隐藏某几个、给候选加注释。</div></div></div>
<div class="stage"><div class="dot">3</div><div class="card"><h4>你选中一个词</h4><div class="mock"><div class="cands"><span class="hl">用Rust写</span><span>用</span><span>永</span></div></div><div class="what">插件可以知道你选了哪个词（需要授权），但改不了你的选择。</div></div></div>
<div class="stage can"><div class="dot">4</div><div class="card"><h4>上屏之前</h4><div class="mock"><div class="cands"><span>用Rust写</span></div><div class="mini" style="color:#ea580c">↓ 改写</div><div class="cands"><span class="hl" style="background:#ea580c;border-color:#ea580c">用 Rust 写</span></div></div><div class="what">插件可以改写马上要写进去的文字。标点不能改。</div></div></div>
<div class="stage"><div class="dot">5</div><div class="card"><h4>上屏之后</h4><div class="mock"><div class="field">用 Rust 写。</div></div><div class="what">文字已经写进去了。插件可以知道写了什么（需要授权），只能看。</div></div></div>
</div>
<div class="card cmd">
<div><span class="badge b-orange">另一个时机</span><h4 style="margin:8px 0 4px;font-size:17px">打 / 指令时</h4><div style="color:#334155;font-size:14.5px">插件可以注册自己的指令。你打出它的指令名之后，它给出几行结果供你选择。它只看得到指令名后面你打的字母，看不到你平时打的字。</div></div>
<div style="display:flex;gap:18px;align-items:center;flex-wrap:wrap"><div class="field" style="font-family:'SF Mono',Menlo,monospace">/uuid</div><span class="to">→</span><div class="rows"><span>1  3f2a9c1e-7b4d-4e8a-9c21-5d6f0a1b2c3d <em>小写</em></span><span>2  3F2A9C1E-7B4D-4E8A-9C21-5D6F0A1B2C3D <em>大写</em></span><span>3  3f2a9c1e7b4d4e8a9c215d6f0a1b2c3d <em>无连字符</em></span><span>4  {3F2A9C1E-7B4D-4E8A-9C21-5D6F0A1B2C3D} <em>GUID</em></span></div></div>
</div>
</section>

<section>
<div class="kicker">生命周期</div>
<h2>插件在一个输入框里能收到的全部事件</h2>
<p class="intro">输入法把你在一个输入框里的整个过程拆成一串「生命周期事件」。插件在清单里声明想听哪几个，你再授权它能看哪些内容，它才会在这些时刻被叫醒。事件本身不带文字，文字要在被叫醒的那一刻、凭授权去读。</p>
<div class="lc">
<div class="lc-end"><span class="lc-ic">→</span><div><b>进入输入框</b><code>focus_in</code><span>点进一个输入框。插件实例此时还没创建，第一次要用时才创建。</span></div></div>
<div class="lc-loop">
<div class="lc-head"><b>每一次操作都走一遍</b><span>打字母、选词、翻页、删除都算一次操作。同一次操作里按下面的顺序发，没发生的事件直接跳过。</span></div>
<div class="lc-grid">
<div class="ev"><div class="evn">1</div><b>打字前</b><code>input</code><span class="badge b-indigo">只能看</span><p>一次操作开始。知道是哪类操作，授权后还能知道打了哪个字母。</p></div>
<div class="ev can"><div class="evn">2</div><b>出候选前</b><code>candidates</code><span class="badge b-orange">能改</span><p>候选排好、还没显示。可以重排、隐藏、加注释。</p></div>
<div class="ev"><div class="evn">3</div><b>已选择</b><code>candidate_selected</code><code>selection_retracted</code><span class="badge b-indigo">只能看</span><p>选中了一个候选，或撤回了刚选的一段。</p></div>
<div class="ev can"><div class="evn">4</div><b>上屏前</b><code>before_commit</code><span class="badge b-orange">能改</span><p>文字马上要写进去，可以改写（标点除外）。每次都在干净的副本上运行。</p></div>
<div class="ev"><div class="evn">5</div><b>已上屏</b><code>commit</code><span class="badge b-indigo">只能看</span><p>文字已经写进输入框。</p></div>
<div class="ev"><div class="evn">6</div><b>拼写变化</b><code>composition_start</code><code>composition_update</code><code>composition_end</code><span class="badge b-indigo">只能看</span><p>一次拼写开始、变化或结束。</p></div>
<div class="ev"><div class="evn">7</div><b>候选窗口</b><code>candidates_shown</code><code>candidates_hidden</code><span class="badge b-indigo">只能看</span><p>候选窗口出现、换页或收起。</p></div>
</div>
<div class="lc-back">↺ 下一次操作，从第 1 步再来</div>
</div>
<div class="lc-end out"><span class="lc-ic">←</span><div><b>离开输入框</b><code>focus_out</code><span>这个事件送达之后，所有插件实例立刻丢弃，状态全部清空。</span></div></div>
</div>
<div class="lc-notes">
<div class="card lc-note"><b>不在这条链上的：/ 指令</b><code>command</code><p>你打出某个插件注册的指令名（如 /uuid）时，只叫醒这一个插件，让它给出几行结果。别的插件既收不到，也看不到。</p></div>
<div class="card lc-note"><b>故意不发的</b><p>按键本身、按键的时间、在线候选和 AI 候选回来的时刻都不发给插件，免得被用来推测你的打字节奏和网络情况。</p></div>
<div class="card lc-note"><b>根本不开始的</b><p>密码框、隐私输入框、以管理员身份运行的窗口里，整条链一个事件都不发；正在运行的实例当场丢弃。</p></div>
</div>

<h3 class="sub-h">插件实例的一生</h3>
<p class="intro">上面是事件的生命周期，这里是插件自己的：从装好、你授权，到在每个输入框里出生又被丢弃。实例活不过一个输入框，所以插件没法把一个应用里看到的东西带到另一个应用。</p>
<div class="card svgwrap">@@INSTANCE@@</div>
</section>

<section>
<div class="kicker">安全</div>
<h2>为什么脚本插件没法偷看你打的字</h2>
<p class="intro">脚本插件能做的事情很强，所以每一层都上了锁。下面六条是代码里硬性规定的，不是靠插件作者自觉。</p>
<div class="safe">
<div class="card sf"><div class="ic" style="background:#fff7ed">@@LOCK@@</div><b>关在沙箱里</b><p>插件只能调用输入法给的几个固定函数，没有读写文件、联网、启动程序的能力。</p></div>
<div class="card sf"><div class="ic" style="background:#eef2ff">@@EYE@@</div><b>只看你允许的</b><p>每项权限单独勾选。没给权限的文字根本不会交给插件，它连「有没有被藏起来」都分辨不出。</p></div>
<div class="card sf"><div class="ic" style="background:#f0fdfa">@@KEY@@</div><b>密码框里不运行</b><p>密码框、标为隐私的输入框、以管理员身份运行的窗口里，插件完全不启动。插件也拿不到时钟和按键本身，推不出你的打字节奏。</p></div>
<div class="card sf"><div class="ic" style="background:#f5f3ff">@@RESET@@</div><b>换个输入框就清空</b><p>每次切换输入框，插件的状态全部清空，没法把一个应用里看到的内容带到另一个应用。</p></div>
<div class="card sf"><div class="ic" style="background:#fffbeb">@@TIMER@@</div><b>每次都有时间预算</b><p>每次出手都有固定的计算额度，用完立即停下，卡不住你的输入。</p></div>
<div class="card sf"><div class="ic" style="background:#ecfdf5">@@SHIELD@@</div><b>出错也不影响打字</b><p>插件出了任何问题，它这一次的改动全部作废，你打的字照原样上屏。</p></div>
</div>
<div class="honest"><b>需要知道的一点：</b>授权记录存在你自己的用户目录里，没有加密签名。电脑上以你的身份运行的其他程序，理论上可以伪造一条授权，让某个脚本插件运行。所以请只安装你信得过的脚本插件。授权页上也写着这条提醒。</div>
</section>

<section>
<div class="kicker">出了问题</div>
<h2>插件出错或太慢时会发生什么</h2>
<p class="intro">输入法会自己处理，不用你操心。</p>
<div class="flow">
<div class="card fs ok"><b>正常运行</b><p>插件在授权范围内做事。</p></div>
<div class="card fs warn"><b>偶尔出错</b><p>这一次的改动作废，文字照原样上屏。严重的错误会让插件在这个输入框里先停下，换个输入框后重新开始。</p></div>
<div class="card fs bad"><b>经常出错或太慢</b><p>最近 50 次里失败 3 次，或最近 20 次里有 3 次超过 4 毫秒，就自动停用。</p></div>
<div class="card fs back"><b>恢复</b><p>你在授权页重新调整授权，或重启输入法后，停用的插件可以再次运行。</p></div>
</div>
</section>

<details class="dev card" @@OPEN@@>
<summary>给开发者：模块分层、时序和代码位置<span>点开查看技术细节</span></summary>
<div class="devbody">
<h3>模块分层</h3>
<p class="note">依赖单向：client-services 管文件与规则，script-runtime 管执行，host-api 把两者接到输入会话上；runtime 只认识两个 trait，wasmi 类型不出现在其他 crate 的签名里。</p>
<div class="svgbox">@@ARCH@@</div>
<h3>一次按键的调用时序</h3>
<p class="note">实线是调用，虚线是返回，橙色是脚本链路。顺序见 <code>host-api/src/lib.rs:2855</code> 与 <code>lifecycle.rs</code> 的 <code>after_rewriting</code>。runtime 在 <code>before_commit</code> 之前已按原文记下引擎学习和 AI 上文，改写不回灌。</p>
<div class="svgbox">@@SEQ@@</div>
<h3>能力与 import 函数</h3>
<table>
<tr><th>能力</th><th>能读或能做</th><th>依赖 / 限制</th><th>import</th></tr>
<tr><td>（不需要）</td><td>事件枚举字段、写日志、command 的随机数</td><td></td><td><code>event_scalar</code> <code>log</code> <code>random_fill</code></td></tr>
<tr><td><code>read_composition</code></td><td>预编辑串、读音、这次打的字符、光标</td><td>与 rewrite_commit 互斥</td><td><code>composition_text</code> <code>composition_scalar</code></td></tr>
<tr><td><code>read_commit</code></td><td>选中的候选、上屏文本、可改写段、座位</td><td></td><td><code>commit_text</code> <code>commit_scalar</code></td></tr>
<tr><td><code>read_candidates</code></td><td>候选窗口前 20 行、页码、高亮</td><td></td><td><code>candidate_*</code></td></tr>
<tr><td><code>rewrite_commit</code></td><td>改写上屏的 body</td><td>需 read_commit；只在 before_commit 有效</td><td><code>rewrite_commit</code></td></tr>
<tr><td><code>rerank_candidates</code></td><td>重排候选</td><td>需 read_candidates；只在 candidates 有效</td><td><code>candidates_order</code></td></tr>
<tr><td><code>filter_candidates</code></td><td>降到末尾并隐藏</td><td>同上</td><td><code>candidates_hide</code></td></tr>
<tr><td><code>annotate_candidates</code></td><td>加注释（另外显示，不替换原注释）</td><td>同上</td><td><code>candidates_annotate</code></td></tr>
<tr><td><code>provide_commands</code></td><td>在自己声明的 / 指令里给出候选行</td><td>需声明 command 钩子和 [[script.commands]]</td><td><code>command_text</code> <code>command_row</code></td></tr>
</table>
<h3>配额</h3>
<table>
<tr><th>项</th><th>值</th></tr>
<tr><td>单次钩子 fuel</td><td>candidates 125 000；before_commit、command 各 100 000；其余 25 000（实际可用的是它与本轮剩余配额中较小的那个）</td></tr>
<tr><td>每轮配额</td><td>每个插件 125 000；before_commit、command 各有自己的一份 100 000。一轮 = 宿主交来一个按键到下一个按键</td></tr>
<tr><td>初始化</td><td>每个插件 5 000 000；每次聚焦和每一轮所有插件合计 5 000 000，超出的推迟到下一轮</td></tr>
<tr><td>本轮配额用完</td><td>跳过投递，不算失败；中途被截断时用掉不到本钩子上限一半不算失败，达到一半算 fuel 耗尽</td></tr>
<tr><td>内存</td><td><code>memory_pages</code> 1 到 64 页，实例化时一次预留；进程总账 64 MiB，启用时检查；最多同时启用 8 个</td></tr>
<tr><td>熔断</td><td>按（根目录, id, 哈希）计，跨会话；健康文件只记 id、哈希和类别（invalid / init / failed / slow）；投递时宿主 panic 关闭本进程全部脚本至重启</td></tr>
</table>
<h3>代码地图</h3>
<table>
<tr><th>位置</th><th>职责</th></tr>
<tr><td><code>crates/client-services/src/plugins.rs</code></td><td>PluginKind、scan、load_package、BuiltinDirs、内置 id</td></tr>
<tr><td><code>crates/client-services/src/plugins/import.rs</code></td><td>stage → check_staged → rename；validate 给 msime-pack 用</td></tr>
<tr><td><code>crates/client-services/src/plugins/script_pack.rs</code> · <code>script_grants.rs</code> · <code>script_commands.rs</code></td><td>脚本清单与内容哈希、授权文件读写、/ 指令冲突解析</td></tr>
<tr><td><code>crates/client-services/src/plugins/community.rs</code></td><td>社区分享：确定性打包，下载校验后走 import</td></tr>
<tr><td><code>crates/host-api/src/lifecycle.rs</code></td><td>事件推导（derive_selection / before_commit_event / derive_rest）与内置 FEEDBACK 消费者</td></tr>
<tr><td><code>crates/host-api/src/scripts/set.rs</code> · <code>process.rs</code></td><td>会话级插件集合与投递；进程级模块缓存、熔断、健康文件、panic 开关</td></tr>
<tr><td><code>crates/host-api/src/plugin_tables.rs</code> · <code>key_sound</code></td><td>数据包读进 EngineOptions；音效与音乐</td></tr>
<tr><td><code>crates/input-runtime/src/transform.rs</code></td><td>CandidateTransform、CommandProvider 两个注入点</td></tr>
<tr><td><code>crates/script-runtime/src/</code> abi · module · instance · limits</td><td>ABI v1 契约、编译与校验、实例与 import、各项上限</td></tr>
<tr><td><code>crates/pack-tool</code></td><td>msime-pack validate / hash / run</td></tr>
<tr><td><code>plugins/official-packs.json</code> · <code>plugins/script</code> · <code>docs/script-plugin-template</code></td><td>官方包哈希锁、可用脚本插件源码、作者模板与 SDK</td></tr>
</table>
</div>
</details>
</main>
"""


def page(open_dev=False):
    body = (BODY.replace("@@OVERVIEW@@", overview())
            .replace("@@ARCH@@", architecture())
            .replace("@@SEQ@@", sequence())
            .replace("@@INSTANCE@@", instance_lifecycle())
            .replace("@@LOCK@@", ICON_LOCK).replace("@@EYE@@", ICON_EYE).replace("@@KEY@@", ICON_KEY)
            .replace("@@RESET@@", ICON_RESET).replace("@@TIMER@@", ICON_TIMER).replace("@@SHIELD@@", ICON_SHIELD)
            .replace("@@OPEN@@", "open" if open_dev else ""))
    return ('<!doctype html>\n<html lang="zh-CN">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width,initial-scale=1">\n'
            '<meta name="color-scheme" content="light">\n'
            '<title>水杉输入法：插件是怎么工作的</title>\n'
            f'<style>{CSS}</style>\n</head>\n<body>{body}</body>\n</html>\n')


if __name__ == "__main__":
    # 默认写到仓库的 docs/ 下；给一个路径参数时写到那里，加 --open 时开发者区默认展开，便于截图核对。
    args = [a for a in sys.argv[1:] if a != "--open"]
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out = args[0] if args else os.path.join(root, "docs", "plugin-architecture.html")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write(page(open_dev="--open" in sys.argv[1:]))
