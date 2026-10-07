package app.msime.android;

/**
 * 手写面板读的 `touch_handwriting`：书写模式、识别等待时间、识别后显示拼音、笔迹颜色与粗细。范围与 client-core 的校验和手写设置页一致：等待 200–1500 ms，粗细 1–8 px；缺键或越界时按默认（叠写、500 ms、显示拼音、跟随皮肤、3 px）。
 *
 * <p>只做取值与换算，不碰 JSON 以外的状态；JSON 由调用方取出各字段后传进来。
 */
public record HandwritingPreferences(Mode mode, int delayMillis, boolean showPinyin, String strokeColor,
                                     int strokeWidth) {
    public static final int DELAY_MIN = 200;
    public static final int DELAY_MAX = 1500;
    public static final int DELAY_DEFAULT = 500;
    public static final int WIDTH_MIN = 1;
    public static final int WIDTH_MAX = 8;
    public static final int WIDTH_DEFAULT = 3;

    /** 书写模式：单字一字一识别；叠写停笔后识别并自动上屏首选；行写整行一次识别并带上文。 */
    public enum Mode {
        SINGLE("single"), OVERLAP("overlap"), LINE("line");

        private final String preference;

        Mode(String preference) { this.preference = preference; }

        public String preference() { return preference; }

        public static Mode fromPreference(String value) {
            for (Mode mode : values()) {
                if (mode.preference.equals(value)) return mode;
            }
            return OVERLAP;
        }
    }

    public static HandwritingPreferences defaults() {
        return new HandwritingPreferences(Mode.OVERLAP, DELAY_DEFAULT, true, "follow_skin", WIDTH_DEFAULT);
    }

    /** 由各字段的原始值构造，越界的数字夹到范围内，未知的颜色按跟随皮肤。 */
    public static HandwritingPreferences of(String mode, int delayMillis, boolean showPinyin, String strokeColor,
                                            int strokeWidth) {
        String color = switch (strokeColor == null ? "" : strokeColor) {
            case "black", "white", "blue" -> strokeColor;
            default -> "follow_skin";
        };
        return new HandwritingPreferences(Mode.fromPreference(mode),
            BoundsPolicy.bounded(delayMillis, DELAY_MIN, DELAY_MAX), showPinyin, color,
            BoundsPolicy.bounded(strokeWidth, WIDTH_MIN, WIDTH_MAX));
    }

    /** 笔迹颜色（ARGB）；跟随皮肤时为 null。蓝色取 Material Blue 700。 */
    public Integer inkColor() {
        return switch (strokeColor) {
            case "black" -> 0xFF000000;
            case "white" -> 0xFFFFFFFF;
            case "blue" -> 0xFF1976D2;
            default -> null;
        };
    }

    /**
     * 键盘服务自己的停笔防抖之外还要再等多久（毫秒）：服务固定等 {@code serviceDebounce}，用户要的更长时补上差值；更短时无法提前，按服务的时间识别。
     */
    public long extraDelay(long serviceDebounce) {
        return BoundsPolicy.nonNegative(delayMillis - serviceDebounce);
    }
}
