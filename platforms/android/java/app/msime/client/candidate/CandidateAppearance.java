package app.msime.client;

import org.json.JSONObject;
import org.json.JSONArray;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Validated candidate presentation values consumed by the Android host. */
public final class CandidateAppearance {
    private CandidateAppearance() {}

    public static boolean isHorizontal(String layout) {
        return "horizontal".equals(layout);
    }

    public static int fontSize(int value) {
        return value >= 12 && value <= 32 ? value : 16;
    }

    public static Palette from(JSONObject preferences, boolean systemDark) {
        String theme = preferences == null ? "follow"
            : preferences.optString("candidate_theme", "follow");
        String globalTheme = preferences == null ? "system"
            : preferences.optString("theme", "system");
        String skin = preferences == null ? "willow_green"
            : preferences.optString("candidate_skin", "willow_green");
        String fontFamily = preferences == null ? "Noto Sans SC"
            : preferences.optString("candidate_font_family", "Noto Sans SC");
        String englishFont = preferences == null ? ""
            : preferences.optString("candidate_english_font", "");
        JSONArray fallback = preferences == null ? null
            : preferences.optJSONArray("candidate_fallback_fonts");
        List<String> fallbackFonts = fallbackFonts(fallback);
        return fromValues(skin, theme, globalTheme, systemDark,
            preferences == null ? "" : preferences.optString("candidate_text_color", ""),
            preferences == null ? "" : preferences.optString("candidate_number_color", ""),
            preferences == null ? "" : preferences.optString("candidate_accent_color", ""),
            preferences == null ? "" : preferences.optString("candidate_selected_color", ""),
            preferences == null ? "" : preferences.optString("candidate_hover_color", ""),
            preferences == null ? "" : preferences.optString("candidate_surface_color", ""),
            preferences == null ? "" : preferences.optString("candidate_border_color", ""),
            fontFamily, englishFont, fallbackFonts);
    }

    /** Value-only resolver used by host smoke tests without an Android JSON runtime. */
    public static Palette fromValues(String skin, String candidateTheme, String globalTheme,
                                     boolean systemDark, String textColor, String numberColor,
                                     String accentColor, String selectedColor, String hoverColor,
                                     String surfaceColor, String borderColor) {
        return fromValues(skin, candidateTheme, globalTheme, systemDark, textColor, numberColor,
            accentColor, selectedColor, hoverColor, surfaceColor, borderColor,
            "Noto Sans SC", "", List.of("Noto Sans SC", "Microsoft YaHei"));
    }

    public static Palette fromValues(String skin, String candidateTheme, String globalTheme,
                                     boolean systemDark, String textColor, String numberColor,
                                     String accentColor, String selectedColor, String hoverColor,
                                     String surfaceColor, String borderColor, String fontFamily,
                                     String englishFont, List<String> fallbackFonts) {
        boolean dark = resolveDark(candidateTheme, globalTheme, systemDark);
        Palette palette = builtIn(skin, dark);
        String primaryFont = safeFont(fontFamily, "Noto Sans SC");
        String preferredEnglishFont = safeFont(englishFont, "");
        List<String> safeFallbackFonts = safeFallbackFonts(fallbackFonts);
        int text = override(textColor, palette.text);
        int number = override(numberColor,
            hasColor(textColor) ? withAlpha(text, 0x9d) : palette.number);
        return palette.with(
            text, number,
            override(accentColor, palette.accent),
            override(selectedColor, palette.selected),
            override(hoverColor, palette.hover),
            override(surfaceColor, palette.surface),
            override(borderColor, palette.border), primaryFont, preferredEnglishFont,
            safeFallbackFonts);
    }

    private static boolean resolveDark(String candidateTheme, String globalTheme,
                                       boolean systemDark) {
        if ("dark".equals(candidateTheme)) return true;
        if ("light".equals(candidateTheme)) return false;
        if ("dark".equals(globalTheme)) return true;
        if ("light".equals(globalTheme)) return false;
        return systemDark;
    }

    private static Palette builtIn(String value, boolean dark) {
        String id = value == null ? "willow_green" : value;
        return switch (id) {
            case "fluent" -> dark
                ? palette(id, 0xffe9e8e8, 0xffe9e89d, 0x2e9b9b9b,
                    0xb93e3e3e, 0xff414141, 0xff202020)
                : palette(id, 0xff1a1a1a, 0x8c1a1a1a, 0x1f000000,
                    0xffe8e8e8, 0xffececec, 0xffffffff);
            case "wechat" -> dark
                ? palette(id, 0xffb7b7b7, 0xff858585, 0xff292929,
                    0xff07c160, 0x5207c160, 0xff151515)
                : palette(id, 0xff333333, 0xff757575, 0xffdedede,
                    0xff07c160, 0x2407c160, 0xfff7f7f7);
            case "graphite" -> dark
                ? palette(id, 0xffaeb6c2, 0xff707987, 0xff30353b,
                    0x00000000, 0x0effffff, 0xff1c1f23)
                : palette(id, 0xff586476, 0xff8993a1, 0xffe2e5e9,
                    0x00000000, 0x0e1f2937, 0xfffbfbfc);
            case "willow_green" -> dark
                ? palette(id, 0xffd8dbd8, 0xffa6aba7, 0x00000000,
                    0xff65c98d, 0x3865c98d, 0xff2d2f2e)
                : palette(id, 0xff343936, 0xff686f6a, 0x00000000,
                    0xff58b980, 0x2958b980, 0xfff4f5f3);
            default -> builtIn("willow_green", dark);
        };
    }

