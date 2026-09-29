import app.msime.android.CandidateAppearance;
import app.msime.android.KeyboardSkin;
import java.util.List;

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

        // The touch strip takes the keyboard palette: its background, the key text, the secondary
        // colour for numbers and translations, and the accent for the selected candidate's text,
        // which the design draws with no fill.
        CandidateAppearance.Palette system = CandidateAppearance.fromSkin(KeyboardSkin.system(false));
        check("system".equals(system.id()));
        check(system.surface() == 0xffe6eae2);
        check(system.text() == 0xff191c19);
        check(system.number() == 0xff414941);
        check(system.accent() == 0xff2c7a4b);
        check(system.selected() == 0 && system.border() == 0);
        check(system.hover() == 0x0f191c19);
        check(system.textFor(true) == 0xff2c7a4b && system.textFor(false) == 0xff191c19);
        CandidateAppearance.Palette dark = CandidateAppearance.fromSkin(KeyboardSkin.system(true));
        check(dark.surface() == 0xff1d201d && dark.accent() == 0xff8fd5a6);
        check(!dark.key().equals(system.key()));

        CandidateAppearance.Palette night = CandidateAppearance.fromSkin(KeyboardSkin.palette(
            "night", "夜青", true, "#0F1B22", "#1D3340", "#15252E", "#E6F1F4", "#86A6B0",
            "#4FD1C5", "#000000"));
        check("night".equals(night.id()));
        check(night.surface() == 0xff0f1b22 && night.text() == 0xffe6f1f4);
        check(night.number() == 0xff86a6b0 && night.accent() == 0xff4fd1c5);
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
            KeyboardSkin.system(false), "", "bad\nfont", List.of("", "x".repeat(129)));
        check("Noto Sans SC".equals(invalidFonts.fontFamily()));
        check(invalidFonts.englishFont().isEmpty());
        check(invalidFonts.fallbackFonts().equals(List.of("Noto Sans SC", "Microsoft YaHei")));
        System.out.println("Android candidate appearance: keyboard-palette strip, accent-only selection, "
            + "alpha-last hints and font fallback passed");
    }
}
