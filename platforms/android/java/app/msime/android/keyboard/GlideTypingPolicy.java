package app.msime.android;

/**
 * 滑行输入：一笔滑过若干字母键，抬手时把键位和轨迹交给引擎（`msime_client_glide`），由引擎解出最可能的全拼字母并像打字一样写进组字。这里是宿主这一侧不依赖 Android 视图的判定：什么时候可以滑行、一次按下什么时候变成滑行、请求怎么写。
 */
public final class GlideTypingPolicy {
    /** 手指离按下点的横向距离要达到这么多个键宽、并且停在另一个字母键上，才算滑行。只看横向，是为了让在同一个键上纵向滑动的「滑动输入符号」照常工作。 */
    public static final float START_KEY_WIDTHS = 0.4f;
    /** 一次请求最多带的轨迹点数，与 `msime_client_glide` 的上限一致；更多的点均匀抽稀，首尾两点总保留。 */
    public static final int MAX_POINTS = 1024;
    private static final int LETTERS = 26;

    private GlideTypingPolicy() { }

    /**
     * 这一次按下能不能开始滑行：设置打开、显示的是标准 26 键的字母层、方案是全拼（View.scheme 为 0）、不在专用英文和本地模式里，并且 26 个字母键都在。
     *
     * @param standardLetters 字母层的 26 键（不是韩文、注音键面，也不是九键、手写、笔画或符号层）
     * @param letterKeys 当前画出来的字母键个数
     */
    public static boolean armed(boolean enabled, boolean standardLetters, int scheme,
            boolean dedicatedEnglish, String localMode, int letterKeys) {
        return enabled && standardLetters && scheme == 0 && !dedicatedEnglish
            && "none".equals(localMode) && letterKeys == LETTERS;
    }

    /**
     * 按着的手指是否已经构成滑行。
     *
     * @param downLetter 按下时所在的字母键（'a'..'z'）
     * @param currentLetter 手指现在所在的字母键，不在字母键上（例如键距里）时为 0
     */
    public static boolean starts(char downLetter, char currentLetter, float downX, float x, float keyWidth) {
        return currentLetter != 0 && currentLetter != downLetter && keyWidth > 0
            && Math.abs(x - downX) >= START_KEY_WIDTHS * keyWidth;
    }

    /**
     * `msime_client_glide` 的请求。坐标都在同一个坐标系里（按键区自己的），由宿主决定。
     *
     * @param centers 'a'..'z' 各键的中心，依次是 x、y，共 52 个数
     * @param xs 轨迹各点的横坐标，前 `count` 个有效
     * @param times 轨迹各点距按下的毫秒数
     */
    public static String request(float[] centers, float keyWidth, float keyHeight, float[] xs,
            float[] ys, long[] times, int count) {
        if (centers.length != LETTERS * 2) throw new IllegalArgumentException("26 letter key centres expected");
        if (count < 2 || count > xs.length || count > ys.length || count > times.length)
            throw new IllegalArgumentException("A glide stroke needs at least two points");
        StringBuilder json = new StringBuilder(64 + LETTERS * 24 + Math.min(count, MAX_POINTS) * 28);
        json.append("{\"keys\":[");
        for (int letter = 0; letter < LETTERS; letter++) {
            if (letter > 0) json.append(',');
            json.append('[').append(number(centers[letter * 2])).append(',')
                .append(number(centers[letter * 2 + 1])).append(']');
        }
        json.append("],\"key_width\":").append(number(keyWidth))
            .append(",\"key_height\":").append(number(keyHeight)).append(",\"points\":[");
        int[] picked = sample(count, MAX_POINTS);
        for (int index = 0; index < picked.length; index++) {
            int point = picked[index];
            if (index > 0) json.append(',');
            json.append('[').append(number(xs[point])).append(',').append(number(ys[point]))
                .append(',').append(Math.max(0, times[point])).append(']');
        }
        return json.append("]}").toString();
    }

    /** `count` 个点里均匀取至多 `limit` 个的下标，首尾两点总在其中，次序不变。 */
    static int[] sample(int count, int limit) {
        int size = Math.min(count, limit);
        int[] picked = new int[size];
        if (size == count) {
            for (int index = 0; index < size; index++) picked[index] = index;
            return picked;
        }
        for (int index = 0; index < size; index++)
            picked[index] = (int) ((long) index * (count - 1) / (size - 1));
        return picked;
    }

    private static String number(float value) {
        if (!Float.isFinite(value)) throw new IllegalArgumentException("Glide coordinates must be finite");
        return Float.toString(value);
    }
}
