package app.msime.android;

/** 共享候选文字色穿过键盘主题解析与应用主题叠加后仍保留，不改变其他候选配色。 */
public final class KeyboardSkinSelectedTextSmoke {
    private static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        KeyboardSkin picked = KeyboardSkin.resolved("custom", "自定义", null, false,
            null, null, "#7b3fbf");
        check(picked.touchSelectedText() == 0xff7b3fbf);
        KeyboardSkin themed = picked.withAppTheme(AppThemePalette.Seed.AUTUMN);
        check(themed.touchSelectedText() != null && themed.touchSelectedText() == 0xff7b3fbf);
        check(themed.candidateSelectedForeground().equals(picked.candidateSelectedForeground()));
        check(themed.keyForeground().equals(picked.keyForeground()));
        check(themed.keyBackground().equals(picked.keyBackground()));
        CandidateAppearance.Palette appearance = CandidateAppearance.from(null, themed);
        check(appearance.chipText() == 0xff7b3fbf);
        check(appearance.correctionFor(true) == 0xff7b3fbf);
        CandidateAppearance.Palette plain = CandidateAppearance.from(null,
            KeyboardSkin.system(false).withAppTheme(AppThemePalette.Seed.AUTUMN));
        check(appearance.textFor(false) == plain.textFor(false));
        check(appearance.surface() == plain.surface() && appearance.chip() == plain.chip());
        for (String absent : new String[] {null, "purple", "#7b3fbf80"}) {
            KeyboardSkin unpicked = KeyboardSkin.resolved("custom", "自定义", null, false,
                null, null, absent).withAppTheme(AppThemePalette.Seed.AUTUMN);
            check(unpicked.touchSelectedText() == null);
        }
    }
}
