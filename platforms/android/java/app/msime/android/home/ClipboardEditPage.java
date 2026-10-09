package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.fragment.app.FragmentActivity;
import app.msime.android.ClipboardHistory;
import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.ClipboardHistoryStore;
import app.msime.android.ViewPolicy;

/**
 * 编辑一条剪贴板历史（#5971）：键盘剪贴板面板里长按一条、点「编辑」打开这一页。上面是预填原文的多行输入框，可以用系统的选择手柄精确地删掉多余的字；下面是「取消」「保存」。
 *
 * <p>键盘里没有可输入的文本框，增删改放在应用里，和常用语（#5673）一样。键盘只传来认出这一条的键（{@link ClipboardHistoryPolicy#EDIT_ENTRY_ARG}，值是 {@link ClipboardHistoryPolicy#editKey}），这一页在共享存储里找到那一条再显示；文字本身不经过 Intent。深链参数都带外部标记，这个键只用来选择显示哪一条，写回要用户自己点「保存」。
 *
 * <p>保存经 {@link ClipboardHistoryStore#replace}：条目留在原位，时间戳和固定状态不变；改成已有的文字时两条合并成一条；那一条已经被删掉或清空时说出来，不悄悄失败。保存或取消后回到原来的应用，接着就能在键盘里插入改好的那条。
 */
public final class ClipboardEditPage extends DetailPage {
    private enum State { LOADING, READY, MISSING, FAILED }

    private State state = State.LOADING;
    /** 要编辑的那一条的原文；找到之前为 null。 */
    @Nullable private String original;
    /** 输入框里的文字，视图重建（旋转、换深浅模式）后照样填回去。 */
    @Nullable private String draft;
    private boolean saving;
    @Nullable private LinearLayout column;
    @Nullable private TextView save;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        if (state == State.LOADING) load(args.getString(ClipboardHistoryPolicy.EDIT_ENTRY_ARG));
        render();
    }

    @Override public void onDestroyView() {
        column = null;
        save = null;
        super.onDestroyView();
    }

    private void load(@Nullable String key) {
        if (key == null || key.isEmpty()) {
            state = State.MISSING;
            return;
        }
        HostTask.run(this, context -> find(context, key), found -> {
            if (found == null) {
                state = State.FAILED;
            } else if (found.isEmpty()) {
                state = State.MISSING;
            } else {
                original = found;
                if (draft == null) draft = found;
                state = State.READY;
            }
            render();
        });
    }

    /** 在共享存储里按键找那一条；找不到时为空串。读不出来时抛异常，由 {@link HostTask} 交回 null。 */
    private static String find(Context context, String key) {
        for (ClipboardHistory.Item item : new ClipboardHistoryStore(context.getFilesDir()).load()) {
            if (key.equals(ClipboardHistoryPolicy.editKey(item.timestamp(), item.text()))) return item.text();
        }
        return "";
    }

    private void render() {
        LinearLayout target = column;
        if (target == null) return;
        Context context = requireContext();
        target.removeAllViews();
        save = null;
        GroupCard card = GroupCard.add(target, "剪贴板记录");
        switch (state) {
            case LOADING -> card.note("正在读取…");
            case MISSING -> card.note(ClipboardHistoryPolicy.editMessage(ClipboardHistoryPolicy.EditResult.NOT_FOUND));
            case FAILED -> card.note("剪贴板历史读取失败，请稍后重试");
            case READY -> {
                EditText input = Ui.styledInput(context, Ui.TEXT_ROW_TITLE, 400, Ui.text(context));
                input.setHint("剪贴板记录的文字");
                input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
                ViewPolicy.setTopStart(input);
                ViewPolicy.setMinLines(input, 4);
                ViewPolicy.clearBackground(input);
                input.setHintTextColor(Ui.subText(context));
                Ui.setSymmetricPaddingDp(input, context, 16, 14);
                input.setText(draft);
                input.setContentDescription("剪贴板记录的文字");
                card.card().addView(input, Ui.matchWidth());
                input.addTextChangedListener(new TextWatcher() {
                    @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

                    @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

                    @Override public void afterTextChanged(Editable text) {
                        draft = text.toString();
                        refresh();
                    }
                });
                card.footer("保存后替换原来那一条，位置和固定状态不变；改成已有的文字时两条合并成一条。");
                input.requestFocus();
                input.setSelection(input.length());
            }
        }

        LinearLayout buttons = Ui.row(context);
        LinearLayout.LayoutParams buttonsParams = Ui.matchWidth();
        buttonsParams.topMargin = Ui.dp(context, Ui.GROUP_GAP);
        target.addView(buttons, buttonsParams);
        boolean editing = state == State.READY;
        TextView cancel = Ui.textButton(context, editing ? "取消" : "返回", 16, 600, Ui.accent(context),
            Ui.rippleOn(context, Ui.rowBackground(context), Ui.dp(context, Ui.GROUP_RADIUS)),
            Ui.ACTION_BUTTON_MIN_HEIGHT, this::leave);
        buttons.addView(cancel, Ui.weightedHeight(context, Ui.ACTION_BUTTON_MIN_HEIGHT, 1f));
        if (editing) {
            TextView primary = Ui.textButton(context, "保存", 16, 600, Ui.onAccent(context), null,
                Ui.ACTION_BUTTON_MIN_HEIGHT, this::submit);
            LinearLayout.LayoutParams primaryParams = Ui.weightedHeight(context, Ui.ACTION_BUTTON_MIN_HEIGHT, 1f);
            primaryParams.setMarginStart(Ui.dp(context, 12));
            buttons.addView(primary, primaryParams);
            save = primary;
        }
        refresh();
    }

    /** 「保存」只在文字有内容、和原文不同、没有正在保存时可点。 */
    private void refresh() {
        TextView button = save;
        Context context = getContext();
        if (button == null || context == null) return;
        boolean ready = !saving && original != null && ClipboardHistoryPolicy.hasText(draft)
            && !original.equals(draft);
        button.setText(saving ? "正在保存…" : "保存");
        ViewPolicy.setEnabled(button, ready);
        ViewPolicy.setTextColor(button, ready ? Ui.onAccent(context) : Ui.subText(context));
        int fill = ready ? Ui.accent(context)
            : Ui.color(context, com.google.android.material.R.attr.colorSurfaceContainerHighest);
        ViewPolicy.setBackground(button, Ui.rippleOn(context, fill, Ui.dp(context, Ui.GROUP_RADIUS)));
    }

    private void submit() {
        String from = original;
        String to = draft;
        if (saving || from == null || to == null || !ClipboardHistoryPolicy.hasText(to) || from.equals(to)) return;
        saving = true;
        refresh();
        HostTask.run(this, context -> new ClipboardHistoryStore(context.getFilesDir()).replace(from, to), result -> {
            saving = false;
            if (result == null) {
                MsToast.show(requireContext(), "保存失败，请稍后重试");
                refresh();
                return;
            }
            MsToast.show(requireContext(), ClipboardHistoryPolicy.editMessage(result));
            switch (result) {
                case SAVED, MERGED -> leave();
                case NOT_FOUND -> {
                    state = State.MISSING;
                    render();
                }
                case INVALID -> refresh();
            }
        });
    }

    /** 关掉这一页并回到原来的应用：这一页是从键盘打开的，用户要回去接着输入。 */
    private void leave() {
        FragmentActivity activity = getActivity();
        if (activity == null) return;
        activity.getOnBackPressedDispatcher().onBackPressed();
        activity.moveTaskToBack(true);
    }
}
