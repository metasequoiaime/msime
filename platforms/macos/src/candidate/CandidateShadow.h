#pragma once

#include "CandidateSkin.h"

namespace msime::mac {

struct CandidateShadowPass {
    double x = 0, y = 0, blur = 0, alpha = 0;
};

struct CandidateShadow {
    bool nativeWindow = true;
    std::array<CandidateShadowPass, 2> passes{};
    SkinInsets outer;
};

// 固定来源 candidate_skin_scene.cpp 的双层配方；档位只改变透明度，缩放只改变几何。
inline CandidateShadow ResolveCandidateShadow(const std::optional<std::string> &style, bool dark,
    bool canvas, bool panelLoaded, double scale) {
    CandidateShadow result;
    result.nativeWindow = false;
    if (!style) {
        result.nativeWindow = !canvas;
        return result;
    }
    if (panelLoaded || *style == "none") return result;
    const double alpha = *style == "soft" ? 0.5 : *style == "strong" ? 1.6 : 1.0;
    result.passes = {{{8 * scale, 10 * scale, 24 * scale, (dark ? 0.34 : 0.18) * alpha},
                      {2 * scale, 3 * scale, 8 * scale, (dark ? 0.22 : 0.10) * alpha}}};
    // 与来源窗口一样留出透明边带，不能让视图边界把卡片外的阴影全部裁掉。
    result.outer = {20 * scale, 32 * scale, 40 * scale, 32 * scale};
    return result;
}

inline SkinInsets CandidateShadowExtraInsets(const CandidateShadow &shadow, const SkinInsets &outer, double topBand) {
    return {std::max(0.0, shadow.outer.top - outer.top - topBand),
            std::max(0.0, shadow.outer.right - outer.right),
            std::max(0.0, shadow.outer.bottom - outer.bottom),
            std::max(0.0, shadow.outer.left - outer.left)};
}

} // msime::mac 命名空间
