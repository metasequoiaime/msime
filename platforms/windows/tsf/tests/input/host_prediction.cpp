// 上屏后联想在 TIP 这一侧的规则（HostPrediction.h）：联想不算组字；按键时 Esc 只关联想、其他键先关联想再照常处理、单按修饰键不动；编辑会话结束时按宿主会话的代次决定显示、保留或关掉。
#include "../../HostCharacterResult.h"
#include "../../HostPrediction.h"
#include <cstdlib>
#include <iostream>
#include <string>
#include <vector>

using namespace msime::tsf;
namespace {
int failures = 0;
void require(bool value, int line) {
    if (!value) {
        std::cerr << "host_prediction failed at line " << line << '\n';
        ++failures;
    }
}
#define REQUIRE(value) require((value), __LINE__)
// 选中「吃」之后宿主会话的联想：没有组字，两行候选，代次 9。
EngineView prediction(uint64_t generation = 9) {
    EngineView view;
    view.prediction = true;
    view.generation = generation;
    view.candidates.push_back({"{}", "饭", true, 0, 0});
    view.candidates.push_back({"{}", "东西", false, 1, 0});
    return view;
}
} // namespace
int main() {
    // 联想不是组字；普通候选、拼音、暂存词段都是。
    REQUIRE(!HostViewComposes(prediction()));
    REQUIRE(HostShowsPrediction(prediction()));
    EngineView ordinary = prediction();
    ordinary.prediction = false;
    REQUIRE(HostViewComposes(ordinary) && !HostShowsPrediction(ordinary));
    EngineView typing = prediction();
    typing.editing_text = "fan";
    REQUIRE(HostViewComposes(typing) && !HostShowsPrediction(typing));
    EngineView held = prediction();
    held.phrase_prefix = "吃";
    REQUIRE(HostViewComposes(held) && !HostShowsPrediction(held));
    EngineView empty;
    empty.prediction = true;
    REQUIRE(!HostViewComposes(empty) && !HostShowsPrediction(empty));

    // 上屏后带出联想的选词：上屏、结束组字，不开新的组字（联想由 _RefreshPrediction 画在光标处）。
    EngineResult selected;
    selected.handled = true;
    selected.has_commit = true;
    selected.commit = "吃";
    selected.view = prediction();
    std::vector<std::string> events;
    const auto status = ApplyHostCharacterResult(selected,
        [&](const std::string &text) { events.push_back(text); return true; },
        [&] { events.push_back("cleanup"); return true; },
        [&] { events.push_back("refresh"); return true; });
    REQUIRE(status == CharacterResultStatus::Applied);
    REQUIRE((events == std::vector<std::string>{"吃", "cleanup"}));
    // 联想时宿主不收的可打印字符不能被改写成上屏：那是联想清掉之后的事，交给引擎的答复。
    EngineResult declined;
    declined.view = prediction();
    EngineResult out;
    REQUIRE(!DeclinedIdleCharacterCommit(declined, 'A', &out));

    // 按键：联想没显示时一律照常。
    REQUIRE(PredictionKey(false, 0x1B, 0) == PredictionKeyAction::None);
    REQUIRE(PredictionKey(false, '1', 0) == PredictionKeyAction::None);
    // Esc 关掉联想并吃掉；带修饰键的 Esc 属于应用，联想照样关掉。
    REQUIRE(PredictionKey(true, 0x1B, 0) == PredictionKeyAction::Close);
    REQUIRE(PredictionKey(true, 0x1B, 0b010) == PredictionKeyAction::Dismiss);
    REQUIRE(PredictionKey(true, 0x1B, 0b001) == PredictionKeyAction::Dismiss);
    // 空格、回车、数字、标点、字母、方向键：先关掉联想，键照常处理（交给应用或开始组字）。
    for (unsigned key : {0x20u, 0x0Du, unsigned('1'), unsigned('9'), 0x60u, 0xBCu, 0xBEu, unsigned('A'), 0x25u, 0x08u, 0x14u})
        REQUIRE(PredictionKey(true, key, 0) == PredictionKeyAction::Dismiss);
    // 单按修饰键不动联想：Shift+字母的大写字母、Ctrl 组合键等那个键按下时再算。
    for (unsigned key : {0x10u, 0x11u, 0x12u, 0x5Bu, 0x5Cu, 0xA0u, 0xA1u, 0xA2u, 0xA3u, 0xA4u, 0xA5u})
        REQUIRE(PredictionKey(true, key, 0b001) == PredictionKeyAction::None);

    // 编辑会话结束：宿主会话出了联想就显示。
    REQUIRE(DecidePredictionRefresh(0, false, true, prediction()) == PredictionRefresh::Show);
    // 显示着的就是这一代：保留，不重新定位。
    REQUIRE(DecidePredictionRefresh(9, false, true, prediction()) == PredictionRefresh::Keep);
    // 点选联想后出了下一代：在新光标处重新显示。
    REQUIRE(DecidePredictionRefresh(9, false, true, prediction(10)) == PredictionRefresh::Show);
    // 联想没了：关掉显示着的，没有显示的什么也不做。
    REQUIRE(DecidePredictionRefresh(9, false, true, EngineView{}) == PredictionRefresh::Hide);
    REQUIRE(DecidePredictionRefresh(0, false, true, EngineView{}) == PredictionRefresh::None);
    // 组字已经开始、或者宿主自己画候选（UI-less）、或者 Server 连不上：不显示。
    REQUIRE(DecidePredictionRefresh(0, true, true, prediction()) == PredictionRefresh::None);
    REQUIRE(DecidePredictionRefresh(9, true, true, prediction()) == PredictionRefresh::Hide);
    REQUIRE(DecidePredictionRefresh(0, false, false, prediction()) == PredictionRefresh::None);
    REQUIRE(DecidePredictionRefresh(9, false, false, prediction()) == PredictionRefresh::Hide);
    // 普通组字的候选不是联想。
    REQUIRE(DecidePredictionRefresh(0, false, true, ordinary) == PredictionRefresh::None);
    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}
