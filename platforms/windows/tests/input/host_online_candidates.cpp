#include "ipc/HostCandidateRelay.h"
#include "../../tsf/EngineSessionAdapter.h"
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <filesystem>

using namespace msime::windows;
using nlohmann::json;
namespace {
void require(bool value, const char *message) {
    if (!value) { std::fprintf(stderr, "FAIL: %s\n", message); std::exit(EXIT_FAILURE); }
}
std::string assemble(const std::string &payload) {
    const auto frames = FanyImeCandidateContinuationPipe::Encode(payload, 17,
        FanyImeWorkerReplyType::HostOnlineCandidates, HostOnlinePayloadLimit);
    require(frames.has_value(), "bounded operation encoded");
    HostOnlineAssembler assembler;
    std::optional<FanyImeCandidateContinuationPipe::Completed> completed;
    for (const auto &frame : *frames) completed = assembler.accept(frame.data, std::size(frame.data), 7, 8);
    require(completed && completed->text == payload, "operation crosses fixed worker frames without truncation");
    if (frames->size() > 1) {
        assembler.accept(frames->front().data, FanyImePipeLimits::CandidateTextCapacity, 7, 8);
        require(!assembler.accept(frames->at(1).data, FanyImePipeLimits::CandidateTextCapacity, 9, 8) &&
                !assembler.active(), "focus change rejects unfinished operation");
        require(!assembler.accept(frames->back().data, FanyImePipeLimits::CandidateTextCapacity, 7, 8),
                "missing prefix never applies");
    }
    return completed->text;
}
}
int main(int argc, char **argv) {
    require(argc == 2, "synthetic dictionary fixture supplied");
    const auto root = std::filesystem::temp_directory_path() /
        ("msime-host-online-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    std::filesystem::create_directories(root);
    struct Cleanup { std::filesystem::path root;
        ~Cleanup() { std::error_code error; std::filesystem::remove_all(root, error); }
    } cleanup{root};
    json options{{"api_version", 1}, {"preferences", {{"scheme", "quanpin"}, {"learning", false},
        {"cloud_candidates", true}, {"ai_assistant", {{"enabled", true},
            {"endpoint", "https://synthetic.invalid/v1/chat/completions"},
            {"tokens", {{"https://synthetic.invalid:443", "synthetic-test-token"}}}}}, {"candidate_page_size", 9}, {"chinese_punctuation", true}}}};
    for (const char *name : {"resources", "user_data", "cache", "dictionaries"}) {
        std::filesystem::create_directories(root / name); options[name] = (root / name).u8string();
    }
    for (const char *name : {"resources", "dictionaries"})
        std::filesystem::copy_file(argv[1], root / name / "msime-pinyin.db");
    ServerSession server(42, options.dump()); server.activate(1);
    msime::tsf::EngineSessionAdapter tip;
    std::string raw, error;
    require(tip.create(options.dump(), &error) && tip.focus(true, &raw, &error), "independent TIP session created");
    uint64_t request = 0;
    const auto type = [&] {
        for (const auto ch : std::string("nihao")) {
            FanyImeNamedpipeData key{}; key.client_id = 42; key.request_id = ++request;
            key.keycode = ch - 'a' + 'A'; key.wch = ch;
            server.key(key, 1);
            require(tip.character(ch, false, &raw, &error), "local input accepted");
            tip.capture_online_request(request);
        }
    };
    type();
    require(tip.online_query(&raw, &error), "TIP query captured");
    const auto localQuery = json::parse(raw).at("value");
    const auto query = server.online_query(1);
    require(query && json::parse(*query).at("session_id") != localQuery.at("session_id"), "two engines have distinct query identities");
    FanyImeNamedpipeData packet{}; packet.modifiers_down = FanyImePipeFlags::UiLess;
    PendingReply reply{}; reply.source.request_id = request; reply.source.transition = {{"view", server.view()}};
    reply.online_query = *query;
    PipeTicket ticket{}; ticket.client = 42; ticket.capabilities = FanyImeProtocol::HostOnlineCandidates;
    FocusLease lease{ticket, 1, 7};
    HostCandidateRelay relay;
    relay.delivered(lease, reply, packet);
    const json answers = json::array({"你好", "拟好", "逆好"});
    const auto operation = relay.update(lease, 1, *query, answers);
    require(operation.has_value(), "local UI-less operation bound to delivered request");
    require(server.apply_ai_candidates(1, *query, answers.dump()).has_value(), "Server applies synthetic AI answer");
    require(tip.apply_online_update(assemble(*operation)), "TIP applies answer to original local query");
    require(tip.view(&raw, &error), "updated TIP view available");
    auto view = json::parse(raw).at("value");
    require(!view.at("candidates").empty() && view.at("session") != server.view().at("session"),
            "updated candidates retain local session");
    for (const auto &item : view.at("candidates"))
        require(item.at("id").at("session") == view.at("session") &&
                item.at("id").at("generation") == view.at("generation"), "all candidate identities belong to local view");
    const std::string body = R"(["SUCCESS", [["nihao", ["呢好"]]]])";
    const auto cloud = relay.update(lease, 0, *query, body);
    require(cloud && tip.apply_online_update(assemble(*cloud)), "cloud follows AI without borrowing Server session identity");
    // 256 KiB 上限内的大响应不得截成一个 199 字符页面；ASCII 转义后也能完整重组。
    assemble(relay.update(lease, 0, *query, body + std::string(100000, ' ')).value());
    require(!relay.update(lease, 1, "{}", answers), "unbound query rejected");
    auto stale = lease; ++stale.token;
    require(!relay.update(stale, 1, *query, answers), "stale focus rejected");
    stale = lease; ++stale.transport.client;
    require(!relay.update(stale, 1, *query, answers), "foreign transport rejected");
    auto changed = json::parse(*operation); changed["request"] = request + 1;
    require(!tip.apply_online_update(changed.dump()), "foreign request rejected");
    changed = json::parse(*operation); changed["query"]["identity"] = "unrelated";
    require(!tip.apply_online_update(changed.dump()), "different query semantics rejected");
    require(!tip.apply_online_update("{") && !tip.apply_online_update("{}"), "malformed operations rejected");
    require(tip.view(&raw, &error), "navigation starts from local view");
    const auto firstPage = json::parse(raw).at("value").at("candidates");
    require(firstPage.size() == 9, "synthetic dictionary fills a local page");
    const auto highlighted = [](const json &rows) {
        for (const auto &row : rows) if (row.at("highlighted") == true) return row.at("text").get<std::string>();
        return std::string{};
    };
    require(tip.command(MSIME_NEXT_CANDIDATE, &raw, &error), "local highlight advances");
    require(highlighted(json::parse(raw).at("value").at("view").at("candidates")) != highlighted(firstPage),
        "local navigation changes actual engine highlight");
    require(tip.command(MSIME_PREVIOUS_CANDIDATE, &raw, &error), "local highlight returns");
    require(tip.command(MSIME_NEXT_PAGE, &raw, &error), "local next page");
    require(json::parse(raw).at("value").at("view").at("candidates").at(0).at("text") != firstPage.at(0).at("text"),
        "local navigation changes actual engine page");
    require(tip.command(MSIME_PREVIOUS_PAGE, &raw, &error), "local page returns");
    require(tip.view(&raw, &error), "selection view available");
    view = json::parse(raw).at("value");
    bool selected = false;
    for (const auto &item : view.at("candidates")) if (item.at("text") == "呢好") {
        const auto id = item.at("id");
        require(tip.select(id.at("generation"), id.at("index"), &raw, &error), "new local candidate selectable");
        require(json::parse(raw).at("value").at("commit") == "呢好", "selection commits displayed candidate");
        selected = true; break;
    }
    require(selected, "cloud candidate displayed");
    // 故意不更新请求号：即使又输入相同拼音，原始查询的引擎代次也必须拒绝旧回答。
    require(tip.command(MSIME_CANCEL, &raw, &error), "reset TIP");
    for (const auto ch : std::string("nihao")) require(tip.character(ch, false, &raw, &error), "retype same input");
    require(!tip.apply_online_update(*operation), "same spelling in new composition rejects old captured query");
    tip.capture_online_request(++request);
    require(!tip.apply_online_update(*operation), "new request rejects old answer");
    require(tip.command(MSIME_CANCEL, &raw, &error), "reset before fresh AI query");
    for (const auto ch : std::string("ni")) require(tip.character(ch, false, &raw, &error), "fresh uncached AI query");
    tip.capture_online_request(++request);
    require(tip.online_query(&raw, &error), "current query available");
    const auto nextQuery = json::parse(raw).at("value");
    require(nextQuery.is_object(), "fresh AI query is an object");
    require(tip.view(&raw, &error), "view before empty AI answer available");
    const auto beforeEmpty = json::parse(raw).at("value");
    changed = {{"version", 1}, {"request", request}, {"operation", 1}, {"query", nextQuery}, {"candidates", json::array()}};
    // 内核不再预留 AI 占位行；空回答与兼容的 resolve 调用都不改视图或代次。
    require(!tip.apply_online_update(changed.dump()), "empty AI answer changes no candidates");
    require(tip.view(&raw, &error), "resolved view available");
    view = json::parse(raw).at("value");
    require(view == beforeEmpty, "empty AI answer preserves the complete local view");
    for (const auto &item : view.at("candidates"))
        require(item.at("source") != 12, "AI no longer reserves placeholder rows");
    packet.modifiers_down = 0; relay.delivered(lease, reply, packet);
    require(!relay.update(lease, 1, *query, answers), "ordinary host receives no relay");
    packet.modifiers_down = FanyImePipeFlags::UiLess | FanyImePipeFlags::ServerUiLessCandidates;
    relay.delivered(lease, reply, packet);
    require(!relay.update(lease, 1, *query, answers), "legacy UI-less host keeps page protocol");
    packet.modifiers_down = FanyImePipeFlags::UiLess;
    auto legacy = lease; legacy.transport.capabilities = 0; relay.delivered(legacy, reply, packet);
    require(!relay.update(legacy, 1, *query, answers), "unnegotiated peer receives no new opcode");
    require(!FanyImeCandidateContinuationPipe::Encode(std::string(HostOnlinePayloadLimit + 1, 'x'), 1,
                FanyImeWorkerReplyType::HostOnlineCandidates, HostOnlinePayloadLimit), "oversized operation rejected");
    return 0;
}
