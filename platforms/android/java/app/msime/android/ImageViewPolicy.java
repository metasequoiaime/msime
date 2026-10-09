package app.msime.android;

import android.content.res.ColorStateList;
import android.widget.ImageView;
import androidx.annotation.ColorInt;

/** Android 图片控件共用的状态与外观策略。 */
public final class ImageViewPolicy {
    private ImageViewPolicy() {}

    /** 通过平台颜色状态列表为图片应用单色着色。 */
    public static void setTint(ImageView view, @ColorInt int color) {
        view.setImageTintList(ColorStateList.valueOf(color));
    }
}
