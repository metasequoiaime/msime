package app.msime.android;

import android.content.Context;

/** 新增低高度范围按原 75% 键帽的 40 dp 同步缩小文字，保留原范围的排版。 */
final class CompactKeyTypography {
    private CompactKeyTypography() { }

    static float scale(Context context, int heightPixels) {
        if (heightPixels <= 0) return 1f;
        return Math.min(1f, heightPixels / DimensionPolicy.floatPixels(context, 40));
    }
}
