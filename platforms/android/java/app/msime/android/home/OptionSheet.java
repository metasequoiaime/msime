package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import androidx.core.widget.NestedScrollView;
import app.msime.android.KeyboardGeometry;
import com.google.android.material.bottomsheet.BottomSheetDialog;
import java.util.function.Supplier;

/**
 * 选择面板：M3 modal bottom sheet 里一个居中的小标题（可带副标题）、一列选项和末尾的「取消」。
 *
 * <p>排版照设计的 Android 选择面板：标题 13sp/600、副标题 13sp，都是次要文字色；选项 56dp 高、19sp、强调色居中，当前项加粗并在右侧 20dp 处打 ✓；带下一级的选项在文字后面跟一个 ›，点了换成下一级面板（可以一层层嵌套）；破坏性选项（例如「移除粤语」）用红色。外壳按用户的决定换成 M3 modal bottom sheet：顶角 28、底色 `colorSurfaceContainerLow`、带拖动条，所以季节主题和深色模式都只靠主题属性。
 *
 * <p>点选项先关掉面板再执行动作，动作里再弹别的面板或对话框不会叠在这一层上面。
 */
public final class OptionSheet {
    private final Context context;
    private final BottomSheetDialog dialog;
    private final LinearLayout options;
    private int count;

    public OptionSheet(Context context, CharSequence title, @Nullable CharSequence subtitle) {
        this.context = context;
        dialog = new BottomSheetDialog(context);
        LinearLayout root = Ui.column(context);
        root.addView(Ui.sheetDragHandle(context));

        LinearLayout header = SheetHeaderView.create(context, title, subtitle);
        root.addView(header);

        options = Ui.column(context);
        NestedScrollView scroll = new NestedScrollView(context);
        scroll.addView(options, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        // 选项多到一屏放不下时，这一段滚动，标题和「取消」留在原处。
        root.addView(scroll, KeyboardGeometry.weightedWidthParams(1f));

        // 「取消」与选项之间一条页面底色的带子，代替设计里分开的两块卡片。
        root.addView(Ui.sheetSeparator(context));
        root.addView(SheetOptionView.create(context, "取消", false, false, Ui.accent(context), true,
            dialog::cancel));
        dialog.setContentView(root);
    }

    /** 一个普通选项；`selected` 为真时加粗并打 ✓。 */
    public OptionSheet option(CharSequence label, boolean selected, Runnable action) {
        addOption(SheetOptionView.create(context, label, selected, false, Ui.accent(context), selected,
            then(action)));
        return this;
    }

    /** 一个带下一级的选项：文字后面跟 ›，点了关掉本面板并打开 `next` 给出的面板。 */
    public OptionSheet submenu(CharSequence label, boolean selected, Supplier<OptionSheet> next) {
        addOption(SheetOptionView.create(context, label, selected, true, Ui.accent(context), selected,
            then(() -> next.get().show())));
        return this;
    }

    /** 一个破坏性选项，红色。 */
    public OptionSheet destructive(CharSequence label, Runnable action) {
        addOption(SheetOptionView.create(context, label, false, false, Ui.danger(context), false,
            then(action)));
        return this;
    }

    public void show() { dialog.show(); }

    public void dismiss() { dialog.dismiss(); }

    private Runnable then(Runnable action) {
        return () -> {
            dialog.dismiss();
            action.run();
        };
    }

    private void addOption(View view) {
        if (count > 0) {
            options.addView(Ui.divider(context, true));
        }
        options.addView(view);
        count++;
    }

}