    private static Palette palette(String id, int text, int number, int border,
                                   int selected, int hover, int surface) {
        int accent = selected == 0 ? text : selected;
        return new Palette(id, text, number, accent, selected, hover, surface, border);
    }

    private static boolean hasColor(String value) {
        return parseColor(value, Integer.MIN_VALUE) != Integer.MIN_VALUE;
    }

    private static int override(String value, int fallback) {
        return parseColor(value, fallback);
    }

    private static int parseColor(String value, int fallback) {
        if (value == null || !value.matches("#[0-9a-fA-F]{6}")) return fallback;
        try { return 0xff000000 | Integer.parseInt(value.substring(1), 16); }
        catch (NumberFormatException ignored) { return fallback; }
    }

    private static int withAlpha(int color, int alpha) {
        return (alpha << 24) | (color & 0x00ffffff);
    }

    private static List<String> fallbackFonts(JSONArray values) {
        if (values == null) return List.of("Noto Sans SC", "Microsoft YaHei");
        ArrayList<String> result = new ArrayList<>();
        for (int index = 0; index < Math.min(values.length(), 32); index++) {
            String value = values.optString(index, "");
            if (validFont(value)) result.add(value);
        }
        return result;
    }

    private static List<String> safeFallbackFonts(List<String> values) {
        ArrayList<String> result = new ArrayList<>();
        if (values != null) {
            for (String value : values) {
                if (result.size() >= 32) break;
                if (validFont(value)) result.add(value);
            }
        }
        if (result.isEmpty()) return List.of("Noto Sans SC", "Microsoft YaHei");
        return Collections.unmodifiableList(result);
    }

    private static String safeFont(String value, String fallback) {
        return validFont(value) ? value : fallback;
    }

    private static boolean validFont(String value) {
        if (value == null || value.isEmpty()
                || TextPolicy.utf8Length(value) > 128) return false;
        return !TextPolicy.hasControl(value);
    }

    public static final class Palette {
        private final String id;
        private final int text;
        private final int number;
        private final int accent;
        private final int selected;
        private final int hover;
        private final int surface;
        private final int border;
        private final String fontFamily;
        private final String englishFont;
        private final List<String> fallbackFonts;

        private Palette(String id, int text, int number, int accent, int selected,
                        int hover, int surface, int border) {
            this(id, text, number, accent, selected, hover, surface, border,
                "Noto Sans SC", "", List.of("Noto Sans SC", "Microsoft YaHei"));
        }

        private Palette(String id, int text, int number, int accent, int selected,
                        int hover, int surface, int border, String fontFamily,
                        String englishFont, List<String> fallbackFonts) {
            this.id = id;
            this.text = text;
            this.number = number;
            this.accent = accent;
            this.selected = selected;
            this.hover = hover;
            this.surface = surface;
            this.border = border;
            this.fontFamily = fontFamily;
            this.englishFont = englishFont;
            this.fallbackFonts = fallbackFonts;
        }

        private Palette with(int text, int number, int accent, int selected, int hover,
                             int surface, int border, String fontFamily, String englishFont,
                             List<String> fallbackFonts) {
            return new Palette(id, text, number, accent, selected, hover, surface, border,
                fontFamily, englishFont, fallbackFonts);
        }

        public String id() { return id; }
        public int text() { return text; }
        public int number() { return number; }
        public int accent() { return accent; }
        public int selected() { return selected; }
        public int hover() { return hover; }
        public int surface() { return surface; }
        public int border() { return border; }
        public String fontFamily() { return fontFamily; }
        public String englishFont() { return englishFont; }
        public List<String> fallbackFonts() { return fallbackFonts; }
        public String preferredFont() { return englishFont.isEmpty() ? fontFamily : englishFont; }

        public int textFor(boolean selected) {
            if (!selected || alpha(this.selected) == 0) return text;
            return contrast(this.selected);
        }

        public String key() {
            return id + ":" + Integer.toHexString(text) + ":" + Integer.toHexString(number)
                + ":" + Integer.toHexString(accent) + ":" + Integer.toHexString(selected)
                + ":" + Integer.toHexString(hover) + ":" + Integer.toHexString(surface)
                + ":" + Integer.toHexString(border) + ":" + fontFamily + ":" + englishFont
                + ":" + String.join(",", fallbackFonts);
        }

        private static int contrast(int color) {
            double luminance = (0.299 * red(color) + 0.587 * green(color)
                + 0.114 * blue(color)) / 255.0;
            return luminance > .62 ? 0xff000000 : 0xffffffff;
        }

        private static int alpha(int color) { return (color >>> 24) & 0xff; }
        private static int red(int color) { return (color >>> 16) & 0xff; }
        private static int green(int color) { return (color >>> 8) & 0xff; }
        private static int blue(int color) { return color & 0xff; }
    }
}
