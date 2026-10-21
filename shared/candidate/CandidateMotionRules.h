#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <string_view>

namespace msime::candidate
{
// 候选窗皮肤动效（`[animation]` 的进场、退场、选中预设）和装饰图待机动效（`decoration.motion`）的时长、幅度与曲线。macOS 用 Core Animation 播（platforms/macos/src/candidate/CandidateMotion.mm 的常量取自这里），Linux 的 X11 原生画布没有合成器动画，按这里的采样函数逐帧画；两边因此同一套数值。规则见 .agents/notes/implemented/feature/2026-10-16-macos-candidate-declared-motion.md。

// 收起后这么短时间内再次显示，算同一段连续输入，不播进场。连续打字的平均击键间隔约 240ms（Dhakal 等，CHI 2018，1.36 亿次击键），上屏键到下一个声母之间再多一点停顿；超过 400ms 才算停下来看，候选窗那时重新出现可以有进场。
constexpr double kMotionTypingBurstSeconds = 0.4;
// 错峰的总延迟上限：再长的一页，最后一行也只比第一行晚这么久开始。
constexpr double kMotionStaggerCapSeconds = 0.08;
// 卡片的位移只用于从下方升起和向下退出；窗口正好是卡片大小，位移越大，起止几帧被窗口边缘裁掉的越多。
constexpr double kMotionSlideDistance = 8.0;
constexpr double kMotionRowRevealDistance = 3.0;
// 装饰图待机动效的幅度上限。实际幅度还按图片大小收小，保证最大位移不超过 kDecorationMotionMargin（CandidateDecoration.h）。
constexpr double kMotionFloatDistance = 3.0;
constexpr double kMotionSwayRadians = 3.0 * 3.14159265358979323846 / 180.0;
constexpr double kMotionBreatheScale = 1.03;
constexpr double kMotionBounceHeight = 3.5;
// 进场、退场的缩放起止值和 glow 只暗到的不透明度。
constexpr double kMotionScaleFrom = 0.96;
constexpr double kMotionGlowOpacity = 0.82;

enum class MotionPreset
{
    none,
    fade,
    fade_scale,
    slide_up,
    spring,
    glow,
};

inline MotionPreset MotionPresetNamed(std::string_view name)
{
    if (name == "fade") return MotionPreset::fade;
    if (name == "fade_scale") return MotionPreset::fade_scale;
    if (name == "slide_up") return MotionPreset::slide_up;
    if (name == "spring") return MotionPreset::spring;
    if (name == "glow") return MotionPreset::glow;
    return MotionPreset::none;
}

enum class DecorationMotionKind
{
    none,
    floating,
    sway,
    breathe,
    bounce,
};

inline DecorationMotionKind DecorationMotionNamed(std::string_view name)
{
    if (name == "float" || name == "floating") return DecorationMotionKind::floating;
    if (name == "sway") return DecorationMotionKind::sway;
    if (name == "breathe") return DecorationMotionKind::breathe;
    if (name == "bounce") return DecorationMotionKind::bounce;
    return DecorationMotionKind::none;
}

// 与共享契约一致的 40–600ms。
inline double MotionSeconds(double milliseconds)
{
    return std::clamp(milliseconds / 1000.0, 0.04, 0.6);
}

// 只有 fade_scale 和 slide_up 逐行错峰，与 Web 候选栏相同。
constexpr bool MotionStaggersRows(MotionPreset preset)
{
    return preset == MotionPreset::fade_scale || preset == MotionPreset::slide_up;
}

// 第 index 行错峰开始的延迟（秒）。
inline double MotionRowDelay(std::size_t index, double staggerMs)
{
    return std::min(static_cast<double>(index) * std::max(0.0, staggerMs) / 1000.0, kMotionStaggerCapSeconds);
}

// Core Animation 的标准三次贝塞尔曲线（kCAMediaTimingFunctionEaseIn/EaseOut/EaseInEaseOut），给出进度 t 时的完成比例。
inline double MotionBezier(double x1, double y1, double x2, double y2, double t)
{
    if (!(t > 0.0)) return 0.0;
    if (t >= 1.0) return 1.0;
    const auto curve = [](double a, double b, double u) {
        const double v = 1.0 - u;
        return 3 * v * v * u * a + 3 * v * u * u * b + u * u * u;
    };
    // 曲线的 x 单调，二分求出 x(u) = t 的参数，再取 y(u)。
    double low = 0.0, high = 1.0, u = t;
    for (int step = 0; step < 32; ++step)
    {
        u = (low + high) / 2;
        if (curve(x1, x2, u) < t) low = u;
        else high = u;
    }
    return curve(y1, y2, u);
}
inline double MotionEaseOut(double t) { return MotionBezier(0.0, 0.0, 0.58, 1.0, t); }
inline double MotionEaseIn(double t) { return MotionBezier(0.42, 0.0, 1.0, 1.0, t); }
inline double MotionEaseInOut(double t) { return MotionBezier(0.42, 0.0, 0.58, 1.0, t); }

// 卡片在某一刻的样子：不透明度、以卡片中心为轴的缩放，以及在屏幕上的抬起量（负数向下）。
struct CardMotionFrame
{
    double opacity = 1.0;
    double scale = 1.0;
    double lift = 0.0;
};

inline double MotionMix(double from, double to, double progress)
{
    return from + (to - from) * progress;
}

// 进场第 t（0–1，已按时长归一）时的卡片。
inline CardMotionFrame EnterFrame(MotionPreset preset, double t)
{
    const double eased = MotionEaseOut(t);
    switch (preset)
    {
    case MotionPreset::none:
        return {};
    case MotionPreset::fade:
        return {eased, 1.0, 0.0};
    case MotionPreset::fade_scale:
        return {eased, MotionMix(kMotionScaleFrom, 1.0, eased), 0.0};
    case MotionPreset::slide_up:
        return {eased, 1.0, MotionMix(-kMotionSlideDistance, 0.0, eased)};
    case MotionPreset::spring:
    {
        // 卡片和窗口一样大，越过 1 的部分会被窗口边缘裁掉，所以卡片的回弹只有千分之几；明显的回弹留给选中的那一行。关键帧 0 → 0.7 → 1 各段都是 ease-out。
        const double clamped = std::clamp(t, 0.0, 1.0);
        const double scale = clamped < 0.7 ? MotionMix(kMotionScaleFrom, 1.004, MotionEaseOut(clamped / 0.7))
                                           : MotionMix(1.004, 1.0, MotionEaseOut((clamped - 0.7) / 0.3));
        return {eased, scale, 0.0};
    }
    case MotionPreset::glow:
        return {MotionMix(kMotionGlowOpacity, 1.0, eased), 1.0, 0.0};
    }
    return {};
}

// 退场第 t 时的卡片，从 start（退场开始时卡片的样子，进场播到一半时不会先跳回完整显示）出发，ease-in。
inline CardMotionFrame ExitFrame(MotionPreset preset, double t, CardMotionFrame start = {})
{
    const double eased = MotionEaseIn(t);
    switch (preset)
    {
    case MotionPreset::none:
        return start;
    case MotionPreset::fade:
        return {MotionMix(start.opacity, 0.0, eased), start.scale, start.lift};
    case MotionPreset::fade_scale:
    case MotionPreset::spring:
        return {MotionMix(start.opacity, 0.0, eased), MotionMix(start.scale, kMotionScaleFrom, eased), start.lift};
    case MotionPreset::slide_up:
        // 进场的倒放：向下沉出。
        return {MotionMix(start.opacity, 0.0, eased), start.scale, MotionMix(start.lift, -kMotionSlideDistance, eased)};
    case MotionPreset::glow:
        return {MotionMix(start.opacity, kMotionGlowOpacity, eased), start.scale, start.lift};
    }
    return start;
}

// 错峰浮现的一行在第 t 时的样子（只有不透明度和抬起量）。
inline CardMotionFrame RowRevealFrame(double t)
{
    const double eased = MotionEaseOut(t);
    return {eased, 1.0, MotionMix(-kMotionRowRevealDistance, 0.0, eased)};
}

// 选中装饰第 t 时的高亮底色：不透明度、以行中心为轴的缩放，以及 glow 外圈描边的不透明度（相对强调色）。
struct SelectionMotionFrame
{
    double fillOpacity = 1.0;
    double scale = 1.0;
    double ringOpacity = 0.0;
};

inline SelectionMotionFrame SelectionFrame(MotionPreset preset, double t)
{
    const double eased = MotionEaseOut(t);
    switch (preset)
    {
    case MotionPreset::none:
        return {};
    case MotionPreset::fade:
    case MotionPreset::fade_scale:
    case MotionPreset::slide_up:
        // 只有底色从 0.45 淡入，文字一开始就是完整的，与 Windows 只动高亮底色相同。
        return {MotionMix(0.45, 1.0, eased), 1.0, 0.0};
    case MotionPreset::spring:
    {
        const double clamped = std::clamp(t, 0.0, 1.0);
        const double scale = clamped < 0.65 ? MotionMix(0.94, 1.025, MotionEaseOut(clamped / 0.65))
                                            : MotionMix(1.025, 1.0, MotionEaseOut((clamped - 0.65) / 0.35));
        return {1.0, scale, 0.0};
    }
    case MotionPreset::glow:
        // 底色照常，外加一圈 2pt 强调色描边从 0.65 淡到 0。
        return {1.0, 1.0, MotionMix(0.65, 0.0, eased)};
    }
    return {};
}

// 装饰图待机动效在一个周期里第 phase（0–1）时的变换：以图片底边中点为轴缩放 scale、旋转 angle 弧度，再在屏幕方向上抬起 lift。幅度按图片大小收小，最大位移不超过 margin。
struct DecorationMotionFrame
{
    double scale = 1.0;
    double angle = 0.0;
    double lift = 0.0;
};

inline DecorationMotionFrame DecorationMotionAt(DecorationMotionKind motion, double phase, double width, double height,
                                                double margin)
{
    phase = phase - std::floor(phase);
    width = std::max(1.0, width);
    height = std::max(1.0, height);
    // 关键帧之间各用 ease-in-out；value(k) 是第 k 段起点的值。
    const auto segment = [&](const double *times, const double *values, int count, bool bounce) {
        for (int at = 0; at + 1 < count; ++at)
        {
            if (phase > times[at + 1] && at + 2 < count) continue;
            const double local = (phase - times[at]) / std::max(1e-9, times[at + 1] - times[at]);
            // 起跳段上升减速、下落段加速，像受重力；其余段平滑。
            const double eased = !bounce ? MotionEaseInOut(local)
                                 : at == 1 ? MotionEaseOut(local)
                                 : at == 2 ? MotionEaseIn(local)
                                           : local;
            return MotionMix(values[at], values[at + 1], eased);
        }
        return values[count - 1];
    };
    switch (motion)
    {
    case DecorationMotionKind::none:
        return {};
    case DecorationMotionKind::floating:
    {
        const double distance = std::min(kMotionFloatDistance, margin);
        const double times[] = {0.0, 0.25, 0.5, 0.75, 1.0};
        const double values[] = {0.0, distance, 0.0, -distance, 0.0};
        return {1.0, 0.0, segment(times, values, 5, false)};
    }
    case DecorationMotionKind::sway:
    {
        const double angle = std::min(kMotionSwayRadians, margin / std::max(height, width / 2.0));
        const double times[] = {0.0, 0.25, 0.5, 0.75, 1.0};
        const double values[] = {0.0, angle, 0.0, -angle, 0.0};
        return {1.0, segment(times, values, 5, false), 0.0};
    }
    case DecorationMotionKind::breathe:
    {
        const double scale = 1.0 + std::min(kMotionBreatheScale - 1.0, std::min(margin / height, 2.0 * margin / width));
        const double times[] = {0.0, 0.5, 1.0};
        const double values[] = {1.0, scale, 1.0};
        return {segment(times, values, 3, false), 0.0, 0.0};
    }
    case DecorationMotionKind::bounce:
    {
        // 大半个周期站着不动，然后起跳、落下。
        const double hop = std::min(kMotionBounceHeight, margin);
        const double times[] = {0.0, 0.6, 0.75, 0.9, 1.0};
        const double values[] = {0.0, 0.0, hop, 0.0, 0.0};
        return {1.0, 0.0, segment(times, values, 5, true)};
    }
    }
    return {};
}
} // namespace msime::candidate
