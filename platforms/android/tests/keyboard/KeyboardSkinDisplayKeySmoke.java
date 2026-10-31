package app.msime.android;

/**
 * 偏好每秒重读一次，`reloadPreferences` 用 {@link KeyboardSkin#displayKey} 判断键盘皮肤变没变、要不要整页重画。这里按它的真实顺序构造前后两张皮肤：重读前是渲染时 `ImeStyler.themed` 写回的当季配色，重读后是按偏好重建、还没着色的 classic 基础色。
 */
public final class KeyboardSkinDisplayKeySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        AppThemePalette.Seed autumn = AppThemePalette.Seed.AUTUMN;
        AppThemePalette.Seed winter = new AppThemePalette.Seed("dongxue", "winter",
            new AppThemePalette.Mode(0xFF3E6A8A, 0xFFEEF3F7, 0xFFFFFFFF, 0x333E6A8A, 0xFFFFFFFF),
            new AppThemePalette.Mode(0xFF9CC3E0, 0xFF10161B, 0xFF1A2229, AppThemePalette.HAIR_DARK, 0xFF0F2A3D));
        AppThemePalette.Seed sijiWinter = new AppThemePalette.Seed("siji", "winter", winter.light, winter.dark);

        // 跟随系统皮肤 + 季节应用主题，偏好没变：渲染后的当季皮肤和重读重建的 classic 皮肤画出来是同一个样子，不能重画。
        KeyboardSkin drawn = KeyboardSkin.system(false, autumn);
        KeyboardSkin reloaded = KeyboardSkin.system(false);
        check(!drawn.key().equals(reloaded.key()),
            "fixture: the themed and the rebuilt system skin carry different raw keys, which is the trap");
        check(drawn.displayKey(autumn).equals(reloaded.displayKey(autumn)),
            "reloading unchanged preferences with a seasonal theme must not request a render");
        KeyboardSkin drawnDark = KeyboardSkin.system(true, autumn);
        check(drawnDark.displayKey(autumn).equals(KeyboardSkin.system(true).displayKey(autumn)),
            "the dark system skin must not request a render on an unchanged reload either");
        check(drawn.followsAppTheme() && reloaded.followsAppTheme(), "system skins follow the app theme");

        // 换了明暗：重画一次。
        check(!drawn.displayKey(autumn).equals(KeyboardSkin.system(true).displayKey(autumn)),
            "switching light and dark must request a render");
        // 换了季节或换了应用主题：种子的季节或 id 变了，重画一次；重画后再重读同一个种子就稳定下来。
        check(!drawn.displayKey(autumn).equals(reloaded.displayKey(winter)),
            "a season change must request a render");
        check(!reloaded.displayKey(winter).equals(reloaded.displayKey(sijiWinter)),
            "switching to another app theme in the same season must request a render");
        check(KeyboardSkin.system(false, winter).displayKey(winter).equals(reloaded.displayKey(winter)),
            "after the season change re-renders, the next unchanged reload must be quiet again");

        // 命名皮肤原样画，不跟应用主题换底色，但键仍带上种子：回车键和开启态底色取应用主题的季节色（`withAppTheme`）；品牌键取皮肤自己的配色，不靠种子。
        KeyboardSkin ink = KeyboardSkin.palette("ink", "水墨", false, "#F4F1EA", "#FFFFFF",
            "#E5E0D5", "#1F1F1F", "#6B6B6B", "#3A3A3A", "#FFFFFF");
        KeyboardSkin inkAgain = KeyboardSkin.palette("ink", "水墨", false, "#F4F1EA", "#FFFFFF",
            "#E5E0D5", "#1F1F1F", "#6B6B6B", "#3A3A3A", "#FFFFFF");
        check(!ink.followsAppTheme(), "a named skin does not follow the app theme");
        check(ink.displayKey(autumn).equals(inkAgain.displayKey(autumn)),
            "reloading an unchanged named skin must not request a render");
        check(!ink.displayKey(autumn).equals(ink.displayKey(winter)),
            "a season change must re-render a named skin too, for the return key and switched-on tiles");
        // 换皮肤：跟随系统换成命名皮肤，或命名皮肤之间换配色，都要重画一次。
        check(!drawn.displayKey(autumn).equals(ink.displayKey(autumn)),
            "switching from the system skin to a named skin must request a render");
        KeyboardSkin inkWarm = KeyboardSkin.palette("ink", "水墨", false, "#F7EFE2", "#FFFFFF",
            "#E5E0D5", "#1F1F1F", "#6B6B6B", "#3A3A3A", "#FFFFFF");
        check(!ink.displayKey(autumn).equals(inkWarm.displayKey(autumn)),
            "a named skin whose colours changed must request a render");
        // 种子缺失时退回到皮肤自己的键，不抛异常。
        check(reloaded.displayKey(null).equals("system:false"), "a missing seed keys on the surface alone");

        System.out.println("Android keyboard skin display key guards passed");
    }
}
