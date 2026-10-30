#pragma once
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>

namespace msime::tsf {
enum class CursorCommitStatus { NotWritten, Committed, CommittedCurrent };

// 位置按 UTF-16 单元计数，不能落在代理项对内部；空正文仍是显式替换。
template<class Char>
bool ValidCursorCommit(std::basic_string_view<Char> text, std::size_t suffix) {
    if (suffix > text.size()) return false;
    const auto target = text.size() - suffix;
    for (std::size_t i = 0; i < text.size(); ++i) {
        const auto unit = static_cast<std::uint32_t>(text[i]);
        if (unit == 0 || unit > 0xffff) return false;
        if (unit >= 0xd800 && unit <= 0xdbff) {
            if (++i == text.size() || i == target) return false;
            const auto low = static_cast<std::uint32_t>(text[i]);
            if (low < 0xdc00 || low > 0xdfff) return false;
        } else if (unit >= 0xdc00 && unit <= 0xdfff) return false;
    }
    return true;
}

// Editor 的每一步可以重入。成功写入之后只返回已消费，不因清理或定位失败要求调用方重放正文。
// 范围的所有权由 Editor 持有；新预编辑只能在 CommittedCurrent 时继续。
template<class Editor, class Char>
CursorCommitStatus CommitQuickPhraseCursor(Editor &editor, std::basic_string_view<Char> text,
                                           std::size_t suffix) {
    using Status = CursorCommitStatus;
    if (!ValidCursorCommit(text, suffix) || !editor.current()) return Status::NotWritten;
    auto range = editor.replacement_range();
    if (!range || !editor.current()) return Status::NotWritten;
    if (!editor.write(*range, text)) return Status::NotWritten;
    if (!editor.current()) return Status::Committed;

    // 先留下普通上屏的末尾位置。后续读回或范围移动被编辑器拒绝时，正文仍已交付。
    bool atEnd = false;
    auto caret = editor.clone(*range);
    if (!editor.current()) return Status::Committed;
    if (caret && editor.collapse_end(*caret)) {
        if (!editor.current()) return Status::Committed;
        atEnd = editor.select(*caret);
    }
    if (!editor.current()) return Status::Committed;
    if (!editor.finish() || !editor.current()) return Status::Committed;
    if (!atEnd) return Status::Committed;

    const auto observed = editor.read(*range, text.size() + 1);
    if (!editor.current()) return Status::Committed;
    if (!observed || std::basic_string_view<Char>(*observed) != text) return Status::CommittedCurrent;
    if (suffix == 0) return Status::CommittedCurrent;
    caret = editor.clone(*range);
    if (!editor.current()) return Status::Committed;
    if (!caret || !editor.collapse_end(*caret))
        return editor.current() ? Status::CommittedCurrent : Status::Committed;
    if (!editor.current()) return Status::Committed;
    const auto moved = editor.shift_start(*caret, suffix);
    if (!editor.current()) return Status::Committed;
    if (!moved || *moved != suffix) return Status::CommittedCurrent;
    if (!editor.collapse_start(*caret))
        return editor.current() ? Status::CommittedCurrent : Status::Committed;
    if (!editor.current()) return Status::Committed;
    if (!editor.select(*caret) || !editor.current()) return Status::Committed;
    return Status::CommittedCurrent;
}
} // namespace msime::tsf
