#include "../../HostComposition.h"
#include <cstdlib>
#include <map>
#include <string>
#include <vector>

namespace {
using msime::tsf::EndHostComposition;
namespace scheme = msime::windows::scheme;

std::string answer(const std::string &commit, const std::string &editing, int scheme_number) {
    const std::string committed = commit.empty() ? "null" : "\"" + commit + "\"";
    return R"({"ok":true,"value":{"handled":true,"commit":)" + committed + R"(,"view":{"preedit":")" + editing +
           R"(","editing_text":")" + editing + R"(","scheme":)" + std::to_string(scheme_number) + "}}}";
}

// A host session holding one composition. FINISH commits what it shows; the Chinese table answers the marks it has with the conversion and the full-width mark; a failing table call answers nothing.
struct Host {
    int scheme_number = scheme::Zhuyin;
    std::string editing = "su3";
    std::string converted = "你";
    std::map<char, std::string> table{{',', "，"}, {'?', "？"}};
    bool punctuation_fails = false;
    std::vector<std::string> calls;

    bool view(std::string *raw, std::string *) const {
        *raw = answer("", editing, scheme_number);
        return true;
    }
    bool command(uint32_t command, std::string *raw, std::string *) {
        if (command != MSIME_FINISH_COMPOSITION) std::abort();
        calls.push_back("finish");
        const std::string commit = editing.empty() ? std::string{} : converted;
        editing.clear();
        *raw = answer(commit, editing, scheme_number);
        return true;
    }
    bool punctuation(uint8_t key, std::string *raw, std::string *) {
        calls.push_back(std::string("punctuation ") + static_cast<char>(key));
        if (punctuation_fails) return false;
        const auto mark = table.find(static_cast<char>(key));
        if (mark == table.end()) {
            *raw = answer("", editing, scheme_number);
            return true;
        }
        const std::string commit = (editing.empty() ? std::string{} : converted) + mark->second;
        editing.clear();
        *raw = answer(commit, editing, scheme_number);
        return true;
    }
};

// A host session whose cancel behaves as the Engine's: a Vietnamese word shows its raw keys on the first one and is discarded by the next; every other scheme discards on the first.
struct EscapeHost {
    int scheme_number = scheme::Vietnamese;
    std::string shown = "tiếng";
    std::string keys = "tieng5";
    bool raw_showing = false;
    unsigned cancels = 0;

    bool view(std::string *raw, std::string *) const {
        *raw = answer("", shown, scheme_number);
        return true;
    }
    bool command(uint32_t command, std::string *raw, std::string *) {
        if (command != MSIME_CANCEL) std::abort();
        ++cancels;
        if ((scheme_number == scheme::Vietnamese || scheme_number == scheme::Tibetan) && !shown.empty() &&
            !raw_showing) {
            raw_showing = true;
            shown = keys;
        } else {
            shown.clear();
            raw_showing = false;
        }
        *raw = answer("", shown, scheme_number);
        return true;
    }
};

bool check(bool condition) {
    if (!condition) std::exit(EXIT_FAILURE);
    return condition;
}
} // namespace

