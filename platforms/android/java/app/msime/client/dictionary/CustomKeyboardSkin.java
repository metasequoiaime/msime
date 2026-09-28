package app.msime.client;

import java.util.Base64;
import java.util.Arrays;
import java.util.Locale;
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
        value.cornerRadius = bounded(object.optDouble("cornerRadius", value.cornerRadius), 0, 20, 8);
        value.borderWidth = bounded(object.optDouble("borderWidth", 0), 0, 2, 0);
        value.shadow = bounded(object.optDouble("shadow", 0), 0, .4, 0);
        value.pattern = KeyboardGeometry.bounded(object.optInt("pattern", 0), 0, 3);
        value.monospaced = object.optBoolean("monospaced", false);
        value.keyShape = oneOf(object.optString("keyShape", "rounded"),
            "rounded", "capsule", "ticket", "pebble");
        value.keyMaterial = oneOf(object.optString("keyMaterial", "flat"),
            "flat", "raised", "glass", "paper");
        value.keyOpacity = bounded(object.optDouble("keyOpacity", 1), .25, 1, 1);
        if (object.has("gradientEnd") && !object.isNull("gradientEnd"))
            value.gradientEnd = color(object, "gradientEnd", value.background);
        value.gradientHorizontal = object.optBoolean("gradientHorizontal", false);
        value.patternOpacity = bounded(object.optDouble("patternOpacity", .15), 0, .5, .15);
        if (object.has("customBorderColor") && !object.isNull("customBorderColor"))
            value.customBorderColor = color(object, "customBorderColor", value.accent);
        value.photoShade = bounded(object.optDouble("photoShade", .25), 0, .8, .25);
        value.photoPosition = bounded(object.optDouble("photoPosition", .5), 0, 1, .5);
        value.photo = photo(object.optString("photo", ""));
        return value;
    }

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
        value.cornerRadius = bounded(cornerRadius, 0, 20, 8);
        value.borderWidth = bounded(borderWidth, 0, 2, 0);
        value.shadow = bounded(shadow, 0, .4, 0);
        value.pattern = KeyboardGeometry.bounded(pattern, 0, 3);
        value.monospaced = monospaced;
        value.keyShape = oneOf(keyShape, "rounded", "capsule", "ticket", "pebble");
        value.keyMaterial = oneOf(keyMaterial, "flat", "raised", "glass", "paper");
        value.keyOpacity = bounded(keyOpacity, .25, 1, 1);
        value.gradientEnd = gradientEnd == null ? null : gradientEnd & 0xFFFFFF;
        value.gradientHorizontal = gradientHorizontal;
        value.patternOpacity = bounded(patternOpacity, 0, .5, .15);
        value.customBorderColor = customBorderColor == null ? null : customBorderColor & 0xFFFFFF;
        value.photo = photo != null && photo.length <= 512_000 && supportedPhoto(photo)
            ? photo.clone() : null;
        value.photoShade = bounded(photoShade, 0, .8, .25);
        value.photoPosition = bounded(photoPosition, 0, 1, .5);
        return value;
    }

    private static int color(JSONObject object, String key, int fallback) {
        return (int) object.optLong(key, fallback) & 0xFFFFFF;
    }

    private static double bounded(double value, double minimum, double maximum, double fallback) {
        return Double.isFinite(value) ? KeyboardGeometry.bounded(value, minimum, maximum) : fallback;
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

    private static String hex(int value) {
        return String.format(Locale.ROOT, "#%06X", value & 0xFFFFFF);
    }

    public String background() { return hex(background); }
    public String keyBackground() { return hex(keyBackground); }
    public String keyForeground() { return hex(keyForeground); }
    public String accent() { return hex(accent); }
    public String actionBackground() { return hex(actionBackground); }
    public String actionForeground() { return luminance(actionBackground) > .179 ? "#000000" : "#FFFFFF"; }
    public double cornerRadius() { return cornerRadius; }
    public double borderWidth() { return borderWidth; }
    public double shadow() { return shadow; }
    public int pattern() { return pattern; }
    public boolean monospaced() { return monospaced; }
    public String keyShape() { return keyShape; }
    public String keyMaterial() { return keyMaterial; }
    public double keyOpacity() { return keyOpacity; }
    public String gradientEnd() { return gradientEnd == null ? null : hex(gradientEnd); }
    public boolean gradientHorizontal() { return gradientHorizontal; }
    public double patternOpacity() { return patternOpacity; }
    public String borderColor() { return hex(customBorderColor == null ? accent : customBorderColor); }
    public byte[] photo() { return photo == null ? null : photo.clone(); }
    public double photoShade() { return photoShade; }
    public double photoPosition() { return photoPosition; }
    public String key() {
        return background + ":" + keyBackground + ":" + keyForeground + ":" + accent + ":"
            + actionBackground + ":" + cornerRadius + ":" + borderWidth + ":" + shadow + ":"
            + pattern + ":" + monospaced + ":" + keyShape + ":" + keyMaterial + ":"
            + keyOpacity + ":" + gradientEnd + ":" + gradientHorizontal + ":" + patternOpacity
            + ":" + customBorderColor + ":" + Arrays.hashCode(photo) + ":" + photoShade + ":"
            + photoPosition;
    }

    private static double luminance(int rgb) {
        return .2126 * channel(rgb >> 16) + .7152 * channel(rgb >> 8) + .0722 * channel(rgb);
    }

    private static double channel(int value) {
        double component = (value & 255) / 255.0;
        return component <= .04045 ? component / 12.92 : Math.pow((component + .055) / 1.055, 2.4);
    }
}
