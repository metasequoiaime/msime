#include "../../src/candidate/CandidateSkin.h"
#include "../../src/candidate/CandidateShadow.h"
#include "../../src/candidate/CandidatePager.h"
#include <algorithm>
#include <cassert>
#include <cmath>
#include <cstdio>
#include <string>
#include <string_view>

using msime::mac::NativeCandidateTokens;
using msime::mac::Rgba;
using msime::mac::ToolbarSkinTokens;
static void CheckColor(Rgba value, unsigned rgb, float alpha = 1) {
    assert(std::abs(value.r - ((rgb >> 16) & 255) / 255.f) < 0.00001f);
    assert(std::abs(value.g - ((rgb >> 8) & 255) / 255.f) < 0.00001f);
    assert(std::abs(value.b - (rgb & 255) / 255.f) < 0.00001f);
    assert(std::abs(value.a - alpha) < 0.00001f);
}
static bool SameColor(Rgba left, Rgba right) {
    return std::abs(left.r - right.r) < 0.00001f && std::abs(left.g - right.g) < 0.00001f &&
           std::abs(left.b - right.b) < 0.00001f && std::abs(left.a - right.a) < 0.00001f;
}
// WCAG 对比度：前景按它的不透明度叠到不透明的底色上，再和底色比。
static double Linear(double channel) {
    return channel <= 0.04045 ? channel / 12.92 : std::pow((channel + 0.055) / 1.055, 2.4);
}
static double Luminance(double r, double g, double b) {
    return 0.2126 * Linear(r) + 0.7152 * Linear(g) + 0.0722 * Linear(b);
}
static double ContrastOn(Rgba foreground, Rgba fill) {
    const double a = foreground.a;
    const double over = Luminance(foreground.r * a + fill.r * (1 - a), foreground.g * a + fill.g * (1 - a), foreground.b * a + fill.b * (1 - a));
    const double under = Luminance(fill.r, fill.g, fill.b);
    return (std::max(over, under) + 0.05) / (std::min(over, under) + 0.05);
}
static bool SameTokens(const msime::mac::SkinTokens &left, const msime::mac::SkinTokens &right) {
    return SameColor(left.surface, right.surface) && SameColor(left.border, right.border) &&
           SameColor(left.text, right.text) && SameColor(left.number, right.number) &&
           SameColor(left.selected, right.selected) && SameColor(left.selectedText, right.selectedText) &&
           SameColor(left.selectedNumber, right.selectedNumber) && SameColor(left.hover, right.hover) &&
           SameColor(left.accent, right.accent) && left.radius == right.radius &&
           left.showSelectedBar == right.showSelectedBar;
}
int main() {
    {
        msime::mac::ResolvedSkin skin;
        skin.tokens = NativeCandidateTokens(false);
        msime::mac::CandidateWindowStyle style;
        style.padding = 7;
        style.paddingHorizontal = 0;
        style.paddingVertical = 12;
        style.cornerRadius = 20;
        style.itemCornerRadius = 0;
        style.itemPaddingHorizontal = 3;
        style.itemPaddingVertical = 2;
        style.itemSpacing = 5;
        for (double scale : {0.5, 1.0, 1.5, 2.0}) {
            style.scale = scale;
            auto resolved = msime::mac::StyledCandidateSkin(skin, style);
            assert(msime::mac::CandidateInset(resolved.tokens) == 0);
            assert(msime::mac::CandidateVerticalInset(resolved.tokens) == 12 * scale);
            assert(resolved.tokens.radius == 20 * scale);
            assert(resolved.tokens.candidateRadius == 0 && resolved.tokens.selectedRadius == 0);
            assert(resolved.tokens.explicitItemRadius);
            assert(*resolved.tokens.itemPaddingHorizontal == 3 * scale);
            assert(*resolved.tokens.itemPaddingVertical == 2 * scale);
            assert(*resolved.tokens.itemSpacing == 5 * scale);
            style.cornerRadius = 32;
            resolved = msime::mac::StyledCandidateSkin(skin, style);
            assert(resolved.tokens.candidateRadius == 0);
            style.cornerRadius = 20;
        }
        style.scale = 1;
        style.paddingHorizontal.reset();
        assert(msime::mac::CandidateInset(msime::mac::StyledCandidateSkin(skin, style).tokens) == 7);
        skin.canvas = msime::mac::SkinCanvas{};
        skin.canvas->content = msime::mac::SkinInsets{8, 9, 10, 11};
        style.paddingHorizontal = 3;
        const auto canvas = msime::mac::StyledCandidateSkin(skin, style);
        const auto content = msime::mac::CanvasContentInsets(*canvas.canvas, 3, 12);
        assert(content.left == 3 && content.right == 3 && content.top == 12 && content.bottom == 12);
    }
    for (const bool dark : {false, true}) for (const double scale : {1.0, 1.5}) {
        for (const auto &style : {"none", "soft", "normal", "strong"}) {
            const auto shadow = msime::mac::ResolveCandidateShadow(std::string(style), dark, false, false, scale);
            assert(!shadow.nativeWindow);
            if (std::string(style) == "none") {
                assert(shadow.passes[0].alpha == 0 && shadow.outer.left == 0);
                continue;
            }
            const double alpha = std::string(style) == "soft" ? 0.5 : std::string(style) == "strong" ? 1.6 : 1;
            assert(std::abs(shadow.passes[0].alpha - (dark ? 0.34 : 0.18) * alpha) < 0.00001);
            assert(std::abs(shadow.passes[1].alpha - (dark ? 0.22 : 0.10) * alpha) < 0.00001);
            assert(shadow.passes[0].x == 8 * scale && shadow.passes[0].y == 10 * scale && shadow.passes[0].blur == 24 * scale);
            assert(shadow.passes[1].x == 2 * scale && shadow.passes[1].y == 3 * scale && shadow.passes[1].blur == 8 * scale);
            const auto suppressed = msime::mac::ResolveCandidateShadow(std::string(style), dark, true, true, scale);
            assert(!suppressed.nativeWindow && suppressed.passes[0].alpha == 0 && suppressed.outer.left == 0);
            const auto outer = msime::mac::CandidateShadowExtraInsets(shadow, {6 * scale, 50 * scale, 8 * scale, 4 * scale}, 12 * scale);
            assert(outer.top == 2 * scale && outer.right == 0 && outer.bottom == 32 * scale && outer.left == 28 * scale);
        }
    }
    assert(msime::mac::ResolveCandidateShadow(std::nullopt, false, false, false, 1).nativeWindow);
    assert(!msime::mac::ResolveCandidateShadow(std::nullopt, false, true, false, 1).nativeWindow);
    const auto chevron = msime::mac::CandidatePager(16, 1, 24, true, false);
    assert(chevron.buttonWidth == 12 && chevron.buttonHeight == 14 && chevron.column == 28 && chevron.marginBottom == 5);
    const auto triangle = msime::mac::CandidatePager(16, 1, 24, true, true);
    assert(triangle.buttonWidth == 15 && triangle.buttonHeight == 24 && triangle.column == 38 && triangle.dividerWidth == 1);
    const auto verticalChevron = msime::mac::CandidatePager(16, 1, 24, false, false);
    const auto verticalTriangle = msime::mac::CandidatePager(16, 1, 24, false, true);
    assert(verticalChevron.column == 0 && verticalChevron.row == 16);
    assert(verticalTriangle.buttonWidth == 13 && verticalTriangle.row == 21);

    // 选中行的释义色：不透明的选中底色上按底色挑白或近黑、0.82 不透明度，和底色的对比度至少 3:1（释义和读音同色，读音不再减淡）；半透明底色上就是皮肤的释义色；皮肤没有释义色时没有它，指定了就不推导。
    {
        struct Case { unsigned selected, translation, text; bool unreadable; };
        for (const Case &c : {Case{0x4D6BFE, 0x2F5BD3, 0xFFFFFF, true}, Case{0x23262F, 0x6B7180, 0xE6E8EE, false}, Case{0xF2C94C, 0xB8860B, 0x1D1D1F, true}}) {
            msime::mac::SkinTokens tokens = NativeCandidateTokens(false);
            char hex[8];
            std::snprintf(hex, sizeof hex, "#%06X", c.selected);
            tokens.selected = *msime::mac::ParseCssColor(hex);
            std::snprintf(hex, sizeof hex, "#%06X", c.translation);
            tokens.translation = msime::mac::ParseCssColor(hex);
            std::snprintf(hex, sizeof hex, "#%06X", c.text);
            tokens.text = *msime::mac::ParseCssColor(hex);
            msime::mac::DeriveSelectedForegrounds(tokens, true, true);
            assert(tokens.selectedTranslation && std::abs(tokens.selectedTranslation->a - 0.82f) < 0.00001f);
            assert(ContrastOn(*tokens.selectedTranslation, tokens.selected) >= 3.0);
            // 蓝底蓝字、黄底金字时皮肤自己的释义色读不清，这正是要换色的原因。
            if (c.unreadable) assert(ContrastOn(*tokens.translation, tokens.selected) < 3.0);
        }
        msime::mac::SkinTokens translucent = NativeCandidateTokens(false);
        translucent.selected = *msime::mac::ParseCssColor("#2F5BD329");
        translucent.translation = msime::mac::ParseCssColor("#2F5BD3");
        msime::mac::DeriveSelectedForegrounds(translucent, true, true);
        assert(translucent.selectedTranslation && SameColor(*translucent.selectedTranslation, *translucent.translation));
        msime::mac::SkinTokens plain = NativeCandidateTokens(false);
        msime::mac::DeriveSelectedForegrounds(plain, true, true);
        assert(!plain.translation && !plain.selectedTranslation);
        msime::mac::SkinTokens given = NativeCandidateTokens(false);
        given.translation = msime::mac::ParseCssColor("#2F5BD3");
        given.selectedTranslation = msime::mac::ParseCssColor("#FFEE00");
        msime::mac::DeriveSelectedForegrounds(given, true, true, false);
        CheckColor(*given.selectedTranslation, 0xFFEE00);
    }
    msime::mac::SetDefaultSkinsRoot("/synthetic/movable-state/skins");
    assert(msime::mac::DefaultSkinsRoot() == "/synthetic/movable-state/skins");
    msime::mac::SetDefaultSkinsRoot({});

    // Golden values from design-tokens-desktop §1.2 (the macOS column), not generated by the resolver: a translucent system card, the brand green as a solid selection with white text, 10pt card and 6pt rows.
    // 深色的选中底是品牌的强调墨绿 #17783F，不是强调色 #5FBF84：后者上的白字只有约 2.3:1。
    struct Fixture { bool dark; unsigned surface, border, text, number, accent, selected, hover; float borderAlpha, hoverAlpha; };
    const Fixture fixtures[] = {
        {false, 0xFFFFFF, 0x000000, 0x1D1D1F, 0x6E6E73, 0x2C7A4B, 0x2C7A4B, 0x000000, .12f, .06f},
        {true, 0x28282A, 0xFFFFFF, 0xF5F5F7, 0x98989D, 0x5FBF84, 0x17783F, 0xFFFFFF, .12f, .10f},
    };
    for (const auto &f : fixtures) {
        const auto t = NativeCandidateTokens(f.dark);
        CheckColor(t.surface, f.surface, .97f);
        CheckColor(t.border, f.border, f.borderAlpha);
        CheckColor(t.text, f.text);
        CheckColor(t.number, f.number);
        CheckColor(t.accent, f.accent);
        CheckColor(t.hover, f.hover, f.hoverAlpha);
        CheckColor(t.selected, f.selected);
        CheckColor(t.selectedHover, f.selected);
        CheckColor(t.selectedText, 0xFFFFFF);
        CheckColor(t.selectedNumber, 0xFFFFFF, .82f);
        // 默认选中行的候选文字和释义（没有皮肤释义色，读音同色）对选中底色都到 WCAG AA 的 4.5:1。白色 0.82 的序号色只有约 4.1–4.3:1，释义因此是加深到 0.86（深色）或 0.89（浅色）的白色，和选中文字同色。
        assert(ContrastOn(t.selectedText, t.selected) >= 4.5);
        assert(ContrastOn(t.selectedNumber, t.selected) < 4.5);
        const Rgba gloss = msime::mac::SelectedRowGloss(t);
        assert(SameColor(Rgba{gloss.r, gloss.g, gloss.b, 1.0f}, t.selectedText) && gloss.a > 0.82f && gloss.a < 1.0f);
        assert(ContrastOn(gloss, t.selected) >= 4.5);
        assert(std::abs(msime::mac::ForegroundContrast(gloss, t.selected) - ContrastOn(gloss, t.selected)) < 0.01);
        assert(t.radius == 10 && t.candidateRadius == 6 && t.selectedRadius == 6 && t.borderWidth == 1 && t.pad == 4 &&
               !t.showSelectedBar);
        // 选中与悬停共享 6pt 行圆角，首尾位置不改变形状。
        assert(msime::mac::CandidateRowRadius(t, true) == 6);
        assert(msime::mac::CandidateRowRadius(t, false) == 6);
    }
    // 用实际浅／深玻璃底色范围检查小字和选中行，不能只检查换了一个色值。
    for (bool dark : {false, true}) {
        auto tokens = NativeCandidateTokens(dark);
        msime::mac::ApplyNativeGlassCandidateContrast(tokens, dark);
        const auto plate = *msime::mac::ParseCssColor(dark ? "#48484A" : "#88888A");
        assert(ContrastOn(tokens.number, plate) >= 4.5);
        assert(ContrastOn(tokens.selectedText, tokens.selected) >= 4.5);
        assert(ContrastOn(tokens.selectedNumber, tokens.selected) >= 4.5);
        // 玻璃上的选中序号色已经够 4.5:1，释义就用它；深色保留 #5FBF84 底配深色字。
        assert(SameColor(msime::mac::SelectedRowGloss(tokens), tokens.selectedNumber));
        if (dark) CheckColor(tokens.selected, 0x5FBF84);
    }
    msime::mac::SkinTokens split;
    split.candidateRadius = 3.0f;
    split.selectedRadius = 7.0f;
    assert(msime::mac::CandidateRowRadius(split, true) == 7.0f);
    assert(msime::mac::CandidateRowRadius(split, false) == 3.0f);

    // The user's 候选窗 style over a resolved skin. The defaults change nothing at all.
    {
        msime::mac::ResolvedSkin skin;
        skin.tokens = NativeCandidateTokens(false);
        skin.backgroundOpacity = 0.8;
        skin.decorationTopDip = 40.0;
        skin.decorationWidthDip = 60.0;
        skin.minWidthDip = 200.0;
        const auto untouched = msime::mac::StyledCandidateSkin(skin, {});
        assert(SameTokens(untouched.tokens, skin.tokens) && untouched.tokens.candidateRadius == 6 && untouched.tokens.pad == 4);
        // 原生默认的 4pt 内边距让行与卡片同心（#7974）。
        assert(skin.tokens.radius - msime::mac::CandidateInset(skin.tokens) == skin.tokens.candidateRadius);
        assert(untouched.backgroundOpacity == 0.8 && untouched.decorationTopDip == 40.0 && untouched.minWidthDip == 200.0);

        // 用户圆角减去有效内边距，选中与悬停得到相同的内圆角。
        auto packaged = skin;
        packaged.tokens.radius = 20.0f;
        msime::mac::CandidateWindowStyle style;
        style.cornerRadius = 3.0;
        const auto tight = msime::mac::StyledCandidateSkin(packaged, style);
        assert(tight.tokens.radius == 3.0f && tight.tokens.candidateRadius == 0.0f && tight.tokens.selectedRadius == 0.0f);
        style.cornerRadius = 0.0;
        assert(msime::mac::StyledCandidateSkin(packaged, style).tokens.radius == 0.0f);
        style.cornerRadius = 14.0;
        const auto loose = msime::mac::StyledCandidateSkin(packaged, style);
        assert(loose.tokens.radius == 14.0f && loose.tokens.candidateRadius == 10.0f && loose.tokens.selectedRadius == 10.0f);
        // 零内边距不再扣除边框宽度；各档缩放下候选行都可以贴边。
        style.cornerRadius = 32.0;
        style.padding = 0.0;
        for (double scale : {0.5, 1.0, 1.5, 2.0}) {
            style.scale = scale;
            const auto extreme = msime::mac::StyledCandidateSkin(packaged, style);
            assert(extreme.tokens.radius == 32.0 * scale);
            assert(msime::mac::CandidateInset(extreme.tokens) == 0.0f);
            assert(extreme.tokens.candidateRadius == 32.0 * scale);
            assert(extreme.tokens.selectedRadius == extreme.tokens.candidateRadius);
        }
        // Without a user radius the package's stays, and the rows keep the host's 6pt shape they drew before the setting existed.
        auto square = skin;
        square.tokens.radius = 2.0f;
        const auto packageOnly = msime::mac::StyledCandidateSkin(square, {});
        assert(packageOnly.tokens.radius == 2.0f && packageOnly.tokens.candidateRadius == 6.0f && packageOnly.tokens.selectedRadius == 6.0f);

        // Opacity fades the card, its border and the package background, never the text, numbers or selection.
        msime::mac::CandidateWindowStyle faded;
        faded.opacity = 0.5;
        const auto half = msime::mac::StyledCandidateSkin(skin, faded);
        assert(std::abs(half.tokens.surface.a - skin.tokens.surface.a * 0.5f) < 0.00001f);
        assert(std::abs(half.tokens.border.a - skin.tokens.border.a * 0.5f) < 0.00001f);
        assert(std::abs(half.backgroundOpacity - 0.4) < 0.00001);
        assert(SameColor(half.tokens.text, skin.tokens.text) && SameColor(half.tokens.number, skin.tokens.number));
        assert(SameColor(half.tokens.selected, skin.tokens.selected) && SameColor(half.tokens.selectedText, skin.tokens.selectedText));
        assert(SameColor(half.tokens.accent, skin.tokens.accent) && SameColor(half.tokens.hover, skin.tokens.hover));

        // 整体大小在推导内圆角后缩放各项长度，细边框保持不变。
        msime::mac::CandidateWindowStyle large;
        large.scale = 1.5;
        large.cornerRadius = 8.0;
        const auto scaled = msime::mac::StyledCandidateSkin(skin, large);
        assert(scaled.tokens.radius == 12.0f && scaled.tokens.candidateRadius == 6.0f && scaled.tokens.selectedRadius == 6.0f);
        assert(scaled.tokens.pad == 6.0f && scaled.tokens.borderWidth == skin.tokens.borderWidth);
        assert(scaled.decorationTopDip == 60.0 && scaled.decorationWidthDip == 90.0 && scaled.minWidthDip == 300.0);
        assert(SameColor(scaled.tokens.surface, skin.tokens.surface) && scaled.backgroundOpacity == 0.8);

        // 用户内边距（#7316）在缩放之前替换皮肤包的或原生的 4pt，不改任何圆角；超出范围截到共享的 0–16。
        auto padded = skin;
        padded.tokens.pad = 10.0f;
        msime::mac::CandidateWindowStyle padStyle;
        padStyle.padding = 2.0;
        const auto compact = msime::mac::StyledCandidateSkin(padded, padStyle);
        assert(compact.tokens.pad == 2.0f && compact.tokens.radius == padded.tokens.radius && compact.tokens.candidateRadius == padded.tokens.candidateRadius);
        assert(msime::mac::StyledCandidateSkin(padded, {}).tokens.pad == 10.0f);
        padStyle.scale = 1.5;
        assert(msime::mac::StyledCandidateSkin(padded, padStyle).tokens.pad == 3.0f);
        padStyle.scale = 1.0;
        padStyle.padding = 40.0;
        assert(msime::mac::StyledCandidateSkin(padded, padStyle).tokens.pad == 16.0f);

        // 边框不限制窗口内边距，0 与 1 分别生效；负值仍不允许把候选行移到卡片外。
        msime::mac::SkinTokens inset = skin.tokens;
        inset.pad = 0.0f;
        assert(msime::mac::CandidateInset(inset) == 0.0f);
        assert(msime::mac::CandidateVerticalInset(inset) == 0.0f);
        inset.pad = 1.0f;
        assert(msime::mac::CandidateInset(inset) == 1.0f);
        assert(msime::mac::CandidateVerticalInset(inset) == 1.0f);
        inset.pad = 1.5f;
        assert(msime::mac::CandidateInset(inset) == 1.5f);
        inset.borderWidth = 0.0f;
        inset.pad = 0.0f;
        assert(msime::mac::CandidateInset(inset) == 0.0f);
        inset.pad = -3.0f;
        assert(msime::mac::CandidateInset(inset) == 0.0f);

        // 只调内边距：候选行保留皮肤的 6pt 圆角，直到内边距小到 6pt 的角会伸出 10pt 的卡片圆角，这时只调圆到刚好留在细边框内沿以内。
        padStyle.padding = 2.0;
        const auto two = msime::mac::StyledCandidateSkin(skin, padStyle);
        assert(two.tokens.candidateRadius == 6.0f && two.tokens.selectedRadius == 6.0f);
        const float tangent = 2.0f + std::sqrt(2.0f);
        for (double tight : {1.0, 0.0}) {
            padStyle.padding = tight;
            const auto hairline = msime::mac::StyledCandidateSkin(skin, padStyle);
            const float expected = 9.5f - (static_cast<float>(tight) - 0.5f) * tangent;
            assert(std::abs(hairline.tokens.candidateRadius - expected) < 0.0001f && hairline.tokens.selectedRadius == hairline.tokens.candidateRadius);
            assert(hairline.tokens.radius == 10.0f && hairline.tokens.pad == static_cast<float>(tight));
        }
        // 无边框且贴边时，行圆角等于卡片圆角，高亮沿着卡片的角走。
        auto borderless = skin;
        borderless.tokens.borderWidth = 0.0f;
        const auto flush = msime::mac::StyledCandidateSkin(borderless, padStyle);
        assert(flush.tokens.candidateRadius == 10.0f && flush.tokens.selectedRadius == 10.0f);
        // 下限在缩放之后计算，和不随缩放变粗的边框比较。
        padStyle.scale = 2.0;
        padStyle.padding = 1.0;
        const auto doubled = msime::mac::StyledCandidateSkin(skin, padStyle);
        assert(doubled.tokens.pad == 2.0f && doubled.tokens.radius == 20.0f);
        assert(std::abs(doubled.tokens.candidateRadius - (19.5f - 1.5f * tangent)) < 0.0001f);
        // 没有用户内边距时，皮肤自己的 pad 再小，行圆角也保持解析出来的值。
        auto thin = skin;
        thin.tokens.pad = 0.0f;
        assert(msime::mac::StyledCandidateSkin(thin, {}).tokens.candidateRadius == 6.0f);
    }

    // The picker is the shared catalog, in its order, and only its ids are themes.
    const auto &catalog = msime::mac::ThemeCatalog();
    const char *ids[] = {"system", "shuishan", "light", "paper", "night", "ink", "custom"};
    assert(catalog.size() == 7);
    for (size_t index = 0; index < catalog.size(); ++index) {
        assert(catalog[index].id == ids[index] && !catalog[index].title.empty());
        assert(msime::mac::IsGlobalThemeId(ids[index]));
        assert(msime::mac::IsThemeBaseId(ids[index]) == (index + 1 < catalog.size()));
    }
    assert(catalog[0].appearance.empty() && !catalog[0].hasPreview && catalog.back().appearance.empty());
    assert(catalog[1].appearance == "dark" && catalog[1].hasPreview && catalog[2].appearance == "light");
    CheckColor(catalog[1].previewAccent, 0x7FE08E);
    for (const char *retired : {"fluent", "wechat", "graphite", "willow_green", "Custom", ""})
        assert(!msime::mac::IsGlobalThemeId(retired));
    assert(msime::mac::ThemeTitle("shuishan") == "水杉" && msime::mac::ThemeTitle("custom") == "自定义");

    const msime::mac::CustomTheme none;
    for (bool dark : {false, true}) {
        // `system` resolves every slot to the native token in the system's mode and fixes no mode.
        const auto system = msime::mac::ResolveSkin("system", none, dark, "horizontal", {});
        assert(system.id == "system" && !system.fixedDark && system.dark == dark && system.candidateSkin.empty());
        assert(SameTokens(system.tokens, NativeCandidateTokens(dark)));
        // An unknown id draws `system` meanwhile.
        const auto unknown = msime::mac::ResolveSkin("fluent", none, dark, "horizontal", {});
        assert(unknown.id == "system" && SameTokens(unknown.tokens, NativeCandidateTokens(dark)));
        // A built-in theme fixes its mode whatever the system's is, and paints the shared palette.
        const auto shuishan = msime::mac::ResolveSkin("shuishan", none, dark, "horizontal", {});
        assert(shuishan.id == "shuishan" && shuishan.fixedDark == true && shuishan.dark);
        CheckColor(shuishan.tokens.surface, 0x2A2B27);
        CheckColor(shuishan.tokens.text, 0xFFFFFF);
        CheckColor(shuishan.tokens.accent, 0x7FE08E);
        CheckColor(shuishan.tokens.selected, 0x7FE08E, 0x24 / 255.f);
        CheckColor(shuishan.tokens.selectedText, 0x7FE08E);
        assert(SameColor(shuishan.tokens.selectedHover, shuishan.tokens.selected));
        const auto paper = msime::mac::ResolveSkin("paper", none, dark, "vertical", {});
        assert(paper.fixedDark == false && !paper.dark);
        CheckColor(paper.tokens.surface, 0xF7F5F0);
        // The native geometry stays: themes only recolour.
        assert(paper.tokens.radius == 10 && paper.tokens.selectedRadius == 6 && !paper.tokens.showSelectedBar);

        // `custom` with no base follows the system mode; an explicit text picker also sets the numbers.
        msime::mac::CustomTheme picked;
        picked.candidateColors.text = "#123456";
        picked.candidateColors.accent = "#ABCDEF";
        const auto custom = msime::mac::ResolveSkin("custom", picked, dark, "horizontal", {});
        assert(custom.id == "custom" && !custom.fixedDark && custom.dark == dark);
        CheckColor(custom.tokens.text, 0x123456);
        CheckColor(custom.tokens.accent, 0xABCDEF);
        CheckColor(custom.tokens.number, 0x123456, 0x9D / 255.f);
        // A null `selected` is the native solid fill, so it follows the picked accent; its foregrounds follow that fill, which is light here, so they turn dark rather than staying the native white.
        CheckColor(custom.tokens.selected, 0xABCDEF);
        assert(SameColor(custom.tokens.selectedText, Rgba{0.1f, 0.1f, 0.1f, 1.0f}));
        assert(SameColor(custom.tokens.selectedNumber, Rgba{0.1f, 0.1f, 0.1f, 0.82f}));
        // A dark picked accent keeps the native white. A picked selection colour has its foregrounds chosen by the shared core instead, black or white by its luminance (custom-theme-parity.json), so a light one reads in black.
        msime::mac::CustomTheme darkAccent;
        darkAccent.candidateColors.accent = "#1F4E3D";
        const auto darkFill = msime::mac::ResolveSkin("custom", darkAccent, dark, "horizontal", {});
        CheckColor(darkFill.tokens.selectedText, 0xFFFFFF);
        CheckColor(darkFill.tokens.selectedNumber, 0xFFFFFF, .82f);
        msime::mac::CustomTheme lightFill;
        lightFill.candidateColors.selected = "#E8E8E8";
        const auto light = msime::mac::ResolveSkin("custom", lightFill, dark, "horizontal", {});
        CheckColor(light.tokens.selected, 0xE8E8E8);
        CheckColor(light.tokens.selectedText, 0x000000);
        CheckColor(light.tokens.selectedNumber, 0x000000, 0x9D / 255.f);
        // A custom theme over a built-in one takes that theme's mode and fills the unpicked slots from it.
        picked.base = "night";
        const auto overNight = msime::mac::ResolveSkin("custom", picked, dark, "horizontal", {});
        assert(overNight.fixedDark == true && overNight.dark);
        CheckColor(overNight.tokens.surface, 0x16262F);
        CheckColor(overNight.tokens.text, 0x123456);
        // The custom pickers mean nothing while another theme is selected.
        const auto ink = msime::mac::ResolveSkin("ink", picked, dark, "horizontal", {});
        CheckColor(ink.tokens.text, 0x9A9A9A);
        // A malformed picker value is dropped rather than failing the whole request.
        msime::mac::CustomTheme malformed;
        malformed.candidateColors.text = "red";
        malformed.candidateSkin = "../escape";
        const auto dropped = msime::mac::ResolveSkin("custom", malformed, dark, "horizontal", {});
        assert(dropped.id == "custom" && dropped.candidateSkin.empty());
        assert(SameColor(dropped.tokens.text, NativeCandidateTokens(dark).text));

        // Without a package the toolbar is the candidate palette itself.
        assert(SameTokens(ToolbarSkinTokens(system, {}), system.tokens));
        assert(SameTokens(ToolbarSkinTokens(shuishan, {}), shuishan.tokens));
        assert(SameTokens(ToolbarSkinTokens(overNight, {}), overNight.tokens));
        // Without a package no theme has a translation colour of its own (secondary follows number) and the toolbar has no divider colour of its own.
        for (const auto *resolved : {&system, &shuishan, &paper, &custom, &overNight, &ink})
        {
            assert(!resolved->tokens.translation && !ToolbarSkinTokens(*resolved, {}).divider);
            assert(resolved->backgroundPath.empty() && resolved->decorationAlign == msime::mac::DecorationAlign::right);
        }
        // A card radius, however it got there, never reaches the toolbar.
        auto rounded = system;
        rounded.tokens.radius = 24.0f;
        assert(ToolbarSkinTokens(rounded, {}).radius == NativeCandidateTokens(rounded.dark).radius);
    }
}
