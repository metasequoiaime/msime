package app.msime.android;

import org.json.JSONObject;
import org.json.JSONArray;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Validated candidate presentation values consumed by the Android host. */
public final class CandidateAppearance {
    /** Maximum number of fallback fonts accepted for candidate rendering. */
    public static final int MAX_FALLBACK_FONTS = 32;

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
            : JsonPolicy.strictString(preferences.opt("candidate_font_family"));
        if (fontFamily == null) fontFamily = "Noto Sans SC";
        String englishFont = preferences == null ? ""
            : JsonPolicy.strictStringOrEmpty(preferences.opt("candidate_english_font"));
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
            parseColor(strip.secondary(), ColorPolicy.withAlpha(text, 0x9d)),
            parseColor(strip.accent(), text), 0, ColorPolicy.withAlpha(text, 0x0f),
            parseColor(strip.background(), 0xffffffff), 0,
            safeFont(fontFamily, "Noto Sans SC"), safeFont(englishFont, ""),
            safeFallbackFonts(fallbackFonts),
            parseColor(strip.candidateSelectedBackground(), parseColor(strip.keyBackground(), 0xffffffff)),
            parseColor(strip.candidateSelectedForeground(), parseColor(strip.accent(), text)));
    }

    /** 新设计里首选候选文字的字重（600）。 */
    public static final int SELECTED_FONT_WEIGHT = 600;

    /** `#RRGGBB` or Android's alpha-first `#AARRGGBB`, the two forms a keyboard skin carries. */
    private static int parseColor(String value, int fallback) {
        return ColorPolicy.parseHex(value, fallback);
    }

    private static List<String> fallbackFonts(JSONArray values) {
        if (values == null) return List.of("Noto Sans SC", "Microsoft YaHei");
        int limit = BoundsPolicy.bounded(values.length(), 0, MAX_FALLBACK_FONTS);
        ArrayList<String> result = new ArrayList<>(limit);
        for (int index = 0; index < limit; index++) {
            String value = JsonPolicy.strictStringOrEmpty(values.opt(index));
            if (validFont(value)) result.add(value);
        }
        return result;
    }

    private static List<String> safeFallbackFonts(List<String> values) {
        ArrayList<String> result = new ArrayList<>(values == null ? 0
            : BoundsPolicy.bounded(values.size(), 0, MAX_FALLBACK_FONTS));
        if (values != null) {
            for (String value : values) {
                if (result.size() >= MAX_FALLBACK_FONTS) break;
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
        return !TextPolicy.hasControl(value) && TextPolicy.validUnicode(value);
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
        private final int chip;
        private final int chipText;

        private Palette(String id, int text, int number, int accent, int selected,
                        int hover, int surface, int border, String fontFamily,
                        String englishFont, List<String> fallbackFonts, int chip, int chipText) {
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
            this.chip = chip;
            this.chipText = chipText;
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
        /** 新设计的首选候选 chip 底色：字母键的颜色（kb.key）。 */
        public int chip() { return chip; }
        /** 新设计的首选候选 chip 文字色：皮肤强调色，配合 {@link CandidateAppearance#SELECTED_FONT_WEIGHT}。 */
        public int chipText() { return chipText; }
        /** 新设计里某个候选的底色：首选为 {@link #chip()}，其余不填（0）。 */
        public int chipFor(boolean selected) { return selected ? chip : 0; }
        /** 新设计里某个候选的字重：首选 600，其余 400。 */
        public int weightFor(boolean selected) { return selected ? SELECTED_FONT_WEIGHT : 400; }
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
                + ":" + String.join(",", fallbackFonts) + ":" + Integer.toHexString(chip)
                + ":" + Integer.toHexString(chipText);
        }
    }
}
