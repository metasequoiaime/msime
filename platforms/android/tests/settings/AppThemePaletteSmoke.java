import app.msime.android.AppThemePalette;
import app.msime.android.ColorPolicy;
import app.msime.android.AppThemePalette.Mode;
import app.msime.android.AppThemePalette.Seed;
import java.util.ArrayList;
import java.util.List;

/**
 * 应用主题的派生色：种子色写死在这里（设计令牌 §1.2，深色 onAccent 用 `mix(accent 25%, #000)`），期望值是原型在浏览器里算出的结果（§1.3、§1.4）。秋杉浅深逐项核对，春夏冬抽样。
 *
 * <p>不读 `platforms/android/tests/settings/app-theme-catalog.json`：那份文件和 Rust 种子的一致性由另外的检查负责，这里只锁住 Java 的公式。
 */
public final class AppThemePaletteSmoke {
    private static final List<String> failures = new ArrayList<>();

    static final Seed SPRING = new Seed("chunya", "spring",
        new Mode(0xFF4E9A3A, 0xFFEAF4E0, 0xFFFAFDF6, 0x2E4E9A3A, 0xFFFFFFFF),
        new Mode(0xFF9AD983, 0xFF16200F, 0xFF202C18, 0xFF2A2F2A, 0xFF263621));
    static final Seed SUMMER = new Seed("xiayin", "summer",
        new Mode(0xFF17704A, 0xFFDDEFE4, 0xFFF6FBF8, 0x2E17704A, 0xFFFFFFFF),
        new Mode(0xFF5FD39A, 0xFF0C1C14, 0xFF14281D, 0xFF2A2F2A, 0xFF183526));
    static final Seed AUTUMN = new Seed("qiushan", "autumn",
        new Mode(0xFFB5562B, 0xFFF6E9DC, 0xFFFFFBF6, 0x33B5562B, 0xFFFFFFFF),
        new Mode(0xFFF0975F, 0xFF21150F, 0xFF2E1E15, 0xFF2A2F2A, 0xFF3C2618));
    static final Seed WINTER = new Seed("dongxue", "winter",
        new Mode(0xFF3A6A8A, 0xFFE6EEF4, 0xFFFAFCFE, 0x2E3A6A8A, 0xFFFFFFFF),
        new Mode(0xFF93C7E6, 0xFF0F1820, 0xFF18232D, 0xFF2A2F2A, 0xFF25323A));

    static void expect(String label, int actual, int expected) {
        if (actual != expected) {
            failures.add(label + ": " + ColorPolicy.hexArgb(actual) + " != " + ColorPolicy.hexArgb(expected));
        }
    }

    static void autumnLight() {
        AppThemePalette p = AppThemePalette.of(AUTUMN, false);
        expect("autumn L accent", p.accent, 0xFFB5562B);
        expect("autumn L onAccent", p.onAccent, 0xFFFFFFFF);
        expect("autumn L accentSoft", p.accentSoft, 0x22B5562B);
        expect("autumn L bg", p.background, 0xFFF6E9DC);
        expect("autumn L andCard", p.card, 0xFFFBF5EF);
        expect("autumn L rowBg", p.rowBackground, 0xFFFFFBF6);
        expect("autumn L hair", p.hair, 0x33B5562B);
        expect("autumn L text", p.text, 0xFF191C19);
        expect("autumn L sub", p.sub, 0xFF414941);
        expect("autumn L outline", p.outline, 0xFF717970);
        expect("autumn L swOff", p.switchOff, 0xFFDFE4DB);
        expect("autumn L logoBg", p.logoBackground, 0xFF944723);
        expect("autumn L logoCirc", p.logoDisc, 0xFFF1DFD4);
        expect("autumn L splash bg", p.splashBackground, 0xFF2C1A11);
        expect("autumn L splash disc", p.splashDisc, 0xFFF6EBE6);
        expect("autumn L splash bar", p.splashBar, 0xFFCB896B);
        expect("autumn L splash glow", p.splashGlow, 0x57B5562B);
        expect("autumn L stat bar", p.statBar, 0xFFEED8CC);
        int[] heat = {0xFFF7ECE3, 0xFFEED8CC, 0xFFE0B9A5, 0xFFD09276, 0xFFBC663F};
        for (int level = 0; level < heat.length; level++) expect("autumn L heat" + level, p.heat(level), heat[level]);
        expect("autumn L kb.bg", p.keyboardBackground, 0xFFEED7C7);
        expect("autumn L kb.key", p.keyboardKey, 0xFFFDF9F6);
        expect("autumn L kb.spec", p.keyboardFunction, 0xFFE6C6B2);
        expect("autumn L kb.fg", p.keyboardText, 0xFF191C19);
        expect("autumn L kb.sub", p.keyboardSub, 0xFF56685A);
        expect("autumn L kb.hair", p.keyboardHair, 0x1F000000);
        expect("autumn L press", p.press, 0x12000000);
        expect("autumn L hover", p.hover, 0x0D000000);
        expect("autumn L toast bg", p.toastBackground, 0xFF1C1C1E);
        expect("autumn L toast fg", p.toastText, 0xFFFFFFFF);
    }

