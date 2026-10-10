package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import app.msime.android.ViewPolicy;
import androidx.annotation.Nullable;
import androidx.core.widget.NestedScrollView;
import com.google.android.material.bottomsheet.BottomSheetDialog;

/**
 * 设置项的 M3 modal bottom sheet：拖动条、一个标题、可选的说明，下面一列内容。
 *
 * <p>面板本身用 Material 3 的默认外观：顶角 28dp、底色 `colorSurfaceContainerLow`（设计令牌里的 andCard），所以季节主题换的就是这里的颜色。外壳集中在这里，各处的面板不必各自为一个标题和一个滚动区写布局文件。纯选择列表用 {@link OptionSheet}。
 */
public final class SettingsSheet {
    private final BottomSheetDialog dialog;
    private final LinearLayout content;
    private final Context context;

    public SettingsSheet(Context context, String title, @Nullable String subtitle) {
        this.context = context;
        dialog = new BottomSheetDialog(context);
        LinearLayout root = Ui.column(context);
        Ui.setPaddingDp(root, context, 24, 0, 24, 24);

        // 拖动条既是可见的把手，也给读屏提供「收起面板」的操作。
        root.addView(Ui.sheetDragHandle(context));

        // M3 headline small：面板标题是标题，不是加粗的标签。
        TextView heading = Ui.headingLabel(context, title, Ui.TEXT_BAR_TITLE, 400, ThemeColorPolicy.text(context));
        root.addView(heading);

        if (subtitle != null && !subtitle.isEmpty()) {
            TextView note = Ui.styledLabel(context, subtitle, Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.subText(context));
            LinearLayout.LayoutParams params = Ui.matchWidth(context, 4);
            root.addView(note, params);
        }

        content = Ui.column(context);
        NestedScrollView scroll = new NestedScrollView(context);
        scroll.addView(content, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        LinearLayout.LayoutParams scrollParams = Ui.matchWidth(context, 12);
        root.addView(scroll, scrollParams);
        dialog.setContentView(root);
    }

    /** 所有行都加在这一列里。 */
    public LinearLayout content() { return content; }

    /** 行与行之间的 M3 组标题：强调色、14sp、500 字重。 */
    public void addHeading(String text) {
        TextView heading = Ui.groupHeading(context, text);
        LinearLayout.LayoutParams params = Ui.matchWidth();
        params.topMargin = Ui.dp(context, 16);
        params.bottomMargin = Ui.dp(context, 2);
        content.addView(heading, params);
    }

    /** 这一列末尾的脚注。 */
    public void addNote(String text) {
        TextView note = Ui.styledLabel(context, text, 12, 400, ThemeColorPolicy.subText(context));
        LinearLayout.LayoutParams params = Ui.matchWidth();
        params.topMargin = Ui.dp(context, 14);
        content.addView(note, params);
    }

    /** 一行状态文字，保存成功或失败后由面板改写。 */
    public TextView addStatus() {
        TextView status = Ui.liveStatus(context, 12);
        ViewPolicy.setCenteredVertically(status);
        LinearLayout.LayoutParams params = Ui.matchWidthHeight(context, 20);
        params.topMargin = Ui.dp(context, 10);
        content.addView(status, params);
        return status;
    }

    public void add(View row) {
        content.addView(row, Ui.matchWidth());
    }

    public void show() { dialog.show(); }

    public void dismiss() { dialog.dismiss(); }

    /** 面板关闭时（不论怎么关的）运行 `action`，边改边存的面板借此写下还没保存的内容。 */
    public void setOnDismiss(Runnable action) { dialog.setOnDismissListener(ignored -> action.run()); }

}
