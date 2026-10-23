package app.msime.android.home;

import app.msime.android.DimensionPolicy;

import android.text.InputType;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CommunityRequest;
import app.msime.android.ViewPolicy;

/**
 * 常用语页（设置、「我的」和键盘常用语面板下方的按钮都进这一页）：一组常用语，点一条修改正文与备注，行尾一个「删除」，下面一组「添加常用语」和「浏览社区短语包」（进社区短语的完整列表）。
 *
 * <p>读写全部经 {@link CommonPhrasesStore}，键盘的「常用语」面板读的是同一份，所以这里改完键盘下次打开面板就能看到。修改经 {@link CommonPhrasesStore#replace}，条目仍属于原来的来源（自己添加的或某个短语包）。
 */
public final class PhrasesPage extends ReloadingDetailPage {
    @Nullable private CommonPhrasesStore.Document document;

    @Override protected void onContentReady() {
        if (document != null) render(document);
    }

    @Override protected void reload() {
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
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        GroupCard list = GroupCard.add(target, "在键盘「常用语」面板中，点一下即可发送");
        if (current.phrases().isEmpty()) {
            list.note("还没有常用语。添加以后，在键盘的「常用语」面板里点一下就能发送。");
        }
        for (CommonPhrasesStore.Phrase phrase : current.phrases()) {
            GroupCard.Row row = list.button(phrase.text(),
                phrase.note().isEmpty() ? null : "备注：" + phrase.note(), "删除", () -> remove(phrase));
            row.setSubtitleMaxLines(2);
            // 点整行修改这一条（#5673）；行尾的「删除」按钮自己响应，不会连带打开修改框。
            ViewPolicy.makeClickable(row.view(), requireContext(), () -> showEditDialog(phrase));
            row.view().setContentDescription(phrase.text() + (phrase.note().isEmpty() ? "" : "，备注：" + phrase.note()) + "，点按修改");
        }
        if (!current.phrases().isEmpty()) list.footer("点一条常用语可以修改。");
        GroupCard add = GroupCard.add(target, null);
        add.button("添加常用语", null, "添加", this::showAddDialog);
        // 社区短语包装进来就在这一页里；浏览和添加在社区短语的完整列表里，表达页的「发现短语」也通向那里。
        add.nav("浏览社区短语包", "别人分享的短语包，添加后出现在这里", null,
            () -> CommunityFragment.browse(requireContext(), CommunityRequest.Kind.PHRASE));
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
        showTextDialog("添加常用语", null, null, "添加", this::add);
    }

    private void showEditDialog(CommonPhrasesStore.Phrase phrase) {
        showTextDialog("修改常用语", phrase.text(), phrase.note(), "保存", (text, note) -> {
            if (!text.equals(phrase.text()) || !note.equals(phrase.note())) replace(phrase, text, note);
        });
    }

    /** 添加和修改共用的输入框：正文和备注各最多 1000 字，不合规时确认按钮不可点。 */
    private void showTextDialog(String title, @Nullable String initial, @Nullable String initialNote, String primary,
            java.util.function.BiConsumer<String, String> action) {
        InputDialog dialog = new InputDialog(requireContext(), title, "正文与备注各最多 1000 字，备注仅本机显示，不会插入");
        EditText field = dialog.addField("输入常用语", initial,
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        field.setContentDescription("常用语正文");
        // 常用语可以多行：换成多行输入框，高度随内容长到 6 行。
        ViewPolicy.setSingleLine(field, false);
        ViewPolicy.setMinLines(field, 3);
        ViewPolicy.setMaxLines(field, 6);
        ViewPolicy.setTopStart(field);
        int padding = DimensionPolicy.pixels(requireContext(), 10);
        int horizontal = DimensionPolicy.pixels(requireContext(), 12);
        ViewPolicy.setPadding(field, horizontal, padding, horizontal, padding);
        ViewGroup.LayoutParams params = field.getLayoutParams();
        if (params != null) {
            params.height = ViewGroup.LayoutParams.WRAP_CONTENT;
            field.setLayoutParams(params);
        }
        EditText note = dialog.addField("备注（可选，清空即可删除）", initialNote,
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        note.setContentDescription("常用语备注");
        ViewPolicy.setSingleLine(note, false);
        ViewPolicy.setMinLines(note, 2);
        ViewPolicy.setMaxLines(note, 2);
        ViewPolicy.setTopStart(note);
        ViewPolicy.setPadding(note, horizontal, padding, horizontal, padding);
        ViewGroup.LayoutParams noteParams = note.getLayoutParams();
        noteParams.height = ViewGroup.LayoutParams.WRAP_CONTENT;
        note.setLayoutParams(noteParams);
        dialog.setValidator(values -> CommonPhrasesStore.validText(values.get(0))
            && CommonPhrasesStore.validNote(values.get(1)));
        dialog.setPrimary(primary, values -> action.accept(values.get(0), values.get(1)));
        dialog.show();
    }

    private void replace(CommonPhrasesStore.Phrase phrase, String text, String note) {
        HostTask.run(this, context -> CommonPhrasesStore.replace(context, phrase.id(), text, note), result -> {
            apply(result);
            if (result != null && result.ok()) MsToast.show(requireContext(), "已修改");
        });
    }

    private void add(String text, String note) {
        HostTask.run(this, context -> CommonPhrasesStore.add(context, text, note), result -> {
            apply(result);
            if (result != null && result.ok()) MsToast.show(requireContext(), "已添加");
        });
    }
}
