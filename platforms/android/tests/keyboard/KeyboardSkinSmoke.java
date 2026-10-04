package app.msime.android;

public final class KeyboardSkinSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // `system` is the design's Material 3 keyboard, in the mode the host asked for.
        KeyboardSkin light = KeyboardSkin.system(false);
        check("system".equals(light.id()) && "跟随系统".equals(light.title()) && !light.dark());
        check("#E6EAE2".equals(light.background()));
        check("#FFFFFF".equals(light.keyBackground()));
        check("#CFE9D6".equals(light.functionBackground()));
        check("#CFE9D6".equals(light.actionBackground()));
        check("#191C19".equals(light.keyForeground()));
        check("#191C19".equals(light.actionForeground()));
        check("#414941".equals(light.secondary()));
        check("#2C7A4B".equals(light.accent()) && "#2C7A4B".equals(light.accentText()));
        check("#2C7A4B".equals(light.returnBackground()) && "#FFFFFF".equals(light.returnForeground()));
        check("#CFE9D6".equals(light.accentSoft()));
        check(light.cornerRadius() == 8 && light.borderWidth() == 0 && light.shadowOpacity() == 0);
        check(!light.designed() && light.pattern() == 0 && light.photo() == null);
        check("#80FFFFFF".equals(light.sidebarBackground()));

        KeyboardSkin dark = KeyboardSkin.system(true);
        check(dark.dark());
        check("#1D201D".equals(dark.background()) && "#343833".equals(dark.keyBackground()));
        check("#2A4F37".equals(dark.functionBackground()) && "#E1E3DE".equals(dark.keyForeground()));
        check("#C0C9BF".equals(dark.secondary()) && "#8FD5A6".equals(dark.accent()));
        check("#8FD5A6".equals(dark.returnBackground()) && "#003920".equals(dark.returnForeground()));
        check(!light.key().equals(dark.key()));

        // A resolved built-in palette (the shuishan row of the shared colour table).
        KeyboardSkin shuishan = KeyboardSkin.palette("shuishan", "水杉", true, "#1E1F1C",
            "#2F302C", "#23241F", "#FFFFFF", "#9FB5A3", "#7FE08E", "#000000");
        check("shuishan".equals(shuishan.id()) && "水杉".equals(shuishan.title()) && shuishan.dark());
        check("#1E1F1C".equals(shuishan.background()) && "#2F302C".equals(shuishan.keyBackground()));
        check("#23241F".equals(shuishan.functionBackground()) && "#FFFFFF".equals(shuishan.keyForeground()));
        check("#9FB5A3".equals(shuishan.secondary()) && "#7FE08E".equals(shuishan.accent()));
        check("#000000".equals(shuishan.onAccent()));
        // 确认 and the switched-on tiles keep the platform accent in every theme, as iOS and Harmony draw them; the theme accent is only the selected strip candidate.
        check("#8FD5A6".equals(shuishan.returnBackground()) && "#003920".equals(shuishan.returnForeground()));
        check("#2A4F37".equals(shuishan.accentSoft()) && "#8FD5A6".equals(shuishan.accentText()));
        KeyboardSkin ink = KeyboardSkin.palette("ink", "水墨", false, "#F4F1EA", "#FFFFFF",
            "#E4DFD4", "#1A1A1A", "#6B6B6B", "#FFFFFF", "#000000");
        check("#2C7A4B".equals(ink.returnBackground()) && "#FFFFFF".equals(ink.returnForeground()));
        check("#CFE9D6".equals(ink.accentSoft()) && "#2C7A4B".equals(ink.accentText()));
        check("#2A4F37".equals(dark.accentSoft()) && "#8FD5A6".equals(dark.accentText()));
        check(shuishan.cornerRadius() == 8);

        // Alpha-last contract colours become Android's alpha-first form; null and junk slots fall
        // back to the Material 3 token for that slot, never to transparent.
        check("#99FFFFFF".equals(KeyboardSkin.androidColor("#FFFFFF99")));
        check("#12ABCDEF".equals(KeyboardSkin.androidColor("#abcdef12")));
        check("#ABCDEF".equals(KeyboardSkin.androidColor("#abcdef")));
        check(KeyboardSkin.androidColor("red") == null && KeyboardSkin.androidColor("#12345") == null);
        check(KeyboardSkin.androidColor(null) == null);
        KeyboardSkin partial = KeyboardSkin.palette("custom", "自定义", false, null, "#FFFFFF",
            "bad", "#1A1A1A", "#1A1A1A99", null, null);
        check("#E6EAE2".equals(partial.background()) && "#CFE9D6".equals(partial.functionBackground()));
        check("#991A1A1A".equals(partial.secondary()) && "#2C7A4B".equals(partial.accent()));
        check("#FFFFFF".equals(partial.onAccent()));
        check("#801A1A1A".equals(KeyboardSkin.palette("x", "x", false, null, "#1A1A1A40",
            null, null, null, null, null).sidebarBackground()));

        check(KeyboardSkin.resolveDark("dark", "light", false));
        check(!KeyboardSkin.resolveDark("light", "dark", true));
        check(KeyboardSkin.resolveDark("follow", "dark", false));
        check(!KeyboardSkin.resolveDark("follow", "light", true));
        check(KeyboardSkin.resolveDark("follow", "system", true));
        check(!KeyboardSkin.resolveDark("follow", "system", false));
        // The emoji and handwriting panels resolve their own surface setting through the same
        // rule. An unknown or missing value has to read as 跟随颜色模式, not as an explicit light:
        // an older snapshot would otherwise flip those two panels while the keyboard stayed dark.
        check(KeyboardSkin.resolveDark("unknown", "dark", false));
        check(KeyboardSkin.resolveDark("", "system", true));
        check(!KeyboardSkin.resolveDark("", "light", true));

        CustomKeyboardSkin design = CustomKeyboardSkin.fixture(0x151022, 0x291E40, 0xFFFFFF,
            0xD4BBFF, 0xFFFFFF, 17, 1.5, .25, 3, true, "pebble", "glass", .45,
            0x30224A, true, .2, 0xA987E8,
            new byte[] {(byte) 0xFF, (byte) 0xD8, (byte) 0xFF, 0}, .8, 1);
        check(CustomKeyboardSkin.colorValue(Integer.valueOf(0x123456), 0) == 0x123456);
        check(CustomKeyboardSkin.colorValue(Long.valueOf(0xABCDEF), 0) == 0xABCDEF);
        check(CustomKeyboardSkin.colorValue(Double.valueOf(1.5), 0x13579B) == 0x13579B);
        check(CustomKeyboardSkin.colorValue(Boolean.TRUE, 0x2468AC) == 0x2468AC);
        check(CustomKeyboardSkin.colorValue("123456", 0x369CF0) == 0x369CF0);
        check(CustomKeyboardSkin.patternValue(2) == 2);
        check(CustomKeyboardSkin.patternValue(2.5) == 0);
        check(CustomKeyboardSkin.patternValue("2") == 0);
        check(CustomKeyboardSkin.patternValue(true) == 0);
        check(CustomKeyboardSkin.patternValue(10) == 3);
        check(CustomKeyboardSkin.doubleValue(8.5, 0) == 8.5);
        check(CustomKeyboardSkin.doubleValue("8.5", 7) == 7);
        check(CustomKeyboardSkin.doubleValue(Boolean.TRUE, 7) == 7);
        check(CustomKeyboardSkin.doubleValue(Double.NaN, 7) == 7);
        KeyboardSkin custom = KeyboardSkin.customFixture(design, false);
        check("custom".equals(custom.id()) && "我的皮肤".equals(custom.title()) && custom.designed());
        check("#151022".equals(custom.background()) && "#30224A".equals(custom.gradientEnd()));
        check(custom.gradientHorizontal() && custom.patternOpacity() == .2);
        check("pebble".equals(custom.keyShape()) && "glass".equals(custom.keyMaterial()));
        check(custom.keyOpacity() == .45 && custom.cornerRadius() == 17);
        check("#A987E8".equals(custom.borderColor()) && "#000000".equals(custom.actionForeground()));
        check("#D4BBFF".equals(custom.accent()) && "#000000".equals(custom.onAccent()));
        // A keyboard design keeps its own accent on 确认 and the switched-on tiles.
        check("#D4BBFF".equals(custom.returnBackground()) && "#000000".equals(custom.returnForeground()));
        check("#24D4BBFF".equals(custom.accentSoft()) && "#D4BBFF".equals(custom.accentText()));
        // The shared flattening's hint colour: the key text at 0x99.
        check("#99FFFFFF".equals(custom.secondary()));
        check(custom.photo().length == 4 && custom.photoShade() == .8 && custom.photoPosition() == 1);

        // The resolver's answer for each kind of global theme. A custom theme with a design is resolved without that design (themeRequest leaves it out), so the keyboard it returns is the base theme's: here shuishan's, whose grey-green hint colour must not reach the design.
        java.util.Map<String, String> shuishanSlots = new java.util.HashMap<>();
        shuishanSlots.put("background", "#1E1F1C");
        shuishanSlots.put("key", "#2F302C");
        shuishanSlots.put("function_key", "#23241F");
        shuishanSlots.put("text", "#FFFFFF");
        shuishanSlots.put("secondary", "#9FB5A3");
        shuishanSlots.put("accent", "#7FE08E");
        shuishanSlots.put("on_accent", "#000000");
        CustomKeyboardSkin lightDesign = CustomKeyboardSkin.fixture(0xFFF4EC, 0xFFFFFF, 0x5A2E1F,
            0xE8866A, 0xF6D5C5, 12, 0, 0, 0, false, "rounded", "flat", 1,
            null, false, .15, null, null, .25, .5);
        KeyboardSkin designed = KeyboardSkin.resolved("custom", "自定义", "dark", false,
            shuishanSlots, lightDesign);
        check(designed.designed() && designed.dark() && "#FFF4EC".equals(designed.background()));
        check("#995A2E1F".equals(designed.secondary()));
        check("#5A2E1F".equals(designed.keyForeground()));
        // Without a design the custom theme is its flattened palette, secondary included.
        KeyboardSkin flatCustom = KeyboardSkin.resolved("custom", "自定义", null, true,
            shuishanSlots, null);
        check(!flatCustom.designed() && "#9FB5A3".equals(flatCustom.secondary()));
        // A built-in theme maps its keyboard slots, alpha-last colours included, and a fixed appearance overrides the host's mode.
        java.util.Map<String, String> nightSlots = new java.util.HashMap<>(shuishanSlots);
        nightSlots.put("secondary", "#FFFFFF99");
        nightSlots.put("accent", null);
        KeyboardSkin night = KeyboardSkin.resolved("night", "夜间", "dark", false, nightSlots, null);
        check("night".equals(night.id()) && "夜间".equals(night.title()) && night.dark());
        check("#1E1F1C".equals(night.background()) && "#2F302C".equals(night.keyBackground()));
        check("#23241F".equals(night.functionBackground()) && "#99FFFFFF".equals(night.secondary()));
        check("#8FD5A6".equals(night.accent()) && "#000000".equals(night.onAccent()));
        check(!KeyboardSkin.resolved("paper", "纸", "light", true, shuishanSlots, null).dark());
        check(KeyboardSkin.resolved("paper", "纸", null, true, shuishanSlots, null).dark());
        // No keyboard in the answer: `system` is the Material 3 keyboard, anything else keeps its own id and title over the same tokens.
        KeyboardSkin bare = KeyboardSkin.resolved("system", "跟随系统", null, true, null, null);
        check("system".equals(bare.id()) && "跟随系统".equals(bare.title()) && bare.dark());
        check("#1D201D".equals(bare.background()));
        KeyboardSkin named = KeyboardSkin.resolved("ink", "水墨", null, false, null, null);
        check("ink".equals(named.id()) && "水墨".equals(named.title()));
        check("#E6EAE2".equals(named.background()) && !named.designed());
        System.out.println("Android keyboard skins: Material 3 tokens, resolved theme palettes, "
            + "alpha-last colours, null-slot fallback, custom materials, photos, mode resolution and "
            + "resolver answer mapping passed");
    }
}