int main() {
    std::string error;

    // Composing 'su3' and Shift+A: the conversion is committed and the letter follows it as itself, and the host session is left empty.
    {
        Host host;
        const auto ended = EndHostComposition(host, scheme::Zhuyin, L'A', &error);
        check(ended.commit == "你" && ended.keyFollows && !ended.hostLetGo);
        check(host.editing.empty() && host.calls == std::vector<std::string>{"finish"});
    }

    // A punctuation key goes through the Chinese table: the mark replaces the key.
    {
        Host host;
        const auto ended = EndHostComposition(host, scheme::Zhuyin, L',', &error);
        check(ended.commit == "你，" && !ended.keyFollows && !ended.hostLetGo);
        check(host.editing.empty() && host.calls == std::vector<std::string>{"punctuation ,"});
    }

    // A table that answers no commit, or fails, falls back to finishing the composition with the key's own character after it.
    for (const bool fails : {false, true}) {
        Host host;
        host.table.clear();
        host.punctuation_fails = fails;
        const auto ended = EndHostComposition(host, scheme::Zhuyin, L'`', &error);
        check(ended.commit == "你" && ended.keyFollows && !ended.hostLetGo);
        check(host.editing.empty() && host.calls == std::vector<std::string>{"punctuation `", "finish"});
    }

    // Space and digits never take the table; Enter has no printable character, so nothing follows the conversion.
    for (const wchar_t key : {L' ', L'7'}) {
        Host host;
        const auto ended = EndHostComposition(host, scheme::Zhuyin, key, &error);
        check(ended.commit == "你" && ended.keyFollows && host.calls == std::vector<std::string>{"finish"});
    }
    {
        Host host;
        const auto ended = EndHostComposition(host, scheme::Zhuyin, L'\r', &error);
        check(ended.commit == "你" && !ended.keyFollows && !ended.hostLetGo);
    }

    // 韩文、越南文和藏文没有中文标点表：标点结束组字并跟在后面。
    for (const int other : {scheme::Korean, scheme::Vietnamese, scheme::Tibetan}) {
        Host host;
        host.scheme_number = other;
        host.converted = other == scheme::Korean ? "한" : other == scheme::Vietnamese ? "tiếng" : "བཀྲ";
        const auto ended = EndHostComposition(host, other, L',', &error);
        check(ended.commit == host.converted && ended.keyFollows && !ended.hostLetGo);
        check(host.calls == std::vector<std::string>{"finish"});
    }

    // A host that already let go commits nothing, so the document's composition is ended first and keeps its text.
    {
        Host host;
        host.editing.clear();
        const auto ended = EndHostComposition(host, scheme::Zhuyin, L'A', &error);
        check(ended.commit.empty() && ended.hostLetGo && ended.keyFollows);
        const auto marked = EndHostComposition(host, scheme::Zhuyin, L'?', &error);
        check(marked.commit == "？" && marked.hostLetGo && !marked.keyFollows);
    }

    // Vietnamese 'tieng5' and Escape: the host session shows 'tieng5' and keeps composing; a second Escape empties it, and the caller discards the composition.
    {
        EscapeHost host;
        check(msime::tsf::RestoreHostRawOnEscape(host, &error));
        check(host.shown == "tieng5" && host.cancels == 1);
        check(!msime::tsf::RestoreHostRawOnEscape(host, &error));
        check(host.shown.empty() && host.cancels == 2);
        // Nothing composing: nothing is sent.
        check(!msime::tsf::RestoreHostRawOnEscape(host, &error) && host.cancels == 2);
    }

    // 藏文 'bkra' 和 Esc：宿主会话显示威利原文 'bkra' 并继续组字；第二次 Esc 清空它。回车结束组字时只上屏藏文，后面不跟任何字符。
    {
        EscapeHost host;
        host.scheme_number = scheme::Tibetan;
        host.shown = "བཀྲ";
        host.keys = "bkra";
        check(msime::tsf::RestoreHostRawOnEscape(host, &error));
        check(host.shown == "bkra" && host.cancels == 1);
        check(!msime::tsf::RestoreHostRawOnEscape(host, &error));
        check(host.shown.empty() && host.cancels == 2);
    }
    {
        Host host;
        host.scheme_number = scheme::Tibetan;
        host.editing = "bkra";
        host.converted = "བཀྲ";
        const auto ended = EndHostComposition(host, scheme::Tibetan, L'\r', &error);
        check(ended.commit == "བཀྲ" && !ended.keyFollows && !ended.hostLetGo);
        check(host.calls == std::vector<std::string>{"finish"});
    }

    // Every other host-composed scheme discards on Escape as before, so the helper leaves its session alone.
    for (const int other : {scheme::Korean, scheme::Zhuyin}) {
        EscapeHost host;
        host.scheme_number = other;
        host.shown = "su3";
        check(!msime::tsf::RestoreHostRawOnEscape(host, &error) && host.cancels == 0 && host.shown == "su3");
    }
    return EXIT_SUCCESS;
}
