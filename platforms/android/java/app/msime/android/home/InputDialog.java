package app.msime.android.home;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.GradientDrawable;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
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
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.Collections;
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
    private Predicate<List<String>> valid = values -> {
        for (String value : values) if (value.isEmpty()) return false;
        return true;
    };
    @Nullable private Consumer<List<String>> action;

    public InputDialog(Context context, CharSequence title, @Nullable CharSequence message) {
        this.context = context;
        dialog = new AppCompatDialog(context);
        dialog.supportRequestWindowFeature(Window.FEATURE_NO_TITLE);

        LinearLayout root = Ui.column(context);
        ViewPolicy.setBackground(root, Ui.rounded(Ui.sheetBackground(context), Ui.dp(context, Ui.DIALOG_RADIUS)));
        root.setClipToOutline(true);

        TextView heading = Ui.styledLabel(context, title, Ui.TEXT_DIALOG_TITLE, 600, Ui.text(context));
        ViewPolicy.setCentered(heading);
        heading.setAccessibilityHeading(true);
        LinearLayout.LayoutParams headingParams = Ui.matchWidth();
        headingParams.topMargin = Ui.dp(context, 20);
        headingParams.leftMargin = Ui.dp(context, 20);
        headingParams.rightMargin = Ui.dp(context, 20);
        root.addView(heading, headingParams);

        if (message != null && message.length() > 0) {
            TextView note = Ui.styledLabel(context, message, Ui.TEXT_SHEET_HEADER, 400, Ui.subText(context));
            ViewPolicy.setCentered(note);
            LinearLayout.LayoutParams params = Ui.matchWidth();
            params.topMargin = Ui.dp(context, 4);
            params.leftMargin = Ui.dp(context, 20);
            params.rightMargin = Ui.dp(context, 20);
            root.addView(note, params);
        }

        fields = Ui.column(context);
        Ui.setPaddingDp(fields, context, 16, 6, 16, 16);
        root.addView(fields, Ui.matchWidth());

        root.addView(Ui.divider(context, true));
        LinearLayout buttons = Ui.row(context);
        TextView cancel = button("取消", 400, Ui.accent(context));
        ViewPolicy.bindClick(cancel, dialog::cancel);
        buttons.addView(cancel, Ui.weightedHeight(context, 48, 1f));
        buttons.addView(Ui.divider(context, false));
        primary = button("确定", 600, Ui.text(context));
        ViewPolicy.bindClick(primary, this::submit);
        buttons.addView(primary, Ui.weightedHeight(context, 48, 1f));
        root.addView(buttons, Ui.matchWidth());

        dialog.setContentView(root);
        Window window = dialog.getWindow();
        if (window != null) {
            window.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
            window.setLayout(BoundsPolicy.atMost(Ui.dp(context, Ui.DIALOG_WIDTH),
                Ui.screenWidthPixels(context) - Ui.dp(context, 48)),
                ViewGroup.LayoutParams.WRAP_CONTENT);
            window.setDimAmount(0.35f);
            window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE
                | WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        }
        refresh();
    }

    /**
     * 加一个输入框，返回它以便调用方加长度限制之类的过滤器。
     *
     * @param inputType `InputType` 的组合；0 表示普通单行文字
     */
    public EditText addField(CharSequence hint, @Nullable CharSequence initial, int inputType) {
        EditText input = Ui.styledInput(context, 15, 400, Ui.text(context));
        input.setHint(hint);
        input.setText(initial);
        ViewPolicy.setSingleLine(input);
        input.setInputType(inputType == 0 ? InputType.TYPE_CLASS_TEXT : inputType);
        input.setHintTextColor(Ui.subText(context));
        GradientDrawable field = Ui.outlined(Ui.rowBackground(context), Ui.dp(context, 10),
            Ui.atLeastOnePx(context, 1), Ui.hairline(context));
        ViewPolicy.setBackground(input, field);
        Ui.setHorizontalPaddingDp(input, context, 12);
        input.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

            @Override public void afterTextChanged(Editable text) { refresh(); }
        });
        input.setOnEditorActionListener((view, actionId, event) -> {
            int index = inputs.indexOf(input);
            if (index < inputs.size() - 1) return false;
            submit();
            return true;
        });
        LinearLayout.LayoutParams params = Ui.matchWidthHeight(context, 40);
        params.topMargin = Ui.dp(context, 8);
        fields.addView(input, params);
        // 前面的输入框回车跳到下一个，最后一个回车就是提交。
        for (EditText earlier : inputs) earlier.setImeOptions(EditorInfo.IME_ACTION_NEXT);
        input.setImeOptions(EditorInfo.IME_ACTION_DONE);
        inputs.add(input);
        refresh();
        return input;
    }

    /** 确认按钮的文字（例如「创建」「添加」）和动作；动作收到各输入框去掉首尾空白后的内容，按添加顺序排列。 */
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
        for (EditText input : inputs) values.add(TextPolicy.trimmed(input.getText().toString()));
        return Collections.unmodifiableList(values);
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
        return Ui.textButton(context, label, Ui.TEXT_DIALOG_TITLE, weight, color, Ui.ripple(context), 0);
    }

    /** 分隔线：横的在按钮上方，竖的在两个按钮之间。 */
}