    static void autumnDark() {
        AppThemePalette p = AppThemePalette.of(AUTUMN, true);
        expect("autumn D accent", p.accent, 0xFFF0975F);
        expect("autumn D onAccent", p.onAccent, 0xFF3C2618);
        expect("autumn D accentSoft", p.accentSoft, 0x40F0975F);
        expect("autumn D bg", p.background, 0xFF21150F);
        expect("autumn D andCard", p.card, 0xFF2C2720);
        expect("autumn D rowBg", p.rowBackground, 0xFF2E1E15);
        expect("autumn D hair", p.hair, 0xFF2A2F2A);
        expect("autumn D text", p.text, 0xFFE1E3DE);
        expect("autumn D sub", p.sub, 0xFFC0C9BF);
        expect("autumn D outline", p.outline, 0xFF8A9389);
        expect("autumn D swOff", p.switchOff, 0xFF323532);
        expect("autumn D logoBg", p.logoBackground, 0xFFC57C4E);
        expect("autumn D logoCirc", p.logoDisc, 0xFF573F2E);
        expect("autumn D splash bg", p.splashBackground, 0xFF38271B);
        expect("autumn D splash disc", p.splashDisc, 0xFFFDF3EC);
        expect("autumn D splash bar", p.splashBar, 0xFFF5B68F);
        expect("autumn D splash glow", p.splashGlow, 0x57F0975F);
        expect("autumn D stat bar", p.statBar, 0xFF5B422F);
        int[] heat = {0xFF382D24, 0xFF4F3B2C, 0xFF775138, 0xFFA66C47, 0xFFDC8C59};
        for (int level = 0; level < heat.length; level++) expect("autumn D heat" + level, p.heat(level), heat[level]);
        expect("autumn D kb.bg", p.keyboardBackground, 0xFF2C241D);
        expect("autumn D kb.key", p.keyboardKey, 0xFF4C453E);
        expect("autumn D kb.spec", p.keyboardFunction, 0xFF42382E);
        expect("autumn D kb.fg", p.keyboardText, 0xFFE1E3DE);
        expect("autumn D kb.sub", p.keyboardSub, 0xFF93A596);
        expect("autumn D kb.hair", p.keyboardHair, 0x24FFFFFF);
        expect("autumn D press", p.press, 0x1AFFFFFF);
        expect("autumn D hover", p.hover, 0x0FFFFFFF);
        expect("autumn D toast bg", p.toastBackground, 0xFFF2F2F2);
        expect("autumn D toast fg", p.toastText, 0xFF111111);
    }

    /** 春夏冬各取 andCard、logo 两色、开屏、统计柱和跟随系统键盘三色，浅深都查。 */
    static void sample(String name, Seed seed, boolean dark, int card, int logoBg, int logoCirc,
            int splashBg, int statBar, int kbBg, int kbKey, int kbSpec) {
        AppThemePalette p = AppThemePalette.of(seed, dark);
        String label = name + (dark ? " D " : " L ");
        expect(label + "andCard", p.card, card);
        expect(label + "logoBg", p.logoBackground, logoBg);
        expect(label + "logoCirc", p.logoDisc, logoCirc);
        expect(label + "splash bg", p.splashBackground, splashBg);
        expect(label + "stat bar", p.statBar, statBar);
        expect(label + "kb.bg", p.keyboardBackground, kbBg);
        expect(label + "kb.key", p.keyboardKey, kbKey);
        expect(label + "kb.spec", p.keyboardFunction, kbSpec);
        expect(label + "accentSoft alpha", p.accentSoft >>> 24, dark ? 0x40 : 0x22);
        expect(label + "onAccent", p.onAccent, seed.mode(dark).onAccent);
    }

