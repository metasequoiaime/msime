package app.msime.android;

import java.util.Base64;
import java.util.Arrays;
import org.json.JSONException;
import org.json.JSONObject;

/** Bounded Android view of Apple's current custom touch-keyboard design. */
public final class CustomKeyboardSkin {
    private int background = 0xE8F0EB;
    private int keyBackground = 0xFFFFFF;
    private int keyForeground = 0x17251D;
    private int accent = 0x185C47;
    private int actionBackground = 0x185C47;
    private double cornerRadius = 8;
    private double borderWidth;
    private double shadow;
    private int pattern;
    private boolean monospaced;
    private String keyShape = "rounded";
    private String keyMaterial = "flat";
    private double keyOpacity = 1;
    private Integer gradientEnd;
    private boolean gradientHorizontal;
    private double patternOpacity = .15;
    private Integer customBorderColor;
    private byte[] photo;
    private double photoShade = .25;
    private double photoPosition = .5;
    private String soundPack = DEFAULT_SOUND_PACK;
    private String pressAnimation = DEFAULT_PRESS_ANIMATION;

    /** 缺省按键音包：沿用系统按键音（plan P23）。 */
    public static final String DEFAULT_SOUND_PACK = "default";
    /** 「静音」：应用这个皮肤时关掉 Android 本地的按键音开关，`plugins.key_sound.pack` 保持原样（plan P23）。 */
    public static final String SILENT_SOUND_PACK = "silent";
    /** 缺省按键动画：保持现有按压态。 */
    public static final String DEFAULT_PRESS_ANIMATION = "none";

    private CustomKeyboardSkin() {}

    public static CustomKeyboardSkin defaults() { return new CustomKeyboardSkin(); }

    public static CustomKeyboardSkin from(JSONObject object) {
        CustomKeyboardSkin value = defaults();
        if (object == null) return value;
        value.background = color(object, "background", value.background);
        value.keyBackground = color(object, "keyBackground", value.keyBackground);
        value.keyForeground = color(object, "keyForeground", value.keyForeground);
        value.accent = color(object, "accent", value.accent);
        value.actionBackground = color(object, "actionBackground", value.actionBackground);
        value.cornerRadius = BoundsPolicy.bounded(doubleValue(object.opt("cornerRadius"), value.cornerRadius), 0, 20, 8);
        value.borderWidth = BoundsPolicy.bounded(doubleValue(object.opt("borderWidth"), 0), 0, 2, 0);
        value.shadow = BoundsPolicy.bounded(doubleValue(object.opt("shadow"), 0), 0, .4, 0);
        value.pattern = patternValue(object.opt("pattern"));
        value.monospaced = JsonPolicy.strictBoolean(object.opt("monospaced"), false);
        value.keyShape = oneOf(JsonPolicy.strictStringOrEmpty(object.opt("keyShape")),
            "rounded", "capsule", "ticket", "pebble");
        value.keyMaterial = oneOf(JsonPolicy.strictStringOrEmpty(object.opt("keyMaterial")),
            "flat", "raised", "glass", "paper");
        value.keyOpacity = BoundsPolicy.bounded(doubleValue(object.opt("keyOpacity"), 1), .25, 1, 1);
        if (object.has("gradientEnd") && !object.isNull("gradientEnd"))
            value.gradientEnd = color(object, "gradientEnd", value.background);
        value.gradientHorizontal = JsonPolicy.strictBoolean(object.opt("gradientHorizontal"), false);
        value.patternOpacity = BoundsPolicy.bounded(doubleValue(object.opt("patternOpacity"), .15), 0, .5, .15);
        if (object.has("customBorderColor") && !object.isNull("customBorderColor"))
            value.customBorderColor = color(object, "customBorderColor", value.accent);
        value.photoShade = BoundsPolicy.bounded(doubleValue(object.opt("photoShade"), .25), 0, .8, .25);
        value.photoPosition = BoundsPolicy.bounded(doubleValue(object.opt("photoPosition"), .5), 0, 1, .5);
        value.photo = photo(JsonPolicy.strictStringOrEmpty(object.opt("photo")));
        value.soundPack = soundPackValue(object.opt("soundPack"));
        value.pressAnimation = pressAnimationValue(object.opt("pressAnimation"));
        return value;
    }

    /** 按键音包 id：小写字母、数字和连字符，1–64 个字符；其他值（含非字符串）为 {@link #DEFAULT_SOUND_PACK}。 */
    static String soundPackValue(Object raw) {
        if (!(raw instanceof String)) return DEFAULT_SOUND_PACK;
        String value = (String) raw;
        return value.matches("[a-z0-9][a-z0-9-]{0,63}") ? value : DEFAULT_SOUND_PACK;
    }

    /** 按键动画：`none|bounce|ripple|glow|lift`，其他值为 {@link #DEFAULT_PRESS_ANIMATION}。 */
    static String pressAnimationValue(Object raw) {
        if (!(raw instanceof String)) return DEFAULT_PRESS_ANIMATION;
        return switch ((String) raw) {
            case "bounce", "ripple", "glow", "lift" -> (String) raw;
            default -> DEFAULT_PRESS_ANIMATION;
        };
    }

