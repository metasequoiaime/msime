package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.text.InputType;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatDialog;
import app.msime.android.BoundsPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.ListPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Predicate;

/**
 * 居中的输入对话框（新建词库、添加词条、添加常用语、导入来源）：r28 的卡片，居中的标题和说明，一到几个输入框，底下「取消 | 确认」两个等宽按钮。
 *
 * <p>排版照设计：标题 17sp/600、说明 13sp 次要文字色，按钮之间和上方各一条分隔线，「取消」是强调色，确认按钮加粗；输入框没填好时确认按钮变淡且不响应。底色 `colorSurfaceContainerLow`，遮罩是设计的 35 % 黑。打开时焦点落在第一个输入框并弹出键盘。
 */
public final class InputDialog {
    private final Context context;
    private final AppCompatDialog dialog;
    private final LinearLayout fields;
    private final List<EditText> inputs = new ArrayList<>(2);
    private final TextView primary;
    private boolean trimValues = true;
    private Predicate<List<String>> valid = values -> {
        for (String value : values) if (value.isEmpty()) return false;
        return true;
    };
    @Nullable private Consumer<List<String>> action;

    public InputDialog(Context context, CharSequence title, @Nullable CharSequence message) {
        this.context = context;
        dialog = new AppCompatDialog(context);
        dialog.supportRequestWindowFeature(Window.FEATURE_NO_TITLE);

        LinearLayout root = LayoutPolicy.column(context);
        ViewPolicy.setBackground(root, DrawablePolicy.rounded(ThemeColorPolicy.sheetBackground(context), DimensionPolicy.pixels(context, Ui.DIALOG_RADIUS)));
        root.setClipToOutline(true);

        TextView heading = ViewPolicy.headingLabel(context, title, Ui.TEXT_DIALOG_TITLE, 600, ThemeColorPolicy.text(context));
        ViewPolicy.setCentered(heading);
        LinearLayout.LayoutParams headingParams = LayoutPolicy.matchWidthWrapParams();
        headingParams.topMargin = DimensionPolicy.pixels(context, 20);
        headingParams.leftMargin = DimensionPolicy.pixels(context, 20);
        headingParams.rightMargin = DimensionPolicy.pixels(context, 20);
        root.addView(heading, headingParams);

        if (message != null && message.length() > 0) {
            TextView note = ViewPolicy.centeredLabel(context, message, Ui.TEXT_SHEET_HEADER, 400,
                ThemeColorPolicy.subText(context));
            LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
            params.topMargin = DimensionPolicy.pixels(context, 4);
            params.leftMargin = DimensionPolicy.pixels(context, 20);
            params.rightMargin = DimensionPolicy.pixels(context, 20);
            root.addView(note, params);
        }

        fields = LayoutPolicy.column(context);
        ViewPolicy.setPaddingDp(context, fields, 16, 6, 16, 16);
        root.addView(fields, LayoutPolicy.matchWidthWrapParams());

        root.addView(LayoutPolicy.divider(context, ThemeColorPolicy.hairline(context), true));
        LinearLayout buttons = LayoutPolicy.row(context);
        TextView cancel = button("取消", 400, ThemeColorPolicy.accent(context));
        ViewPolicy.bindClick(cancel, dialog::cancel);
        buttons.addView(cancel,
            LayoutPolicy.weightedHeightPxParams(DimensionPolicy.pixels(context, 48), 1f));
        buttons.addView(LayoutPolicy.divider(context, ThemeColorPolicy.hairline(context), false));
        primary = button("确定", 600, ThemeColorPolicy.text(context));
        ViewPolicy.bindClick(primary, this::submit);
        buttons.addView(primary,
            LayoutPolicy.weightedHeightPxParams(DimensionPolicy.pixels(context, 48), 1f));
        root.addView(buttons, LayoutPolicy.matchWidthWrapParams());

        dialog.setContentView(root);
        Window window = dialog.getWindow();
        if (window != null) {
            window.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
            window.setLayout(BoundsPolicy.atMost(DimensionPolicy.pixels(context, Ui.DIALOG_WIDTH),
                DimensionPolicy.screenWidthPixels(context) - DimensionPolicy.pixels(context, 48)),
                ViewGroup.LayoutParams.WRAP_CONTENT);
            window.setDimAmount(0.35f);
            configureFloatingInput(window);
        }
        refresh();
    }

