import app.msime.android.CandidateAppearance;
import app.msime.android.KeyboardSkin;
import java.util.List;
import java.util.Map;

public final class CandidateAppearanceSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        check(!CandidateAppearance.isHorizontal("vertical"));
        check(CandidateAppearance.isHorizontal("horizontal"));
        check(!CandidateAppearance.isHorizontal("untrusted"));
        check(CandidateAppearance.fontSize(12) == 12);
        check(CandidateAppearance.fontSize(32) == 32);
        check(CandidateAppearance.fontSize(11) == 16);
        check(CandidateAppearance.fontSize(33) == 16);
        // 候选条按钮只有 30dp 的触控下限，与 iOS、鸿蒙相同；皮肤的 itemMinWidth 是桌面候选窗的格宽，不用于候选条。
        check(CandidateAppearance.STRIP_CHIP_MIN_WIDTH_DP == 30);

        // The touch strip takes the keyboard palette: its background, the key text, the secondary
        // colour for numbers and translations, and the accent for the selected candidate's text,
        // which the design draws with no fill.
        CandidateAppearance.Palette system = CandidateAppearance.fromSkin(KeyboardSkin.system(false));
        check("system".equals(system.id()));
        check(system.surface() == 0xffdfecdf);
        check(system.text() == 0xff191c19);
        check(system.number() == 0xff56685a);
        check(system.accent() == 0xff2c7a4b);
        check(system.selected() == 0 && system.border() == 0);
        check(system.hover() == 0x0f191c19);
        check(system.textFor(true) == 0xff2c7a4b && system.textFor(false) == 0xff191c19);
        // 新设计的首选 chip：底为字母键色、字为皮肤的首选字色 600。跟随系统皮肤的首选字色是同一色相加深到 WCAG AA 的 #267547（#2C7A4B 在键盘底色上约 4.3:1）。
        check(system.chip() == 0xfffdfefc && system.chipText() == 0xff267547);
        check(system.chipFor(true) == 0xfffdfefc && system.chipFor(false) == 0);
        check(system.weightFor(true) == 600 && system.weightFor(false) == 400);
        check(CandidateAppearance.SELECTED_FONT_WEIGHT == 600);
        CandidateAppearance.Palette dark = CandidateAppearance.fromSkin(KeyboardSkin.system(true));
        check(dark.surface() == 0xff222a24 && dark.accent() == 0xff8fd5a6);
        check(dark.chip() == 0xff424b45 && dark.chipText() == 0xff8fd5a6);
        check(!dark.key().equals(system.key()));

        CandidateAppearance.Palette night = CandidateAppearance.fromSkin(KeyboardSkin.palette(
            "night", "夜青", true, "#0F1B22", "#1D3340", "#15252E", "#E6F1F4", "#86A6B0",
            "#4FD1C5", "#000000"));
        check("night".equals(night.id()));
        check(night.surface() == 0xff0f1b22 && night.text() == 0xffe6f1f4);
        check(night.number() == 0xff86a6b0 && night.accent() == 0xff4fd1c5);
        check(night.chip() == 0xff1d3340 && night.chipText() == 0xff4fd1c5);
        // An alpha-last hint colour from the contract reaches the strip with its alpha intact.
        CandidateAppearance.Palette translucent = CandidateAppearance.fromSkin(KeyboardSkin.palette(
            "custom", "自定义", false, null, null, null, "#123456", "#12345699", null, null));
        check(translucent.number() == 0x99123456 && translucent.text() == 0xff123456);

        CandidateAppearance.Palette fonts = CandidateAppearance.fromSkin(KeyboardSkin.system(false),
            "Noto Sans CJK", "Noto Sans Mono", List.of("Microsoft YaHei", "Noto Sans SC"));
        check("Noto Sans CJK".equals(fonts.fontFamily()));
        check("Noto Sans Mono".equals(fonts.englishFont()));
        check("Noto Sans Mono".equals(fonts.preferredFont()));
        check(fonts.fallbackFonts().equals(List.of("Microsoft YaHei", "Noto Sans SC")));
        CandidateAppearance.Palette invalidFonts = CandidateAppearance.fromSkin(
            KeyboardSkin.system(false), "", "bad\nfont", List.of("", "x".repeat(129), "坏\uD800"));
        check("Noto Sans SC".equals(invalidFonts.fontFamily()));
        check(invalidFonts.englishFont().isEmpty());
        check(invalidFonts.fallbackFonts().equals(List.of("Noto Sans SC", "Microsoft YaHei")));
        // 包的选中填充与文字单独解析，八位颜色保持共享契约的末尾透明度。
        CandidateAppearance.Palette external = CandidateAppearance.fromResolved(fonts, "fixture",
            Map.of("surface", "#123456", "text", "#ABCDEF", "number", "#98765499",
                "selected", "#224466", "selected_text", "#FFFFFF",
                "selected_number", "#FFFFFF99", "secondary", "#876543", "hover", "#00000000"));
        check(external.externalSkin() && !fonts.externalSkin());
        check(external.surface() == 0xff123456 && external.textFor(false) == 0xffabcdef);
        check(external.chip() == 0xff224466 && external.chipText() == 0xffffffff);
        check(external.textFor(true) == 0xffffffff && external.numberFor(true) == 0x99ffffff);
        check(external.numberFor(false) == 0x99987654 && external.secondaryFor(false) == 0xff876543);
        check(external.hover() == 0 && external.fontFamily().equals(fonts.fontFamily()));
        check(external.preferredFont().equals(fonts.preferredFont()));
        CandidateAppearance.Palette partial = CandidateAppearance.fromResolved(system, "partial",
            Map.of("surface", "invalid", "accent", "#112233"));
        check(partial.surface() == system.surface() && partial.chip() == system.chip());
        check(partial.chipText() == 0xff112233 && partial.text() == system.text());
        check(!partial.key().equals(system.key()));
        System.out.println("Android candidate appearance: keyboard-palette strip, accent-only selection, "
            + "alpha-last hints and font fallback passed");
    }
}
