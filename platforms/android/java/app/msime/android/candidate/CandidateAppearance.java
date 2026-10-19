package app.msime.android;

import org.json.JSONObject;
import org.json.JSONArray;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

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

    /** 从键盘配色和候选字体偏好生成默认外观；生效的候选包再通过 fromResolved 覆盖颜色。 */
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
        int text = ColorPolicy.parseHex(strip.keyForeground(), 0xff000000);
        return new Palette(strip.id(), text,
            ColorPolicy.parseHex(strip.secondary(), ColorPolicy.withAlpha(text, 0x9d)),
            ColorPolicy.parseHex(strip.accent(), text), 0, ColorPolicy.withAlpha(text, 0x0f),
            ColorPolicy.parseHex(strip.background(), 0xffffffff), 0,
            safeFont(fontFamily, "Noto Sans SC"), safeFont(englishFont, ""),
            safeFallbackFonts(fallbackFonts),
            ColorPolicy.parseHex(strip.candidateSelectedBackground(),
                ColorPolicy.parseHex(strip.keyBackground(), 0xffffffff)),
            ColorPolicy.parseHex(strip.candidateSelectedForeground(),
                ColorPolicy.parseHex(strip.accent(), text)));
    }

    /** 共享解析器确认当前包生效后，只覆盖候选栏，缺失色槽和字体仍取宿主的既有外观。 */
    public static Palette fromResolved(Palette fallback, String id, Map<String, String> colours) {
        return fromResolved(fallback, id, colours, null);
    }

    public static Palette fromResolved(Palette fallback, String id, Map<String, String> colours,
            CandidateCanvasAssets canvas) {
        int text = resolvedColour(colours, "text", fallback.text());
        int accent = resolvedColour(colours, "accent", fallback.accent());
        int selected = resolvedColour(colours, "selected", fallback.chip());
        int selectedText = resolvedColour(colours, "selected_text", accent);
        int number = resolvedColour(colours, "number", fallback.number());
        return new Palette(id, text,
            number, accent, selected,
            resolvedColour(colours, "hover", fallback.hover()),
            resolvedColour(colours, "surface", fallback.surface()),
            resolvedColour(colours, "border", fallback.border()),
            fallback.fontFamily(), fallback.englishFont(), fallback.fallbackFonts(),
            selected, selectedText,
            resolvedColour(colours, "secondary", number),
            resolvedColour(colours, "selected_number", selectedText), true, canvas, colours);
    }

    private static int resolvedColour(Map<String, String> colours, String name, int fallback) {
        Integer colour = ColorPolicy.contractArgb(colours.get(name));
        return colour == null ? fallback : colour;
    }

    /** 新设计里首选候选文字的字重（600）。 */
    public static final int SELECTED_FONT_WEIGHT = 600;

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
        return ListPolicy.copyOrEmpty(result);
    }

    private static String safeFont(String value, String fallback) {
        return validFont(value) ? value : fallback;
    }

    private static boolean validFont(String value) {
        return TextPolicy.boundedNonEmpty(value, 128);
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
        private final int secondary;
        private final int selectedNumber;
        private final boolean externalSkin;
        private final CandidateCanvasAssets canvas;
        private final Map<String, String> details;

        private Palette(String id, int text, int number, int accent, int selected,
                        int hover, int surface, int border, String fontFamily,
                        String englishFont, List<String> fallbackFonts, int chip, int chipText) {
            this(id, text, number, accent, selected, hover, surface, border, fontFamily,
                englishFont, fallbackFonts, chip, chipText, number, number, false);
        }

        private Palette(String id, int text, int number, int accent, int selected,
                        int hover, int surface, int border, String fontFamily,
                        String englishFont, List<String> fallbackFonts, int chip, int chipText,
                        int secondary, int selectedNumber, boolean externalSkin) {
            this(id, text, number, accent, selected, hover, surface, border, fontFamily,
                englishFont, fallbackFonts, chip, chipText, secondary, selectedNumber, externalSkin, null, Map.of());
        }

        private Palette(String id, int text, int number, int accent, int selected,
                        int hover, int surface, int border, String fontFamily,
                        String englishFont, List<String> fallbackFonts, int chip, int chipText,
                        int secondary, int selectedNumber, boolean externalSkin, CandidateCanvasAssets canvas,
                        Map<String, String> details) {
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
            this.secondary = secondary;
            this.selectedNumber = selectedNumber;
            this.externalSkin = externalSkin;
            this.canvas = canvas;
            this.details = Map.copyOf(details);
        }

        public String id() { return id; }
        public int text() { return text; }
        public int number() { return number; }
        public int numberFor(boolean selected) { return selected ? selectedNumber : number; }
        public int secondaryFor(boolean selected) {
            return selected ? resolvedColour(details, "selected_translation", ColorPolicy.withAlpha(chipText, 0.58f)) : secondary;
        }
        public int preeditText() { return resolvedColour(details, "preedit_text", number); }
        public int preeditCaret(int fallback) { return resolvedColour(details, "preedit_caret", fallback); }
        public int accent() { return accent; }
        public int selected() { return selected; }
        public int hover() { return hover; }
        public int surface() { return surface; }
        public int border() { return border; }
        public String fontFamily() { return fontFamily; }
        public String englishFont() { return englishFont; }
        public List<String> fallbackFonts() { return fallbackFonts; }
        /** 首选候选底色：候选包的选中填充，未使用包时为字母键的颜色。 */
        public int chip() { return chip; }
        /** 首选候选文字色：包的选中文字色，未使用包时为皮肤强调色。 */
        public int chipText() { return chipText; }
        /** 是否使用外部候选包，展开网格据此使用候选底色而非键帽底色。 */
        public boolean externalSkin() { return externalSkin; }
        public CandidateCanvasAssets canvas() { return canvas; }
        /** 新设计里某个候选的底色：首选为 {@link #chip()}，其余不填（0）。 */
        public int chipFor(boolean selected) { return selected ? chip : 0; }
        /** 新设计里某个候选的字重：首选 600，其余 400。 */
        public int weightFor(boolean selected) { return selected ? SELECTED_FONT_WEIGHT : 400; }
        public String preferredFont() { return englishFont.isEmpty() ? fontFamily : englishFont; }

        /** 包可指定选中文字色；未使用包时保留候选条的强调色文字。 */
        public int textFor(boolean selected) {
            return selected ? (externalSkin ? chipText : accent) : text;
        }

        public String key() {
            return id + ":" + Integer.toHexString(text) + ":" + Integer.toHexString(number)
                + ":" + Integer.toHexString(accent) + ":" + Integer.toHexString(selected)
                + ":" + Integer.toHexString(hover) + ":" + Integer.toHexString(surface)
                + ":" + Integer.toHexString(border) + ":" + fontFamily + ":" + englishFont
                + ":" + String.join(",", fallbackFonts) + ":" + Integer.toHexString(chip)
                + ":" + Integer.toHexString(chipText) + ":" + externalSkin
                + ":" + Integer.toHexString(secondary) + ":" + Integer.toHexString(selectedNumber)
                + ":" + preeditText() + ":" + preeditCaret(accent) + ":" + secondaryFor(true)
                + ":" + (canvas == null ? "" : canvas.key());
        }
    }
}
