package app.msime.android.home;

import android.content.Context;
import app.msime.android.DimensionPolicy;
import app.msime.android.ThemeColorPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;

/** 设置页复用键盘彩蛋场景，由 Activity 承载 Material 对话框。 */
final class AboutEasterEgg {
    private AboutEasterEgg() {}

    static void show(Context context) {
        int horizontal = DimensionPolicy.pixels(context, 12);
        new MaterialAlertDialogBuilder(context)
            .setTitle("水杉的小秘密")
            .setView(app.msime.android.AboutEasterEgg.scene(context, ThemeColorPolicy.page(context)),
                horizontal, 0, horizontal, 0)
            .setPositiveButton("好", null)
            .show();
    }
}
