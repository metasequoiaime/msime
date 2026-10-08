package app.msime.android.home;

import android.os.Bundle;
import android.text.InputType;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.ViewPolicy;

/**
 * 常用语页（设置、「我的」和键盘常用语面板下方的按钮都进这一页）：一组常用语，点一条修改正文，行尾一个「删除」，下面一组「添加常用语」。
 *
 * <p>读写全部经 {@link CommonPhrasesStore}，键盘的「常用语」面板读的是同一份，所以这里改完键盘下次打开面板就能看到。修改经 {@link CommonPhrasesStore#replace}，条目仍属于原来的来源（自己添加的或某个短语包）。
 */
public final class PhrasesPage extends DetailPage {
    @Nullable private LinearLayout column;
    @Nullable private CommonPhrasesStore.Document document;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        if (document != null) render(document);
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void reload() {
        HostTask.run(this, CommonPhrasesStore::load, this::apply);
    }

    private void apply(@Nullable CommonPhrasesStore.Result result) {
        if (result == null) {
            MsToast.show(requireContext(), CommonPhrasesStore.failureMessage(""));
            return;
        }
        if (!result.ok()) {
            MsToast.show(requireContext(), result.failure());
            return;
        }
        document = result.document();
        render(document);
    }

    private void render(CommonPhrasesStore.Document current) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        GroupCard list = GroupCard.add(target, "在键盘「常用语」面板中，点一下即可发送");
        if (current.phrases().isEmpty()) {
            list.note("还没有常用语。添加以后，在键盘的「常用语」面板里点一下就能发送。");
        }
        for (CommonPhrasesStore.Phrase phrase : current.phrases()) {
            GroupCard.Row row = list.button(phrase.text(), null, "删除", () -> remove(phrase));
            // 点整行修改这一条（#5673）；行尾的「删除」按钮自己响应，不会连带打开修改框。
            Ui.makeClickable(row.view(), requireContext(), () -> showEditDialog(phrase));
            row.view().setContentDescription(phrase.text() + "，点按修改");
        }
        if (!current.phrases().isEmpty()) list.footer("点一条常用语可以修改。");
        GroupCard add = GroupCard.add(target, null);
        add.button("添加常用语", null, "添加", this::showAddDialog);
    }

    private void remove(CommonPhrasesStore.Phrase phrase) {
        HostTask.run(this, context -> CommonPhrasesStore.remove(context, phrase.id()), result -> {
            apply(result);
            if (result != null && result.ok()) MsToast.show(requireContext(), "已删除");
        });
    }

    private void showAddDialog() {
        CommonPhrasesStore.Document current = document;
        if (current != null && current.ownCount() >= CommonPhrasesStore.MAX_OWN_PHRASES) {
            MsToast.show(requireContext(), CommonPhrasesStore.failureMessage("common_phrases_limit"));
            return;
        }
        showTextDialog("添加常用语", null, "添加", this::add);
    }

    private void showEditDialog(CommonPhrasesStore.Phrase phrase) {
        showTextDialog("修改常用语", phrase.text(), "保存", text -> {
            if (!text.equals(phrase.text())) replace(phrase, text);
        });
    }

    /** 添加和修改共用的输入框：多行，最多 1000 字，正文不合规时确认按钮不可点。 */
    private void showTextDialog(String title, @Nullable String initial, String primary,
            java.util.function.Consumer<String> action) {
        InputDialog dialog = new InputDialog(requireContext(), title, "可以换行，最多 1000 字");
        EditText field = dialog.addField("输入常用语", initial,
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        // 常用语可以多行：换成多行输入框，高度随内容长到 6 行。
        ViewPolicy.setSingleLine(field, false);
        ViewPolicy.setMinLines(field, 3);
        ViewPolicy.setMaxLines(field, 6);
        ViewPolicy.setTopStart(field);
        int padding = Ui.dp(requireContext(), 10);
        int horizontal = Ui.dp(requireContext(), 12);
        ViewPolicy.setPadding(field, horizontal, padding, horizontal, padding);
        ViewGroup.LayoutParams params = field.getLayoutParams();
        if (params != null) {
            params.height = ViewGroup.LayoutParams.WRAP_CONTENT;
            field.setLayoutParams(params);
        }
        dialog.setValidator(values -> CommonPhrasesStore.validText(values.get(0)));
        dialog.setPrimary(primary, values -> action.accept(values.get(0)));
        dialog.show();
    }

    private void replace(CommonPhrasesStore.Phrase phrase, String text) {
        HostTask.run(this, context -> CommonPhrasesStore.replace(context, phrase.id(), text), result -> {
            apply(result);
            if (result != null && result.ok()) MsToast.show(requireContext(), "已修改");
        });
    }

    private void add(String text) {
        HostTask.run(this, context -> CommonPhrasesStore.add(context, text), result -> {
            apply(result);
            if (result != null && result.ok()) MsToast.show(requireContext(), "已添加");
        });
    }
}
