#include "../../Global/CandidateNavigationPolicy.h"

#include <stdexcept>

namespace
{
void require(bool condition, const char *message)
{
    if (!condition) throw std::runtime_error(message);
}
} // 匿名命名空间

int main()
{
    using msime::windows::tsf::CandidateNavigationCommand;
    require(CandidateNavigationCommand(FanyImeReplyType::MoveSelectionPrevious) == MSIME_PREVIOUS_CANDIDATE,
            "previous selection reply must reach the host command");
    require(CandidateNavigationCommand(FanyImeReplyType::MoveSelectionNext) == MSIME_NEXT_CANDIDATE,
            "next selection reply must reach the host command");
    require(CandidateNavigationCommand(FanyImeReplyType::MovePagePrevious) == MSIME_PREVIOUS_PAGE,
            "previous page reply must reach the host command");
    require(CandidateNavigationCommand(FanyImeReplyType::MovePageNext) == MSIME_NEXT_PAGE,
            "next page reply must reach the host command");
    require(CandidateNavigationCommand(FanyImeReplyType::NavigationIgnored) == 0,
            "ignored navigation must not issue a host command");
    return 0;
}
