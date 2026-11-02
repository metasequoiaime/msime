#include "../../QuickPhraseCursorEdit.h"
#include <algorithm>
#include <iostream>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

using msime::tsf::CursorCommitStatus;
namespace {
void require(bool value) { if (!value) throw std::runtime_error("quick phrase cursor edit assertion"); }
struct Editor {
    struct Range { std::size_t start, end; };
    std::u16string document = u"左旧拼写右";
    Range selection{1, 4};
    std::string fail, reenter;
    std::vector<std::string> operations;
    bool valid = true, composition = true, staleOperation = false, rewrite = false;
    unsigned writes = 0, attempts = 0, finishes = 0, clones = 0, collapses = 0, selections = 0;
    std::optional<std::size_t> clamp;
    bool current() const { return valid; }
    bool step(const std::string &name) {
        if (!valid) staleOperation = true;
        operations.push_back(name);
        if (reenter == name) {
            // 新的编辑已经拥有文档与组字；旧调用不能再清理或设置选区。
            valid = false;
            document = u"新的预编辑";
            composition = true;
            selection = {2, 2};
        }
        return fail != name;
    }
    std::optional<Range> replacement_range() {
        const auto range = selection;
        if (!step("range")) return std::nullopt;
        return range;
    }
    bool write(Range &range, std::u16string_view text) {
        ++attempts;
        if (fail == "write") { step("write"); return false; }
        const auto value = rewrite ? std::u16string(u"改写") : std::u16string(text);
        document.replace(range.start, range.end - range.start, value);
        range.end = range.start + value.size();
        ++writes;
        return step("write");
    }
    std::optional<Range> clone(Range &range) {
        if (!step("clone" + std::to_string(++clones))) return std::nullopt;
        return range;
    }
    bool collapse_end(Range &range) {
        range.start = range.end;
        return step("end" + std::to_string(++collapses));
    }
    bool collapse_start(Range &range) { range.end = range.start; return step("start"); }
    bool select(Range &range) {
        require(range.start == range.end);
        const auto name = "select" + std::to_string(++selections);
        if (fail != name) selection = range;
        return step(name);
    }
    bool finish() { ++finishes; composition = false; return step("finish"); }
    std::optional<std::u16string> read(Range &range, std::size_t capacity) {
        require(!composition);
        auto text = document.substr(range.start, (std::min)(capacity, range.end - range.start));
        if (!step("read")) return std::nullopt;
        return text;
    }
    std::optional<std::size_t> shift_start(Range &range, std::size_t units) {
        const auto moved = clamp.value_or(units);
        if (fail != "shift") range.start -= (std::min)(moved, range.start);
        if (!step("shift")) return std::nullopt;
        return moved;
    }
};
CursorCommitStatus apply(Editor &editor, const std::u16string &text, std::size_t suffix) {
    return msime::tsf::CommitQuickPhraseCursor(editor, std::u16string_view(text), suffix);
}
}
int main() {
    try {
        for (const auto &entry : std::vector<std::pair<std::u16string, std::size_t>>{
                 {u"", 0}, {u"正文", 0}, {u"正文", 2}, {u"前🌲é后", 5},
                 {u"前👩‍💻后", 6}, {u"前العربية后", 8}, {u"前\r\n后", 3},
                 {std::u16string(256, u'文'), 128}}) {
            for (bool composition : {false, true}) {
                Editor editor;
                editor.composition = composition;
                const auto &text = entry.first;
                const auto suffix = entry.second;
                require(apply(editor, text, suffix) == CursorCommitStatus::CommittedCurrent);
                require(editor.attempts == 1 && editor.writes == 1 && editor.finishes == 1);
                require(editor.document == u"左" + text + u"右" && !editor.composition);
                require(editor.selection.start == 1 + text.size() - suffix &&
                        editor.selection.start == editor.selection.end);
                editor.document.insert(editor.selection.start, u"x");
                require(editor.document == u"左" + text.substr(0, text.size() - suffix) + u"x" +
                                          text.substr(text.size() - suffix) + u"右");
            }
        }
        for (const auto &failure : std::vector<std::string>{
                 "range", "write", "clone1", "end1", "select1", "finish", "read",
                 "clone2", "end2", "shift", "start", "select2"}) {
            Editor editor;
            editor.fail = failure;
            const auto status = apply(editor, u"前🌲后", 3);
            if (failure == "range" || failure == "write") {
                require(status == CursorCommitStatus::NotWritten && editor.writes == 0);
                require(editor.document == u"左旧拼写右");
            } else {
                require(status != CursorCommitStatus::NotWritten && editor.writes == 1 && editor.attempts == 1);
                require(editor.document == u"左前🌲后右" && !editor.composition);
            }
            require(!editor.staleOperation);
        }
        for (const auto &stage : std::vector<std::string>{
                 "range", "write", "clone1", "end1", "select1", "finish", "read",
                 "clone2", "end2", "shift", "start", "select2"}) {
            Editor editor;
            editor.reenter = stage;
            const auto status = apply(editor, u"前🌲后", 3);
            require(status == (stage == "range" ? CursorCommitStatus::NotWritten : CursorCommitStatus::Committed));
            require(editor.document == u"新的预编辑" && editor.composition &&
                    editor.selection.start == 2 && editor.selection.end == 2);
            require(!editor.staleOperation && editor.attempts <= 1);
            require(editor.operations.back() == stage);
        }
        for (std::size_t moved : {std::size_t{0}, std::size_t{1}, std::size_t{4}}) {
            Editor editor;
            editor.clamp = moved;
            require(apply(editor, u"前🌲后", 3) == CursorCommitStatus::CommittedCurrent);
            require(editor.selection.start == 5 && editor.selections == 1 && editor.writes == 1);
        }
        {
            Editor editor;
            editor.rewrite = true;
            require(apply(editor, u"前🌲后", 3) == CursorCommitStatus::CommittedCurrent);
            require(editor.document == u"左改写右" && editor.selection.start == 3 && editor.selections == 1);
        }
        for (const auto &entry : std::vector<std::pair<std::u16string, std::size_t>>{
                 {u"前🌲后", 2}, {u"前", 2}, {std::u16string{0xd800}, 0},
                 {std::u16string{0xdc00}, 0}, {std::u16string{u'a', 0, u'b'}, 1}}) {
            Editor editor;
            require(apply(editor, entry.first, entry.second) == CursorCommitStatus::NotWritten);
            require(editor.attempts == 0 && editor.operations.empty());
        }
        std::cout << "Quick phrase cursor edit: selection, UTF-16, failure and reentry passed\n";
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
