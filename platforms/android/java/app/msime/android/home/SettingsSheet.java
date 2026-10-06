package app.msime.android.home;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.widget.NestedScrollView;
import com.google.android.material.bottomsheet.BottomSheetDialog;
import com.google.android.material.bottomsheet.BottomSheetDragHandleView;

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
        LinearLayout root = new LinearLayout(context);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(24), 0, dp(24), dp(24));

        // 拖动条既是可见的把手，也给读屏提供「收起面板」的操作。
        BottomSheetDragHandleView handle = new BottomSheetDragHandleView(context);
        root.addView(handle, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        TextView heading = new TextView(context);
        heading.setText(title);
        // M3 headline small：面板标题是标题，不是加粗的标签。
        Ui.style(heading, Ui.TEXT_BAR_TITLE, 400, Ui.text(context));
        heading.setAccessibilityHeading(true);
        root.addView(heading);

        if (subtitle != null && !subtitle.isEmpty()) {
            TextView note = new TextView(context);
            note.setText(subtitle);
            Ui.style(note, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            params.topMargin = dp(4);
            root.addView(note, params);
        }

        content = new LinearLayout(context);
        content.setOrientation(LinearLayout.VERTICAL);
        NestedScrollView scroll = new NestedScrollView(context);
        scroll.addView(content, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        LinearLayout.LayoutParams scrollParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        scrollParams.topMargin = dp(12);
        root.addView(scroll, scrollParams);
        dialog.setContentView(root);
    }

    /** 所有行都加在这一列里。 */
    public LinearLayout content() { return content; }

    /** 行与行之间的 M3 组标题：强调色、14sp、500 字重。 */
    public void addHeading(String text) {
        TextView heading = new TextView(context);
        heading.setText(text);
        Ui.style(heading, Ui.TEXT_GROUP_TITLE, 500, Ui.accent(context));
        heading.setAccessibilityHeading(true);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.topMargin = dp(16);
        params.bottomMargin = dp(2);
        content.addView(heading, params);
    }

    /** 这一列末尾的脚注。 */
    public void addNote(String text) {
        TextView note = new TextView(context);
        note.setText(text);
        Ui.style(note, 12, 400, Ui.subText(context));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.topMargin = dp(14);
        content.addView(note, params);
    }

    /** 一行状态文字，保存成功或失败后由面板改写。 */
    public TextView addStatus() {
        TextView status = new TextView(context);
        Ui.style(status, 12, 400, Ui.subText(context));
        status.setGravity(Gravity.CENTER_VERTICAL);
        status.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, dp(20));
        params.topMargin = dp(10);
        content.addView(status, params);
        return status;
    }

    public void add(View row) {
        content.addView(row, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
    }

    public void show() { dialog.show(); }

    public void dismiss() { dialog.dismiss(); }

    /** 面板关闭时（不论怎么关的）运行 `action`，边改边存的面板借此写下还没保存的内容。 */
    public void setOnDismiss(Runnable action) { dialog.setOnDismissListener(ignored -> action.run()); }

    private int dp(int value) {
        return Ui.dp(context, value);
    }
}
