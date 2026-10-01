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

    // Korean and Vietnamese have no Chinese table: punctuation finishes the composition and follows it.
    for (const int other : {scheme::Korean, scheme::Vietnamese}) {
        Host host;
        host.scheme_number = other;
        host.converted = other == scheme::Korean ? "한" : "tiếng";
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
    return EXIT_SUCCESS;
}