    // 浮动对话框仍由 WindowManager 缩小可用区域；edge-to-edge 的 Insets 方案面向铺满窗口的布局。
    // 保留这一处兼容常量，避免 Android 9 上输入框或确认按钮被软键盘遮住。
    @SuppressWarnings("deprecation")
    private static void configureFloatingInput(Window window) {
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE
            | WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
    }

    /**
     * 加一个输入框，返回它以便调用方加长度限制之类的过滤器。
     *
     * @param inputType `InputType` 的组合；0 表示普通单行文字
     */
    public EditText addField(CharSequence hint, @Nullable CharSequence initial, int inputType) {
        EditText input = ViewPolicy.styledInput(context, 15, 400, ThemeColorPolicy.text(context));
        input.setHint(hint);
        input.setText(initial);
        ViewPolicy.setSingleLine(input);
        input.setInputType(inputType == 0 ? InputType.TYPE_CLASS_TEXT : inputType);
        input.setHintTextColor(ThemeColorPolicy.subText(context));
        GradientDrawable field = DrawablePolicy.outlined(ThemeColorPolicy.rowBackground(context), DimensionPolicy.pixels(context, 10),
            DimensionPolicy.atLeastOnePixel(context, 1), ThemeColorPolicy.hairline(context));
        ViewPolicy.setBackground(input, field);
        ViewPolicy.setHorizontalPaddingDp(context, input, 12);
        ViewPolicy.afterTextChanged(input, ignored -> refresh());
        input.setOnEditorActionListener((view, actionId, event) -> {
            int index = inputs.indexOf(input);
            if (index < inputs.size() - 1) return false;
            submit();
            return true;
        });
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthHeightDp(context, 40);
        params.topMargin = DimensionPolicy.pixels(context, 8);
        fields.addView(input, params);
        // 前面的输入框回车跳到下一个，最后一个回车就是提交。
        for (EditText earlier : inputs) earlier.setImeOptions(EditorInfo.IME_ACTION_NEXT);
        input.setImeOptions(EditorInfo.IME_ACTION_DONE);
        inputs.add(input);
        refresh();
        return input;
    }

    /** 确认按钮的文字（例如「创建」「添加」）和动作；动作按添加顺序收到输入内容，默认去首尾空白，preserveWhitespace 时保留原文。 */
    public InputDialog setPrimary(CharSequence label, Consumer<List<String>> action) {
        primary.setText(label);
        this.action = action;
        return this;
    }

    /** 替换「所有输入框都非空」这条默认的可提交条件。 */
    public InputDialog setValidator(Predicate<List<String>> valid) {
        this.valid = valid;
        refresh();
        return this;
    }

    /** 交给共享规则处理首尾字符，避免 Java 的 trim 提前吞掉无效控制字符。 */
    public InputDialog preserveWhitespace() {
        trimValues = false;
        refresh();
        return this;
    }

    public void show() {
        dialog.show();
        if (!inputs.isEmpty()) {
            EditText first = inputs.get(0);
            first.requestFocus();
            first.setSelection(first.length());
        }
    }

    public void dismiss() { dialog.dismiss(); }

    private List<String> values() {
        List<String> values = new ArrayList<>(inputs.size());
        for (EditText input : inputs) {
            String value = input.getText().toString();
            values.add(trimValues ? TextPolicy.trimmed(value) : value);
        }
        return ListPolicy.copyOrEmpty(values);
    }

    private void refresh() {
        boolean ok = valid.test(values());
        ViewPolicy.setEnabledWithAlpha(primary, ok, 0.38f);
    }

    private void submit() {
        List<String> values = values();
        if (!valid.test(values)) return;
        dialog.dismiss();
        if (action != null) action.accept(values);
    }

    private TextView button(CharSequence label, int weight, int color) {
        return ViewPolicy.textButton(context, label, Ui.TEXT_DIALOG_TITLE, weight, color, ViewPolicy.ripple(context), 0);
    }

    /** 分隔线：横的在按钮上方，竖的在两个按钮之间。 */
}