    /** 同一个设计换上新的按键音包与按键动画（各自按 {@link #from} 的规则校验）。 */
    public CustomKeyboardSkin withFeedback(String soundPack, String pressAnimation) {
        CustomKeyboardSkin value = copy();
        value.soundPack = soundPackValue(soundPack);
        value.pressAnimation = pressAnimationValue(pressAnimation);
        return value;
    }

    private CustomKeyboardSkin copy() {
        CustomKeyboardSkin value = defaults();
        value.background = background;
        value.keyBackground = keyBackground;
        value.keyForeground = keyForeground;
        value.accent = accent;
        value.actionBackground = actionBackground;
        value.cornerRadius = cornerRadius;
        value.borderWidth = borderWidth;
        value.shadow = shadow;
        value.pattern = pattern;
        value.monospaced = monospaced;
        value.keyShape = keyShape;
        value.keyMaterial = keyMaterial;
        value.keyOpacity = keyOpacity;
        value.gradientEnd = gradientEnd;
        value.gradientHorizontal = gradientHorizontal;
        value.patternOpacity = patternOpacity;
        value.customBorderColor = customBorderColor;
        value.photo = photo;
        value.photoShade = photoShade;
        value.photoPosition = photoPosition;
        value.soundPack = soundPack;
        value.pressAnimation = pressAnimation;
        return value;
    }

    /**
     * 写回 `custom_theme.keyboard` 的 JSON 形式，{@link #from} 读回得到同一个设计。
     *
     * @param includePhoto false 时不带照片（导出设计参数时用）
     */
    public JSONObject toJson(boolean includePhoto) {
        JSONObject object = new JSONObject();
        try {
            object.put("background", background)
                .put("keyBackground", keyBackground)
                .put("keyForeground", keyForeground)
                .put("accent", accent)
                .put("actionBackground", actionBackground)
                .put("cornerRadius", cornerRadius)
                .put("borderWidth", borderWidth)
                .put("shadow", shadow)
                .put("pattern", pattern)
                .put("monospaced", monospaced)
                .put("keyShape", keyShape)
                .put("keyMaterial", keyMaterial)
                .put("keyOpacity", keyOpacity)
                .put("gradientHorizontal", gradientHorizontal)
                .put("patternOpacity", patternOpacity)
                .put("photoShade", photoShade)
                .put("photoPosition", photoPosition)
                .put("soundPack", soundPack)
                .put("pressAnimation", pressAnimation);
            if (gradientEnd != null) object.put("gradientEnd", gradientEnd.intValue());
            if (customBorderColor != null)
                object.put("customBorderColor", customBorderColor.intValue());
            if (includePhoto && photo != null)
                object.put("photo", Base64.getEncoder().encodeToString(photo));
        } catch (JSONException error) {
            // 键都是非空字符串常量、值都是有限数或字符串，org.json 只在空键或非有限数时抛出。
            throw new IllegalStateException(error);
        }
        return object;
    }

    /** 这个设计是否带照片。 */
    public boolean hasPhoto() { return photo != null; }

    static CustomKeyboardSkin fixture(int background, int keyBackground, int keyForeground,
            int accent, int actionBackground, double cornerRadius, double borderWidth,
            double shadow, int pattern, boolean monospaced, String keyShape,
            String keyMaterial, double keyOpacity, Integer gradientEnd,
            boolean gradientHorizontal, double patternOpacity, Integer customBorderColor,
            byte[] photo, double photoShade, double photoPosition) {
        CustomKeyboardSkin value = defaults();
        value.background = background & 0xFFFFFF;
        value.keyBackground = keyBackground & 0xFFFFFF;
        value.keyForeground = keyForeground & 0xFFFFFF;
        value.accent = accent & 0xFFFFFF;
        value.actionBackground = actionBackground & 0xFFFFFF;
        value.cornerRadius = BoundsPolicy.bounded(cornerRadius, 0, 20, 8);
        value.borderWidth = BoundsPolicy.bounded(borderWidth, 0, 2, 0);
        value.shadow = BoundsPolicy.bounded(shadow, 0, .4, 0);
        value.pattern = BoundsPolicy.bounded(pattern, 0, 3);
        value.monospaced = monospaced;
        value.keyShape = oneOf(keyShape, "rounded", "capsule", "ticket", "pebble");
        value.keyMaterial = oneOf(keyMaterial, "flat", "raised", "glass", "paper");
        value.keyOpacity = BoundsPolicy.bounded(keyOpacity, .25, 1, 1);
        value.gradientEnd = gradientEnd == null ? null : gradientEnd & 0xFFFFFF;
        value.gradientHorizontal = gradientHorizontal;
        value.patternOpacity = BoundsPolicy.bounded(patternOpacity, 0, .5, .15);
        value.customBorderColor = customBorderColor == null ? null : customBorderColor & 0xFFFFFF;
        value.photo = photo != null && photo.length <= 512_000 && supportedPhoto(photo)
            ? photo.clone() : null;
        value.photoShade = BoundsPolicy.bounded(photoShade, 0, .8, .25);
        value.photoPosition = BoundsPolicy.bounded(photoPosition, 0, 1, .5);
        return value;
    }

