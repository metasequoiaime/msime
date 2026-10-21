#pragma once

namespace msime::candidate
{
enum class CandidateWheelAction
{
    None,
    PreviousPage,
    NextPage,
};

constexpr CandidateWheelAction CandidateWheelPageAction(
    double deltaY, bool enabled, bool hasPreviousPage, bool hasNextPage)
{
    if (!enabled || deltaY == 0.0)
        return CandidateWheelAction::None;
    if (deltaY > 0.0 && hasPreviousPage)
        return CandidateWheelAction::PreviousPage;
    if (deltaY < 0.0 && hasNextPage)
        return CandidateWheelAction::NextPage;
    return CandidateWheelAction::None;
}

// Precise (trackpad / Magic Mouse) scroll distance in points that counts as one page. Precise events arrive every frame with deltas of a few points, so paging on each of them turned one swipe into many pages. WebKit maps one line-based wheel tick to 40 px (Scrollbar::pixelsPerLineStep), so this is the distance one classic wheel notch would scroll, and one short deliberate swipe pages about once. It plays the role of WHEEL_DELTA in the Windows ConsumeWheelDelta.
inline constexpr double CandidateWheelPreciseNotch = 40.0;

// Folds one scroll event into `accumulator` and returns the whole pages it completes: positive pages back (deltaY > 0), negative pages forward. Momentum (inertia) events never page and drop the remainder; a gesture begin starts from zero; a direction reversal drops the old remainder so it cannot eat into the first notch of the new direction. A classic (non-precise) wheel event is one notch and pages exactly once, whatever line count acceleration reports for it.
constexpr int ConsumeCandidateWheelDelta(
    double &accumulator, double deltaY, bool precise, bool gestureBegan, bool momentum,
    double notch = CandidateWheelPreciseNotch)
{
    if (momentum || notch <= 0.0)
    {
        accumulator = 0.0;
        return 0;
    }
    if (gestureBegan)
        accumulator = 0.0;
    if (deltaY == 0.0)
        return 0;
    if (!precise)
    {
        accumulator = 0.0;
        return deltaY > 0.0 ? 1 : -1;
    }
    if ((accumulator > 0.0 && deltaY < 0.0) || (accumulator < 0.0 && deltaY > 0.0))
        accumulator = 0.0;
    accumulator += deltaY;
    int steps = 0;
    while (accumulator >= notch)
    {
        accumulator -= notch;
        ++steps;
    }
    while (accumulator <= -notch)
    {
        accumulator += notch;
        --steps;
    }
    return steps;
}
} // namespace msime::candidate
