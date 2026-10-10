package app.msime.android.home;

import android.content.Context;
import android.widget.TextView;
import app.msime.android.DrawablePolicy;
import app.msime.android.R;
import app.msime.android.ViewPolicy;

/** 安装检查共用的完成与警告状态标记策略。 */
public final class StatusMarkPolicy {
    private StatusMarkPolicy() {}

    /** 根据安装状态更新字形、前景色、圆形背景与无障碍可见性。 */
    public static void apply(TextView mark, Context context, boolean done) {
        mark.setText(done ? "✓" : "!");
        ViewPolicy.setTextColor(mark, done ? Ui.onAccent(context) : 0xFFFFFFFF);
        ViewPolicy.setBackground(mark,
            DrawablePolicy.circle(done ? Ui.accent(context) : Ui.color(context, R.attr.msWarn)));
        ViewPolicy.hideFromAccessibility(mark);
    }
}
