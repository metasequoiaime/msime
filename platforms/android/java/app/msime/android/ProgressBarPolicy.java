package app.msime.android;

import android.content.res.ColorStateList;
import android.widget.ProgressBar;

/** Android 进度条共用的状态与外观策略。 */
public final class ProgressBarPolicy {
    private ProgressBarPolicy() {}

    /** 为不确定进度指示器应用单色着色。 */
    public static void setIndeterminateTint(ProgressBar view, int color) {
        view.setIndeterminateTintList(ColorStateList.valueOf(color));
    }
}