    private static int color(JSONObject object, String key, int fallback) {
        return colorValue(object.opt(key), fallback);
    }

    static int colorValue(Object raw, int fallback) {
        int value = NumberPolicy.strictInt(raw, fallback);
        return value < 0 || value > 0xFFFFFF ? fallback : value;
    }

    static int patternValue(Object raw) {
        return BoundsPolicy.bounded(NumberPolicy.strictInt(raw, 0), 0, 3);
    }

    static double doubleValue(Object raw, double fallback) {
        return NumberPolicy.strictDouble(raw, fallback);
    }

    private static String oneOf(String value, String first, String second, String third, String fourth) {
        if (second.equals(value) || third.equals(value) || fourth.equals(value)) return value;
        return first;
    }

    private static byte[] photo(String encoded) {
        if (encoded.isEmpty() || encoded.length() > 682_668) return null;
        try {
            byte[] bytes = Base64.getDecoder().decode(encoded);
            return bytes.length <= 512_000 && supportedPhoto(bytes) ? bytes : null;
        } catch (IllegalArgumentException error) {
            return null;
        }
    }

    private static boolean supportedPhoto(byte[] bytes) {
        if (bytes.length >= 3 && (bytes[0] & 0xFF) == 0xFF
                && (bytes[1] & 0xFF) == 0xD8 && (bytes[2] & 0xFF) == 0xFF) return true;
        if (starts(bytes, new byte[] {(byte) 0x89, 'P', 'N', 'G', 13, 10, 26, 10})) return true;
        if (starts(bytes, "GIF87a".getBytes(java.nio.charset.StandardCharsets.US_ASCII))
                || starts(bytes, "GIF89a".getBytes(java.nio.charset.StandardCharsets.US_ASCII))) return true;
        return bytes.length >= 12 && starts(bytes, new byte[] {'R', 'I', 'F', 'F'})
            && bytes[8] == 'W' && bytes[9] == 'E' && bytes[10] == 'B' && bytes[11] == 'P';
    }

    private static boolean starts(byte[] bytes, byte[] prefix) {
        if (bytes.length < prefix.length) return false;
        for (int index = 0; index < prefix.length; index++)
            if (bytes[index] != prefix[index]) return false;
        return true;
    }

    public String background() { return ColorPolicy.hexRgb(background); }
    public String keyBackground() { return ColorPolicy.hexRgb(keyBackground); }
    public String keyForeground() { return ColorPolicy.hexRgb(keyForeground); }
    public String accent() { return ColorPolicy.hexRgb(accent); }
    public String actionBackground() { return ColorPolicy.hexRgb(actionBackground); }
    public String actionForeground() { return luminance(actionBackground) > .179 ? "#000000" : "#FFFFFF"; }
    public double cornerRadius() { return cornerRadius; }
    public double borderWidth() { return borderWidth; }
    public double shadow() { return shadow; }
    public int pattern() { return pattern; }
    public boolean monospaced() { return monospaced; }
    public String keyShape() { return keyShape; }
    public String keyMaterial() { return keyMaterial; }
    public double keyOpacity() { return keyOpacity; }
    public String gradientEnd() {
        return gradientEnd == null ? null : ColorPolicy.hexRgb(gradientEnd);
    }
    public boolean gradientHorizontal() { return gradientHorizontal; }
    public double patternOpacity() { return patternOpacity; }
    public String borderColor() {
        return ColorPolicy.hexRgb(customBorderColor == null ? accent : customBorderColor);
    }
    public byte[] photo() { return photo == null ? null : photo.clone(); }
    public double photoShade() { return photoShade; }
    public double photoPosition() { return photoPosition; }
    /** 应用这个皮肤时写进 `plugins.key_sound.pack` 的按键音包。 */
    public String soundPack() { return soundPack; }
    /** 应用这个皮肤时写进 `touch_key_animation` 的按键动画。 */
    public String pressAnimation() { return pressAnimation; }
    public String key() {
        return background + ":" + keyBackground + ":" + keyForeground + ":" + accent + ":"
            + actionBackground + ":" + cornerRadius + ":" + borderWidth + ":" + shadow + ":"
            + pattern + ":" + monospaced + ":" + keyShape + ":" + keyMaterial + ":"
            + keyOpacity + ":" + gradientEnd + ":" + gradientHorizontal + ":" + patternOpacity
            + ":" + customBorderColor + ":" + Arrays.hashCode(photo) + ":" + photoShade + ":"
            + photoPosition + ":" + soundPack + ":" + pressAnimation;
    }

    private static double luminance(int rgb) {
        return .2126 * channel(rgb >> 16) + .7152 * channel(rgb >> 8) + .0722 * channel(rgb);
    }

    private static double channel(int value) {
        double component = (value & 255) / 255.0;
        return component <= .04045 ? component / 12.92 : Math.pow((component + .055) / 1.055, 2.4);
    }
}
