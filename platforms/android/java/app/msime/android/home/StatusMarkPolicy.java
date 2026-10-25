package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.widget.TextView;
import app.msime.android.DrawablePolicy;
import app.msime.android.R;
import app.msime.android.ViewPolicy;

/** 安装检查共用的完成与警告状态标记策略。 */
public final class StatusMarkPolicy {
    private StatusMarkPolicy() {}

    /** 查询未知时使用中性标记，不显示警告或完成。 */
    public static void apply(TextView mark, Context context, Boolean done) {
        if (done != null) { apply(mark, context, done.booleanValue()); return; }
        mark.setText("?");
        ViewPolicy.setTextColor(mark, ThemeColorPolicy.text(context));
        ViewPolicy.setBackground(mark, DrawablePolicy.circle(ThemeColorPolicy.hairline(context)));
        ViewPolicy.hideFromAccessibility(mark);
    }

    /** 根据安装状态更新字形、前景色、圆形背景与无障碍可见性。 */
    public static void apply(TextView mark, Context context, boolean done) {
        mark.setText(done ? "✓" : "!");
        ViewPolicy.setTextColor(mark, done ? ThemeColorPolicy.onAccent(context) : 0xFFFFFFFF);
        ViewPolicy.setBackground(mark,
            DrawablePolicy.circle(done ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.color(context, R.attr.msWarn)));
        ViewPolicy.hideFromAccessibility(mark);
    }
}
