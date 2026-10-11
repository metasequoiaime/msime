// TIP 和生命周期核心的边界，按源码检查：TIP 被注入每个应用进程，它在进程里的宿主会话只给 Server 转述，从不选择加入生命周期、不报按键、字段属性和激活（msime_client.h 的「Windows TIP 会话的约束」）；这些都由 Server 的会话报告，TIP 只经 Main 管道把字段属性交给 Server。将来的脚本插件因此也永远不会在应用进程里运行。从仓库根目录运行，或者传入 TSF 的源码目录。
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>

namespace {
int failures = 0;

std::string read(const std::filesystem::path &path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.u8string().c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    return text.str();
}

bool identifier_char(char value) {
    return (value >= 'a' && value <= 'z') || (value >= 'A' && value <= 'Z') || (value >= '0' && value <= '9') ||
           value == '_';
}

// 按整个标识符找：msime_client_key 是 msime_client_key_sound、msime_client_key_event 的前缀，那些不算。
bool references(const std::string &text, const std::string &name) {
    for (auto at = text.find(name); at != std::string::npos; at = text.find(name, at + 1)) {
        const bool starts = at == 0 || !identifier_char(text[at - 1]);
        const auto end = at + name.size();
        const bool ends = end == text.size() || !identifier_char(text[end]);
        if (starts && ends)
            return true;
    }
    return false;
}

void expect(const std::string &text, const char *needle, bool present, const char *what) {
    if ((text.find(needle) != std::string::npos) != present) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

// `first` 出现在 `anchor` 之后、`second` 之前。不按行比较，检出成 CRLF 也一样。
void expect_order(const std::string &text, const char *anchor, const char *first, const char *second, const char *what) {
    const auto start = text.find(anchor);
    const auto before = start == std::string::npos ? start : text.find(first, start);
    const auto after = before == std::string::npos ? before : text.find(second, start);
    if (before == std::string::npos || after == std::string::npos || before > after) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main(int argc, char **argv) {
    const std::filesystem::path root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const char *forbidden[] = {"msime_client_set_lifecycle", "msime_client_key", "msime_client_set_field_traits",
                               "msime_client_set_active"};
    size_t scanned = 0;
    for (const auto &entry : std::filesystem::recursive_directory_iterator(root)) {
        if (!entry.is_regular_file())
            continue;
        const auto relative = entry.path().lexically_relative(root);
        // 测试自己要写出这几个名字。
        if (!relative.empty() && *relative.begin() == "tests")
            continue;
        const auto extension = entry.path().extension().u8string();
        if (extension != ".cpp" && extension != ".h" && extension != ".hpp" && extension != ".def" && extension != ".rc")
            continue;
        ++scanned;
        const auto text = read(entry.path());
        for (const char *name : forbidden) {
            if (references(text, name)) {
                std::fprintf(stderr, "FAIL: %s references %s; only the Server's session joins the lifecycle core\n",
                             relative.u8string().c_str(), name);
                ++failures;
            }
        }
    }
    if (scanned < 50) {
        std::fprintf(stderr, "FAIL: only %zu TSF sources found under %s\n", scanned, root.u8string().c_str());
        ++failures;
    }

    // 字段属性经 Main 管道交给 Server：每个键之前、每次状态快照之前记下，协商到能力位才发。
    const auto sink = read(root / "Key/KeyEventSink.cpp");
    expect_order(sink, "PerfTimer writeShmTimer;", "_NoteFieldTraitsForServer();", "SendKeyEventToUIProcess(&requestId);",
                 "the field traits are noted before each key is sent to the Server");
    expect(sink, "FanyImeFieldTraits::Observe(_IsKeyboardDisabled() != FALSE, _IsSecureMode() != FALSE,", true,
           "a disabled keyboard, an empty context and the secure desktop are reported as SECURE");
    expect(sink, "GetTokenInformation(token, TokenElevation,", true, "an elevated client process is reported as ELEVATED");
    expect(sink, "FanyImeFieldTraits::ProcessElevated(queried, tokenElevated)", true,
           "a process whose token cannot be queried is reported as ELEVATED");
    const auto ime = read(root / "IME/MetasequoiaIME.cpp");
    expect_order(ime, "void SendCurrentImeStatusSnapshot(", "pIME->_NoteFieldTraitsForServer();",
                 "SendIMEStatusSnapshotToUIProcessViaNamedPipe(",
                 "a status snapshot carries the field traits, so entering a password field stops the music without a key");
    const auto ipc = read(root / "IPC/Ipc.cpp");
    expect(ipc, "fieldTraitsObserved && SupportsFieldTraits() &&", true,
           "FieldTraits is sent only once negotiated: an old Server drops the connection on an unknown event");
    expect(ipc, "FanyImeProtocol::GameHostCandidate | FanyImeProtocol::FieldTraits", true,
           "the Main hello announces FieldTraits");
    expect(ipc, "fieldTraitsDelivered = false;", true, "a new Main connection reports the traits again");
    expect_order(ipc, "fieldTraitsObserved && SupportsFieldTraits() &&",
                 "packet.event_type == FanyImePipeEventType::ClientActivated ||",
                 "traits.event_type = FanyImePipeEventType::FieldTraits;",
                 "the field traits precede ClientActivated, so the Server prepares the session with this field's traits");
    // 补发键和重连之前可能先发 ClientActivated：激活前先记下属性。
    expect_order(sink, "void CMetasequoiaIME::_DrainOneDeferredKeyDown()", "_NoteFieldTraitsForServer();",
                 "EnsureNamedpipeFocusSessionActivated()",
                 "the field traits are noted before a deferred key activates the focus session");
    expect_order(ime, "case WM_IpcReconnect: {", "pIME->_NoteFieldTraitsForServer();",
                 "EnsureNamedpipeFocusSessionActivated()",
                 "the field traits are noted before a reconnect activates the focus session");
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}