    static void otherSeasons() {
        sample("spring", SPRING, false, 0xFFF6FAF1, 0xFF407E30, 0xFFDEEDD7, 0xFF182814, 0xFFD7E9D0, 0xFFD7E9CC, 0xFFFAFCF7, 0xFFC5DEB8);
        sample("spring", SPRING, true, 0xFF252C23, 0xFF7EB26B, 0xFF3F5238, 0xFF273422, 0xFF41563A, 0xFF232A21, 0xFF444C41, 0xFF364133);
        sample("summer", SUMMER, false, 0xFFF0F8F3, 0xFF135C3D, 0xFFD1E5DB, 0xFF0D1F17, 0xFFC9DFD4, 0xFFC5E0D2, 0xFFF7FBF8, 0xFFADD1BF);
        sample("summer", SUMMER, true, 0xFF202C25, 0xFF4EAD7E, 0xFF2E503F, 0xFF1B3327, 0xFF2F5441, 0xFF1D2A23, 0xFF3E4B44, 0xFF2E4036);
        sample("winter", WINTER, false, 0xFFF4F7FA, 0xFF305771, 0xFFDAE4EA, 0xFF141E24, 0xFFD2DEE6, 0xFFD1DEE7, 0xFFF9FBFC, 0xFFBDCEDB);
        sample("winter", WINTER, true, 0xFF252B2B, 0xFF79A3BD, 0xFF3D4D54, 0xFF253136, 0xFF3F5058, 0xFF22292B, 0xFF434A4B, 0xFF353E41);
        AppThemePalette summerLight = AppThemePalette.of(SUMMER, false);
        expect("summer L splash disc", summerLight.splashDisc, 0xFFE3EEE9);
        expect("summer L splash bar", summerLight.splashBar, 0xFF5D9B80);
        AppThemePalette winterDark = AppThemePalette.of(WINTER, true);
        expect("winter D splash disc", winterDark.splashDisc, 0xFFF2F8FC);
        expect("winter D splash bar", winterDark.splashBar, 0xFFB3D8EE);
        expect("winter L hair", AppThemePalette.of(WINTER, false).hair, 0x2E3A6A8A);
        expect("spring D hair", AppThemePalette.of(SPRING, true).hair, 0xFF2A2F2A);
    }

    static void helpers() {
        expect("mix 100", AppThemePalette.mix(0xFF123456, 100, 0xFFFFFFFF), 0xFF123456);
        expect("mix 0", AppThemePalette.mix(0xFF123456, 0, 0xFFFFFFFF), 0xFFFFFFFF);
        expect("contract #RRGGBB", AppThemePalette.contractColor("#B5562B"), 0xFFB5562B);
        expect("contract #RRGGBBAA", AppThemePalette.contractColor("#B5562B33"), 0x33B5562B);
        expect("contract lower case", AppThemePalette.contractColor("#f0975f40"), 0x40F0975F);
        for (String bad : new String[] {null, "", "B5562B", "#B5562", "#B5562B3", "#GG562B", "#B5562B33FF"}) {
            if (AppThemePalette.contractColor(bad) != null) failures.add("contract colour accepted " + bad);
        }
        if (AppThemePalette.Seed.fromResolved(null, null) != null) failures.add("fromResolved accepted null");
        if (AppThemePalette.Mode.fromResolved(null) != null) failures.add("Mode.fromResolved accepted null");
        // 代码里的秋杉回退种子必须与写死在这里的设计值一致，res/values* 的基础主题也取这一组。
        for (boolean dark : new boolean[] {false, true}) {
            Mode code = Seed.AUTUMN.mode(dark);
            Mode design = AUTUMN.mode(dark);
            String label = "fallback autumn " + (dark ? "D " : "L ");
            expect(label + "accent", code.accent, design.accent);
            expect(label + "bg", code.background, design.background);
            expect(label + "card", code.card, design.card);
            expect(label + "hair", code.hair, design.hair);
            expect(label + "onAccent", code.onAccent, design.onAccent);
        }
        if (!"qiushan".equals(Seed.AUTUMN.id) || !"autumn".equals(Seed.AUTUMN.season)) failures.add("fallback autumn id");
        try {
            AppThemePalette.of(AUTUMN, false).heat(5);
            failures.add("heat level 5 accepted");
        } catch (IllegalArgumentException expected) {
            // 五档之外必须拒绝。
        }
    }

    public static void main(String[] args) {
        autumnLight();
        autumnDark();
        otherSeasons();
        helpers();
        if (!failures.isEmpty()) throw new AssertionError(String.join("\n", failures));
    }
}
