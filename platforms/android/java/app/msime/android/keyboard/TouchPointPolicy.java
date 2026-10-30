package app.msime.android;

/**
 * 26 键双拼的按下点（#10018）：宿主这一侧不依赖 Android 视图的部分。按键区在每根手指按下时按键记下按下点（{@link KeyboardKeyArea}），字母键被点击时取出并清掉；只有 26 键双拼的小写字母和微软双拼的 `;` 才把它连同几何代次交给引擎（`msime_client_touch_character`），其余按键照常走 `msime_client_character`。几何按 `msime_client_set_key_geometry` 的 JSON 写出（{@link #geometryJson}）。宿主不算距离，纠错怎么计价归引擎。
 *
 * <p>按下点按键存而不按指针存：点击在抬起时才发生，那时已经不知道是哪根手指；多指连按时每个键各自留着自己那一下。只存最近 {@link #SLOTS} 个键。坐标只在内存里，不落盘、不进日志。
 */
public final class TouchPointPolicy {
    /** 按下到点击超过这么久，按下点作废：没有点击的按下（滑出键外抬起）留下的按下点不能被很久以后的一次无障碍点击捡走。 */
    public static final long MAX_AGE_MS = 2000;
    /** 同时留着按下点的键数上限，够十根手指。 */
    public static final int SLOTS = 10;

    private final Object[] keys = new Object[SLOTS];
    private final float[] xs = new float[SLOTS];
    private final float[] ys = new float[SLOTS];
    private final long[] times = new long[SLOTS];

    /** 记下 `key` 这一次按下的位置（按键区坐标）和时刻（`SystemClock.uptimeMillis` 的毫秒）；同一个键之前没被取走的那一下作废。没有空位时覆盖按下最早的那个键：还按着的键总比早已抬起却没被取走的那些按下新，不会被后来的手指挤掉。 */
    public void record(Object key, float x, float y, long time) {
        if (key == null) return;
        int slot = slotOf(key);
        if (slot < 0) slot = slotOf(null);
        if (slot < 0) {
            slot = 0;
            for (int other = 1; other < SLOTS; other++) if (times[other] < times[slot]) slot = other;
        }
        keys[slot] = key;
        xs[slot] = x;
        ys[slot] = y;
        times[slot] = time;
    }

    /** 丢掉 `key` 留着的按下点：这一下被手势吞掉了（滑出符号、长按选项、按下后滑离太远）。 */
    public void forget(Object key) {
        if (key == null) return;
        int slot = slotOf(key);
        if (slot >= 0) keys[slot] = null;
    }

    /** 丢掉全部按下点：整次触摸被取消，或者被滑行接管，或者键行重建。 */
    public void clear() {
        java.util.Arrays.fill(keys, null);
    }

    /**
     * 取出并清掉 `key` 留着的按下点，写进 `point`（x、y）。没有、超过 {@link #MAX_AGE_MS}、时刻倒流或坐标不是有限数时返回 false；无论结果如何，这个键的按下点都已清掉。
     */
    public boolean take(Object key, long now, float[] point) {
        if (key == null) return false;
        int slot = slotOf(key);
        if (slot < 0) return false;
        keys[slot] = null;
        long age = now - times[slot];
        if (age < 0 || age > MAX_AGE_MS) return false;
        if (!Float.isFinite(xs[slot]) || !Float.isFinite(ys[slot])) return false;
        point[0] = xs[slot];
        point[1] = ys[slot];
        return true;
    }

    private int slotOf(Object key) {
        for (int slot = 0; slot < SLOTS; slot++) if (keys[slot] == key) return slot;
        return -1;
    }

