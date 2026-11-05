#pragma once
#import <AppKit/AppKit.h>

// Use the same attributed-string measurement as drawing: fallback glyphs can
// exceed the primary font's ascent/descent even at the same point size.
inline CGFloat MSIMECandidateTextHeight(NSString *text, NSFont *font) {
    return ceil(MAX(font.ascender - font.descender + font.leading,
                    [text sizeWithAttributes:@{NSFontAttributeName:font}].height));
}

// 候选窗首行的 NSTextField 标签（页码「1 / 17」、展开面板行号、输入状态、光标后编辑提示）的宽度。cell 绘制时左右各让出约 2pt，frame 只按 sizeWithAttributes 给宽会把最后一个字形截掉一点（#10852）；改用同字体、无边框的 NSTextFieldCell 的 cellSize 量（含内边距；与 labelWithString: 建出的标签 cell 量出的宽度一致，由 candidate-header-label-width 测试对照真实标签核对），排版占位和 frame 用同一个值。
inline CGFloat MSIMECandidateLabelWidth(NSString *text, NSFont *font) {
    if (!text.length) return 0;
    NSTextFieldCell *cell = [[NSTextFieldCell alloc] initTextCell:text];
    cell.font = font;
    return ceil(cell.cellSize.width);
}
