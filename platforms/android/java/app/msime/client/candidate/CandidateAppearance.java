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

    /**
     * The touch candidate strip for one resolved keyboard skin, with the font preferences.
     *
     * <p>The strip draws from the keyboard palette, as the shared theme contract says: background, key text, `secondary` for numbers and translations, and `accent` for the selected candidate's text, which carries no fill.
     */
    public static Palette from(JSONObject preferences, KeyboardSkin strip) {
        String fontFamily = preferences == null ? "Noto Sans SC"
            : preferences.optString("candidate_font_family", "Noto Sans SC");
        String englishFont = preferences == null ? ""
            : preferences.optString("candidate_english_font", "");
        JSONArray fallback = preferences == null ? null
            : preferences.optJSONArray("candidate_fallback_fonts");
        return fromSkin(strip, fontFamily, englishFont, fallbackFonts(fallback));
    }

    /** Value-only resolver used by host smoke tests without an Android JSON runtime. */
    public static Palette fromSkin(KeyboardSkin strip) {
        return fromSkin(strip, "Noto Sans SC", "", List.of("Noto Sans SC", "Microsoft YaHei"));
    }

    public static Palette fromSkin(KeyboardSkin strip, String fontFamily, String englishFont,
                                   List<String> fallbackFonts) {
        int text = parseColor(strip.keyForeground(), 0xff000000);
        return new Palette(strip.id(), text,
            parseColor(strip.secondary(), withAlpha(text, 0x9d)),
            parseColor(strip.accent(), text), 0, withAlpha(text, 0x0f),
            parseColor(strip.background(), 0xffffffff), 0,
            safeFont(fontFamily, "Noto Sans SC"), safeFont(englishFont, ""),
            safeFallbackFonts(fallbackFonts));
    }

    /** `#RRGGBB` or Android's alpha-first `#AARRGGBB`, the two forms a keyboard skin carries. */
    private static int parseColor(String value, int fallback) {
        if (value == null || !value.matches("#[0-9a-fA-F]{6}|#[0-9a-fA-F]{8}")) return fallback;
        long parsed = Long.parseLong(value.substring(1), 16);
        return value.length() == 7 ? 0xff000000 | (int) parsed : (int) parsed;
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

        /** The selected candidate is told apart by its accent text alone; the strip draws no fill. */
        public int textFor(boolean selected) {
            return selected ? accent : text;
        }

        public String key() {
            return id + ":" + Integer.toHexString(text) + ":" + Integer.toHexString(number)
                + ":" + Integer.toHexString(accent) + ":" + Integer.toHexString(selected)
                + ":" + Integer.toHexString(hover) + ":" + Integer.toHexString(surface)
                + ":" + Integer.toHexString(border) + ":" + fontFamily + ":" + englishFont
                + ":" + String.join(",", fallbackFonts);
        }
    }
}