    /**
     * 这一键是否带按下点走 `msime_client_touch_character`：显示的是 26 键的字母层，引擎在跑双拼、没有开字母网格（`View.key_grid` 为 `none`），不在专用英文和本地模式里，送的是小写字母或 `;`、没有 Shift，并且当前会话注册过几何（`geometryId` 非 0）。辅助码的大写字母、14/18 键、九键、实体键盘不走这里；`;` 只有在调用方已经判定它是微软双拼的韵母时才会送到这里。
     *
     * @param standardLetters 显示的是标准 26 键的字母层（不是符号层、九键、手写、笔画、韩文或注音键面）
     */
    public static boolean usesTouch(boolean standardLetters, int scheme, String keyGrid,
            boolean dedicatedEnglish, String localMode, char output, boolean shifted, int geometryId) {
        return standardLetters && scheme == InputSchemeTraits.SHUANGPIN && "none".equals(keyGrid)
            && !dedicatedEnglish && "none".equals(localMode) && !shifted && geometryId != 0
            && ((output >= 'a' && output <= 'z') || output == ';');
    }

    /** 抬起点（键自己的坐标）是否还在键上：和 `View` 判断点击是否成立一样，四边各放宽 `slop`。候选可见时宿主替离键太远的抬起补的那次点击不带按下点。 */
    public static boolean releasedOnKey(float x, float y, int width, int height, float slop) {
        return x >= -slop && y >= -slop && x < width + slop && y < height + slop;
    }

    /**
     * `msime_client_set_key_geometry` 的请求：`{"id":…,"keys":[[cx,cy,w,h]×26，按 a..z],"semicolon":[cx,cy,w,h]|null,"duplicates":[["g",cx,cy,w,h],…]}`。矩形都按 left、top、right、bottom 给（按键区坐标），这里换成中心和宽高。
     *
     * @param letters 'a'..'z' 各键的矩形，共 104 个数
     * @param semicolon 微软双拼 `;` 键的矩形（4 个数），不显示时为 null
     * @param duplicateLetters 分离键盘里重复出现的字母，前 `duplicateCount` 个有效
     * @param duplicates 这些重复键的矩形，每个 4 个数
     */
    public static String geometryJson(int id, float[] letters, float[] semicolon,
            char[] duplicateLetters, float[] duplicates, int duplicateCount) {
        if (id <= 0) throw new IllegalArgumentException("Key geometry id must be positive");
        if (letters.length != 26 * 4) throw new IllegalArgumentException("26 letter key rectangles expected");
        if (semicolon != null && semicolon.length != 4) throw new IllegalArgumentException("One semicolon rectangle expected");
        if (duplicateCount < 0 || duplicateCount > duplicateLetters.length || duplicateCount * 4 > duplicates.length)
            throw new IllegalArgumentException("Duplicate key count out of range");
        StringBuilder json = new StringBuilder(64 + (27 + duplicateCount) * 40);
        json.append("{\"id\":").append(id).append(",\"keys\":[");
        for (int letter = 0; letter < 26; letter++) {
            if (letter > 0) json.append(',');
            json.append('[');
            appendRect(json, letters, letter * 4);
            json.append(']');
        }
        json.append("],\"semicolon\":");
        if (semicolon == null) {
            json.append("null");
        } else {
            json.append('[');
            appendRect(json, semicolon, 0);
            json.append(']');
        }
        json.append(",\"duplicates\":[");
        for (int index = 0; index < duplicateCount; index++) {
            char letter = duplicateLetters[index];
            if (letter < 'a' || letter > 'z') throw new IllegalArgumentException("Duplicate keys must be letters");
            if (index > 0) json.append(',');
            json.append("[\"").append(letter).append("\",");
            appendRect(json, duplicates, index * 4);
            json.append(']');
        }
        return json.append("]}").toString();
    }

    private static void appendRect(StringBuilder json, float[] rects, int at) {
        float left = rects[at];
        float top = rects[at + 1];
        float right = rects[at + 2];
        float bottom = rects[at + 3];
        if (!(right > left) || !(bottom > top)) throw new IllegalArgumentException("Key rectangles must have a positive size");
        json.append(number((left + right) / 2)).append(',').append(number((top + bottom) / 2)).append(',')
            .append(number(right - left)).append(',').append(number(bottom - top));
    }

    private static String number(float value) {
        if (!Float.isFinite(value)) throw new IllegalArgumentException("Key geometry must be finite");
        return Float.toString(value);
    }
}
