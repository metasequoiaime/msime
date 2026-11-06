// 延迟键屏障里的智能标点：策略逐条钉住，TSF 里的接线按源码检查。
//
// 屏障（Server 回得慢、偏好事务在排队）会把没有功能的可打印键当应用文本排队，由 TIP 自己
// 按顺序插进文档。两件事只能靠这里的断言守住：那样插进去的字符要算「用户刚打的字」（否则
// 数字后直出失去依据，QQ 这类读不回前文的宿主就时好时坏），以及空格转换/重复标点转中文
// 必须先于「可打印键排队」被认领（否则空格被当普通空格插入，刚打出的中文标点换不回来）。
#include "SmartPunctuationDeferralPolicy.h"

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

namespace
{
int failures = 0;

void check(bool condition, const char *what)
{
    if (!condition)
    {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

std::string read(const std::string &path)
{
    std::ifstream file(path, std::ios::binary);
    if (!file)
    {
        std::fprintf(stderr, "FAIL: cannot open %s\n", path.c_str());
        std::exit(EXIT_FAILURE);
    }
    std::ostringstream text;
    text << file.rdbuf();
    return text.str();
}
} // namespace

int main(int argc, char **argv)
{
    using Global::DeferredKeyIsApplicationText;
    using Global::DeferredSmartPunctuationClaim;
    using Global::DeferredSmartPunctuationClaimFor;
    using Global::SmartPunctuationArmStillBelongsToDocument;

    // 1. 「没有功能」= 组字和命令都不是，只是要打的字。队列按它分应用文本，智能标点按它
    //    决定要不要把字符记进前文影子；两条路径共用这一条规则。
    check(DeferredKeyIsApplicationText(false, false), "没有 Category 也没有 Function：应用文本");
    check(!DeferredKeyIsApplicationText(true, false), "有 Category：组字，不是应用文本");
    check(!DeferredKeyIsApplicationText(false, true), "有 Function：命令，不是应用文本");
    check(!DeferredKeyIsApplicationText(true, true), "两者都有：不是应用文本");

    // 2. 两条本地命令在屏障里也要先认领，且各自只认自己的键。
    check(DeferredSmartPunctuationClaimFor(true, true, false) == DeferredSmartPunctuationClaim::SpaceConvert,
          "空格 + 已武装 = 空格转换");
    check(DeferredSmartPunctuationClaimFor(false, false, true) == DeferredSmartPunctuationClaim::Revert,
          "同一按键的第二次 = 重复标点转中文");
    check(DeferredSmartPunctuationClaimFor(true, false, false) == DeferredSmartPunctuationClaim::None,
          "没有武装时空格照常交给宿主");
    check(DeferredSmartPunctuationClaimFor(false, true, false) == DeferredSmartPunctuationClaim::None,
          "空格转换只认空格键");

    // 3. 已武装的空格转换只在「真的回不来」时才丢，没有时限（其他宿主也没有）。
    check(SmartPunctuationArmStillBelongsToDocument(7, 7, true), "同一篇文档、同一窗口");
    check(!SmartPunctuationArmStillBelongsToDocument(7, 8, true), "焦点会话换了");
    check(!SmartPunctuationArmStillBelongsToDocument(7, 7, false), "前台窗口换了");
    check(!SmartPunctuationArmStillBelongsToDocument(0, 0, true), "没有焦点令牌就等于没有文档");
    // 会话还在协商（令牌不变、窗口不变）不属于上面的任何一种：那正是丢掉手势的原因。
    check(SmartPunctuationArmStillBelongsToDocument(7, 7, true), "会话暂时没就绪不算换了文档");

    // 4. 接线。屏障里认领两条命令（含排空重分类）由 scripts/test-windows-deferred-classification.py
    //    直接编译 `_ClassifyDeferredKeyDown` 验证（那里断言的是行为），这里只钉住它看不到的部分。
    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const std::string sink = read(root + "/Key/KeyEventSink.cpp");
    // 队列和智能标点共用同一条「应用文本」规则，不能再各写一份。
    check(sink.find("keyState.Category == CATEGORY_NONE && keyState.Function == FUNCTION_NONE") == std::string::npos,
          "应用文本的判定只写一份");
    check(sink.find("Global::DeferredKeyIsApplicationText(") != std::string::npos, "队列按这条规则分应用文本");
    // 认领在「可打印键排队」之外，回放重分类（replayedKey 非空）才走得到。
    check(sink.find("if (!replayedKey && (_deferredKeyClassificationPending") != std::string::npos &&
              sink.find("composingBeforeKey") != std::string::npos,
          "认领对所有屏障按键生效，应用文本排队只限首次分类");

    // 以下几条是抽取式用例（scripts/test-windows-deferred-classification.py）执行不到的接线：
    // 影子在应用文本真正写进文档时记、读不回前文时的三条退路，以及重复标点武装过期后的退路。
    const std::string composition = read(root + "/Composition/Composition.cpp");
    check(composition.find("Global::SmartPunctuationArmStillBelongsToDocument(") != std::string::npos,
          "武装的存活判断走同一条规则");
    check(composition.find("_smartPunctuationSpaceInterrupted = true;") != std::string::npos,
          "标点之后按过别的键要记下来");
    check(composition.find("IsSmartPunctuationModifierKey(code)") != std::string::npos, "修饰键不算打断");
    check(composition.find("if (code == VK_BACK)") != std::string::npos, "退格有专门的作废分支");
    check(composition.find("_ClearSmartPunctuationSpace();") != std::string::npos, "作废时清掉武装");
    check(composition.find("if (preceding == 0 && interrupted)") != std::string::npos,
          "读不回文档又被打断时只插入普通空格");
    check(composition.find("if (preceding == 0 && _QueueSmartPunctuationRewrite(ascii))") != std::string::npos,
          "读不回文档且没被打断时才盲删一个字符");
    // 过期之后的结果（和普通标点路径一致、前文影子更新）由 scripts/test-windows-smart-punctuation-revert.py 编译真实函数执行。
    check(composition.find("return _HandleCompositionPunctuation(ec, pContext, code, wch, FANY_IME_NO_REQUEST_ID,") !=
              std::string::npos,
          "重复标点的武装在排队期间过期时按普通标点处理，不丢这颗键");
    check(read(root + "/IME/MetasequoiaIME.cpp").find("_NoteSmartPunctuationDocumentText(_wch)") != std::string::npos,
          "排队等回放的应用文本写进文档时才记前文");

    if (failures != 0)
    {
        std::fprintf(stderr, "%d failure(s)\n", failures);
        return EXIT_FAILURE;
    }
    std::puts("延迟键屏障里的智能标点：策略与接线回归通过");
    return EXIT_SUCCESS;
}
