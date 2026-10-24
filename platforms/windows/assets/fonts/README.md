# Windows 7 表情字体

`NotoEmoji.ttf` 是 Google Fonts 的 `NotoEmoji[wght].ttf` 原始文件，未修改字体内容或名称。固定提交、源路径和逐文件 SHA-256 见 `SOURCE.json`，分发许可证见 `OFL.txt`。

此字体提供 TrueType 单色轮廓，用于 Windows 7 缺少表情字形时的进程私有 DirectWrite 字体集合；不向系统安装字体，也不改变用户选定的中文字体。现代系统继续使用原有字体回退与彩色表情。Windows 7 使用字体的默认字重轮廓，不要求可变字体轴支持。

字体、来源锁与许可证一起随 Win7 程序安装在 `server/fonts/`。构建先验证摘要，原生验证检查挥手表情、较新的表情字形与完整 UTF-16 范围，并在目标系统检查实际排版。

Win7 的组合表情另由 HarfBuzz Core 根据此原始字体的 OpenType 规则整形，通过 DirectWrite 内联对象绘制；不改写候选文本，也不维护第二份表情字形表。依赖来源基线与许可证收集见 `../../Notices.md`。
