#include "../../KeyCompositionState.h"
#include "../../Global/LocalModeKeyPolicy.h"
#include <cstdio>
#include <string>

namespace {
int failures = 0;
void check(bool condition, const char *message) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", message);
        ++failures;
    }
}

struct Host {
    bool active = true;
    bool readable = true;
    std::string reply;
    bool valid() const { return active; }
    bool view(std::string *raw, std::string *) const {
        *raw = reply;
        return readable;
    }
};

std::wstring toWide(const std::string &text) {
    // editing_text 按当前引擎契约为 ASCII。
    return std::wstring(text.begin(), text.end());
}

Host hostView(const std::string &editing, std::size_t caret, const std::string &mode) {
    return {true, true, R"({"ok":true,"value":{"view":{"editing_text":")" + editing +
        R"(","preedit":"显示文本不应参与按键分类","caret_position":)" + std::to_string(caret) +
        R"(,"local_mode":")" + mode + R"("}}})"};
}
} // namespace

int main() {
    using namespace msime::tsf;
    // 点选网址开头、顶级域名后没有上屏；空旧缓冲必须被新预编辑和模式一起替换。
    for (const auto &text : {std::string("https://"), std::string("https://example.com")}) {
        auto host = hostView(text, text.size(), "url");
        const auto state = ReadKeyCompositionState(&host, {}, toWide);
        check(state.rawInput == toWide(text), "URL continuation replaces the empty legacy text");
        check(state.caret == text.size(), "URL continuation restores its caret");
        check(state.urlMode && !state.unicodeMode, "URL continuation uses the host local mode");
        for (const auto key : {L'.', L'3', L'/', L'-'}) {
            const bool input = !state.rawInput.empty() && state.urlMode &&
                Global::ClassifyModeKey(Global::UrlSpellingSymbols, key == L'.' ? 0xBE : key, key) ==
                    Global::ExpressionKey::Input;
            check(input, "continued URL punctuation and digits remain input");
        }
    }
    // 尚未进入网址模式时，触发键也必须检查宿主原文和光标。
    {
        auto host = hostView("www", 3, "none");
        auto state = ReadKeyCompositionState(&host, {}, toWide);
        check(Global::OpensUrlMode(state.rawInput.c_str(), state.rawInput.size(), state.caret, L'.', true, false),
              "typed host prefix accepts its URL trigger");
        host = hostView("www", 2, "none");
        state = ReadKeyCompositionState(&host, {}, toWide);
        check(!Global::OpensUrlMode(state.rawInput.c_str(), state.rawInput.size(), state.caret, L'.', true, false),
              "caret inside the host prefix does not open URL mode");
    }
    {
        Host host{true, true, R"({"ok":true,"value":{"view":{"editing_text":"","preedit":"","phrase_prefix":"合成词","caret_position":0}}})"};
        const auto state = ReadKeyCompositionState(&host, {}, toWide);
        check(state.phraseActive && state.rawInput.empty() && state.caret == 0,
              "prefix-only composition stays active without invented spelling");
    }
    const KeyCompositionState stale{L"U123", 99, true, true};
    // 删除触发键、取消和提交后，旧模式不能污染新视图。
    for (const auto &text : {std::string("www"), std::string{}}) {
        auto host = hostView(text, text.size(), "none");
        const auto state = ReadKeyCompositionState(&host, stale, toWide);
        check(state.rawInput == toWide(text) && state.caret == text.size() && !state.urlMode && !state.unicodeMode,
              "ordinary or empty host view clears stale text and modes");
    }
    {
        auto host = hostView("U4e2d", 3, "unicode");
        const auto state = ReadKeyCompositionState(&host, {}, toWide);
        check(state.unicodeMode && !state.urlMode && state.caret == 3, "host unicode mode and caret are authoritative");
    }
    {
        auto host = hostView("www.", 99, "url");
        const auto state = ReadKeyCompositionState(&host, {}, toWide);
        check(state.caret == 4, "host caret is bounded by the current text");
    }
    // 不存在宿主会话的旧路径保留原文和模式，只钳制越界光标。
    {
        Host host;
        host.active = false;
        const auto state = ReadKeyCompositionState(&host, stale, toWide);
        check(state.rawInput == stale.rawInput && state.caret == 4 && state.urlMode && state.unicodeMode,
              "legacy profile retains its state");
        const auto absent = ReadKeyCompositionState<Host>(nullptr, stale, toWide);
        check(absent.rawInput == stale.rawInput && absent.caret == 4, "missing host retains legacy state");
    }
    // 已初始化宿主读失败时不可退回另一份旧会话。
    for (const bool readable : {false, true}) {
        Host host{true, readable, "invalid"};
        const auto state = ReadKeyCompositionState(&host, stale, toWide);
        check(state.rawInput.empty() && state.caret == 0 && !state.urlMode && !state.unicodeMode,
              "failed host view does not resurrect stale composition");
    }
    // 回车观测解析真实宿主 JSON，不能拿显示文字或旧缓冲代替原文与词段。
    {
        Host host{true, true, R"({"ok":true,"value":{"view":{"editing_text":"ab","preedit":"display","phrase_prefix":"synthetic-"}}})"};
        check(ReadLocalCommitObservation(&host, L"stale", toWide) == L"synthetic-ab",
              "Enter observation uses the host raw text and prefix");
        host.reply = R"({"ok":true,"value":{"view":{"editing_text":"","phrase_prefix":"synthetic-prefix"}}})";
        check(ReadLocalCommitObservation(&host, L"stale", toWide) == L"synthetic-prefix",
              "prefix-only Enter observation comes from the host");
        host.readable = false;
        check(ReadLocalCommitObservation(&host, L"stale", toWide).empty(),
              "unreadable host cannot supply stale Enter text");
        host.readable = true;
        host.reply = "invalid";
        check(ReadLocalCommitObservation(&host, L"stale", toWide).empty(),
              "invalid host JSON cannot supply stale Enter text");
        host.active = false;
        check(ReadLocalCommitObservation(&host, L"legacy", toWide) == L"legacy",
              "inactive host keeps the legacy Enter observation");
        check(ReadLocalCommitObservation<Host>(nullptr, L"legacy", toWide) == L"legacy",
              "missing host keeps the legacy Enter observation");
    }
    std::printf("key composition state: %d failures\n", failures);
    return failures ? 1 : 0;
}
