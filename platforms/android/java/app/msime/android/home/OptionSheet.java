package app.msime.android.home;

import android.content.Context;
import android.content.res.ColorStateList;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.AccessibilityDelegateCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.R;
import com.google.android.material.bottomsheet.BottomSheetDialog;
import com.google.android.material.bottomsheet.BottomSheetDragHandleView;
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
        LinearLayout root = new LinearLayout(context);
        root.setOrientation(LinearLayout.VERTICAL);
        root.addView(new BottomSheetDragHandleView(context), new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        LinearLayout header = new LinearLayout(context);
        header.setOrientation(LinearLayout.VERTICAL);
        header.setGravity(Gravity.CENTER_HORIZONTAL);
        header.setPadding(Ui.dp(context, 16), 0, Ui.dp(context, 16), Ui.dp(context, 12));
        TextView heading = new TextView(context);
        heading.setText(title);
        heading.setGravity(Gravity.CENTER);
        Ui.style(heading, Ui.TEXT_SHEET_HEADER, 600, Ui.subText(context));
        heading.setAccessibilityHeading(true);
        header.addView(heading);
        if (subtitle != null && subtitle.length() > 0) {
            TextView note = new TextView(context);
            note.setText(subtitle);
            note.setGravity(Gravity.CENTER);
            Ui.style(note, Ui.TEXT_SHEET_HEADER, 400, Ui.subText(context));
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            params.topMargin = Ui.dp(context, 2);
            header.addView(note, params);
        }
        root.addView(header);

        options = new LinearLayout(context);
        options.setOrientation(LinearLayout.VERTICAL);
        NestedScrollView scroll = new NestedScrollView(context);
        scroll.addView(options, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        // 选项多到一屏放不下时，这一段滚动，标题和「取消」留在原处。
        root.addView(scroll, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));

        // 「取消」与选项之间一条页面底色的带子，代替设计里分开的两块卡片。
        View band = new View(context);
        band.setBackgroundColor(Ui.page(context));
        root.addView(band, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Ui.dp(context, 8)));
        root.addView(optionView("取消", false, false, false, Ui.accent(context), true, dialog::cancel));
        dialog.setContentView(root);
    }

    /** 一个普通选项；`selected` 为真时加粗并打 ✓。 */
    public OptionSheet option(CharSequence label, boolean selected, Runnable action) {
        addOption(optionView(label, selected, true, false, Ui.accent(context), selected, then(action)));
        return this;
    }

    /** 一个带下一级的选项：文字后面跟 ›，点了关掉本面板并打开 `next` 给出的面板。 */
    public OptionSheet submenu(CharSequence label, boolean selected, Supplier<OptionSheet> next) {
        addOption(optionView(label, selected, true, true, Ui.accent(context), selected,
            then(() -> next.get().show())));
        return this;
    }

    /** 一个破坏性选项，红色。 */
    public OptionSheet destructive(CharSequence label, Runnable action) {
        addOption(optionView(label, false, false, false, Ui.danger(context), false, then(action)));
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
            View rule = new View(context);
            rule.setBackgroundColor(Ui.hairline(context));
            options.addView(rule, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                Math.max(1, Ui.dp(context, 0.5f))));
        }
        options.addView(view);
        count++;
    }

    private View optionView(CharSequence label, boolean selected, boolean checkable, boolean nested,
            int color, boolean bold, Runnable action) {
        FrameLayout row = new FrameLayout(context);
        row.setMinimumHeight(Ui.dp(context, Ui.SHEET_OPTION_HEIGHT));
        row.setBackground(Ui.ripple(context));
        row.setClickable(true);
        row.setFocusable(true);
        row.setOnClickListener(ignored -> action.run());

        TextView text = new TextView(context);
        text.setText(nested ? label + " ›" : label);
        text.setGravity(Gravity.CENTER);
        Ui.style(text, Ui.TEXT_SHEET_OPTION, bold ? 600 : 400, color);
        FrameLayout.LayoutParams textParams = new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.CENTER);
        // 两侧各留出 ✓ 的位置，长选项不会压到它。
        textParams.leftMargin = Ui.dp(context, 48);
        textParams.rightMargin = Ui.dp(context, 48);
        textParams.topMargin = Ui.dp(context, 8);
        textParams.bottomMargin = Ui.dp(context, 8);
        row.addView(text, textParams);

        if (selected) {
            ImageView check = new ImageView(context);
            check.setImageResource(R.drawable.ms_w1_a2_check);
            check.setImageTintList(ColorStateList.valueOf(Ui.accent(context)));
            check.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
            FrameLayout.LayoutParams checkParams = new FrameLayout.LayoutParams(Ui.dp(context, 18), Ui.dp(context, 18),
                Gravity.CENTER_VERTICAL | Gravity.END);
            checkParams.setMarginEnd(Ui.dp(context, 20));
            row.addView(check, checkParams);
        }

        ViewCompat.setAccessibilityDelegate(row, new AccessibilityDelegateCompat() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfoCompat info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Button.class.getName());
                // 读屏不念「›」，下一级用文字说出来。
                info.setContentDescription(nested ? label + "，更多选项" : label);
                if (checkable && selected) info.setStateDescription("已选择");
            }
        });
        return row;
    }

}
