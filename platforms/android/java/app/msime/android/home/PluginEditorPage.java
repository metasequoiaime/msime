package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import android.os.Bundle;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.activity.OnBackPressedCallback;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.fragment.app.FragmentManager;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginEditorPolicy;
import app.msime.android.policy.PluginEditorPolicy.Command;
import app.msime.android.policy.PluginEditorPolicy.Draft;
import app.msime.android.policy.PluginEditorPolicy.DraftGroup;
import app.msime.android.policy.PluginEditorPolicy.DraftWord;
import app.msime.android.policy.PluginEditorPolicy.Phrase;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 插件编辑器：新建或编辑短语表、指令表、符号集和单词本，与共享设置页 `plugin-editor-view.tsx` 同样的表单、说明和报错。
 *
 * <p>先选类型（编辑已安装的包时跳过这一步，经 `msime_client_plugins` 的 `authoring_load` 读出原内容），再填基本信息和内容；短语表和单词本可以批量粘贴，长列表每次画 100 行。保存按钮上方只报第一处问题（{@link PluginEditorPolicy#draftIssue}），保存经 `authoring_save` 交给 client-services，按导入同样的规则再查一遍并安装，拒绝时把它的 `detail` 就地显示。保存后打开这个包的详情，顶上说明还差哪一步才能用上。
 *
 * <p>输入时只改草稿、就地更新问题和保存按钮，不重画，免得输入框丢焦点；加减行、批量添加和保存才重画。有没保存的修改时，返回先确认。草稿在旋转、换深浅模式时经 saved state 保留（太大的单词本不保留，只在内存里）。
 *
 * <p>参数 `kind`、`id` 都给时编辑那个包，都没有时新建。
 */
public final class PluginEditorPage extends ColumnDetailPage {
    private static final String ARG_KIND = "kind";
    private static final String ARG_ID = "id";
    /** 长列表一次画出的行数：单词本可以有两万行，全部画出来会让页面卡住。 */
    private static final int ROW_PAGE = 100;
    /** 草稿序列化后超过这么多字符就不放进 saved state：Binder 事务有 1 MB 的上限。 */
    private static final int MAX_SAVED_DRAFT_CHARS = 200_000;

    static void create(Context context) {
        SettingsNavigator.open(context, PageId.PLUGIN_EDITOR, null);
    }

    static void edit(Context context, String kind, String id) {
        Bundle args = new Bundle();
        args.putString(ARG_KIND, kind);
        args.putString(ARG_ID, id);
        SettingsNavigator.open(context, PageId.PLUGIN_EDITOR, args);
    }

    @Nullable private String targetKind;
    @Nullable private String targetId;
    @Nullable private Draft draft;
    @Nullable private Draft baseline;
    @Nullable private String loadError;
    @Nullable private String saveError;
    private boolean saving;
    private int shown = ROW_PAGE;
    private String bulk = "";
    @Nullable private String bulkIssue;
    @Nullable private TextView issueLabel;
    @Nullable private TextView saveErrorLabel;
    @Nullable private TextView saveButton;
    private final OnBackPressedCallback guard = new OnBackPressedCallback(false) {
        @Override public void handleOnBackPressed() {
            MessageDialog.confirm(requireContext(), "放弃修改", "还没有保存的修改会丢掉。", "放弃", () -> {
                setEnabled(false);
                requireActivity().getOnBackPressedDispatcher().onBackPressed();
            });
        }
    };

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        Bundle args = getArguments();
        targetKind = args == null ? null : args.getString(ARG_KIND);
        targetId = args == null ? null : args.getString(ARG_ID);
        if (targetKind == null || targetId == null || !PluginEditorPolicy.isAuthoredKind(targetKind)) {
            targetKind = null;
            targetId = null;
        }
        if (saved != null) {
            draft = draftFrom(saved.getString("plugin-editor-draft"));
            baseline = draftFrom(saved.getString("plugin-editor-baseline"));
            if (draft == null || baseline == null) {
                draft = null;
                baseline = null;
            }
        }
    }

    @Override public void onSaveInstanceState(@NonNull Bundle out) {
        super.onSaveInstanceState(out);
        if (draft == null || baseline == null) return;
        String draftText = draftJson(draft);
        String baselineText = draftJson(baseline);
        if (draftText.length() + baselineText.length() > MAX_SAVED_DRAFT_CHARS) return;
        out.putString("plugin-editor-draft", draftText);
        out.putString("plugin-editor-baseline", baselineText);
    }

    @Override protected CharSequence title() {
        return titleText();
    }

    private String titleText() {
        if (targetKind != null) return "编辑「" + (baseline == null ? targetId : baseline.name) + "」";
        return draft == null ? "新建插件" : "新建" + PluginEditorPolicy.kindTitle(draft.kind);
    }

    @Override protected void onColumnReady(Bundle args) {
        requireActivity().getOnBackPressedDispatcher().addCallback(getViewLifecycleOwner(), guard);
        if (targetKind != null && draft == null && loadError == null) {
            String kind = targetKind;
            String id = targetId;
            HostTask.run(this, context -> PluginStore.load(context, kind, id), result -> {
                if (result == null || !result.succeeded()) {
                    loadError = result == null ? "无法读取这个插件，请重试。"
                        : PluginCatalogPolicy.errorMessage(result.code(), result.detail(), "无法读取这个插件，请重试。");
                } else {
                    draft = PluginEditorPolicy.draftFromPack(result.value());
                    baseline = draft.copy();
                }
                render();
            });
        }
        render();
    }

    @Override protected void onContentDestroyed() {
        issueLabel = null;
        saveErrorLabel = null;
        saveButton = null;
    }

    private boolean dirty() {
        return draft != null && !draft.equals(baseline);
    }

    // ---- 画 ----

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        setTitle(titleText());
        Context context = requireContext();
        Draft current = draft;
        if (current == null) {
            if (targetKind != null) {
                GroupCard.add(target, null).note(loadError == null ? "正在读取插件…" : loadError);
            } else {
                GroupCard kinds = GroupCard.add(target, "想做哪一种插件？").withDividers(Ui.ROW_PADDING_H);
                for (PluginEditorPolicy.KindEntry entry : PluginEditorPolicy.AUTHORED_KINDS) {
                    kinds.nav(entry.title(), null, null, () -> pick(entry.kind())).help(entry.description());
                }
            }
            refreshStatus();
            return;
        }
        String kindLabel = PluginEditorPolicy.kindTitle(current.kind);

        GroupCard basics = GroupCard.add(target, "基本信息");
        LinearLayout basicsColumn = padded(basics);
        field(basicsColumn, "名字", current.name, "我的" + kindLabel, false, false, value -> current.name = value);
        field(basicsColumn, "作者（可不填）", current.author, null, false, false, value -> current.author = value);
        field(basicsColumn, "版本", current.version, null, false, true, value -> current.version = value);
        field(basicsColumn, "许可证", current.license, null, false, true, value -> current.license = value);
        basics.help("许可证决定别人能怎样使用这个插件。" + PluginEditorPolicy.DEFAULT_LICENSE
            + " 表示放弃全部权利，任何人都能随意使用和改编；想让别人注明出自你，可以写 CC-BY-4.0。这里只能写 SPDX 许可证代码。");
        field(padded(basics), "介绍（可不填）", current.description, null, true, false, value -> current.description = value);

        switch (current.kind) {
            case PluginEditorPolicy.PHRASE_TABLE -> renderPhrases(target, current);
            case PluginEditorPolicy.COMMAND_TABLE -> renderCommands(target, current);
            case PluginEditorPolicy.SYMBOL_SET -> renderGroups(target, current);
            default -> renderWords(target, current);
        }
        if (PluginEditorPolicy.PHRASE_TABLE.equals(current.kind) || PluginEditorPolicy.WORDBOOK.equals(current.kind)) {
            renderBulk(target, current);
        }

        GroupCard save = GroupCard.add(target, null);
        save.note(targetKind != null
            ? "保存后替换本机的这一份。从社区装来的插件改过之后就是你自己的副本，原作者那边不受影响。"
            : "保存后装进本机，和导入的插件一样出现在「我的插件」里，也可以在「社区插件」里发布给别人。");
        issueLabel = warning(save);
        saveErrorLabel = warning(save);
        TextView button = KeyboardSheets.bigButton(context, targetKind != null ? "保存修改" : "保存并安装", true, this::save);
        LinearLayout.LayoutParams buttonParams = LayoutPolicy.matchWidthWrapParams();
        int margin = DimensionPolicy.pixels(context, Ui.ROW_PADDING_H);
        buttonParams.setMargins(margin, DimensionPolicy.pixels(context, 4), margin, margin);
        save.card().addView(button, buttonParams);
        saveButton = button;
        refreshStatus();
    }

    /** 只更新问题、保存失败和保存按钮，不重画：输入框保持焦点。 */
    private void refreshStatus() {
        boolean dirty = dirty();
        guard.setEnabled(dirty && !saving);
        String issue = draft == null ? null : PluginEditorPolicy.draftIssue(draft);
        if (issueLabel != null) ViewPolicy.setTextAndVisibility(issueLabel, issue != null && dirty ? issue : null);
        if (saveErrorLabel != null) ViewPolicy.setTextAndVisibility(saveErrorLabel, saveError);
        if (saveButton != null) {
            boolean ready = !saving && dirty && issue == null;
            saveButton.setEnabled(ready);
            saveButton.setAlpha(ready ? 1f : 0.45f);
        }
    }

    private void pick(String kind) {
        draft = PluginEditorPolicy.emptyDraft(kind);
        baseline = draft.copy();
        render();
    }

    /** 改了草稿：清掉上一次的保存失败，更新问题和按钮。 */
    private void changed() {
        saveError = null;
        refreshStatus();
    }

    private void renderPhrases(LinearLayout target, Draft current) {
        GroupCard group = GroupCard.add(target, "短语（" + current.phrases.size() + "）");
        group.help("编码是 K 之后输入的小写字母，最多 32 个；同一个编码可以对应几段文字。文字最多 "
            + PluginEditorPolicy.MAX_TEXT_UTF16 + " 个字符，不能换行。");
        group.note("在文字中用 ${cursor} 指定上屏后的光标位置，例如 title:\"${cursor}\" 会输入 title:\"\"，"
            + "光标停在引号内。只定位第一个标记；要输入标记本身，请写 $${cursor}。编辑器不支持定位时光标留在末尾。");
        for (int index = 0; index < Math.min(current.phrases.size(), shown); index++) {
            int row = index;
            LinearLayout block = rowBlock(group, "第 " + (row + 1) + " 条短语", () -> {
                current.phrases.remove(row);
                render();
            });
            field(block, "编码", current.phrases.get(row).key(), "dz", false, true,
                value -> current.phrases.set(row, new Phrase(value, current.phrases.get(row).text())));
            field(block, "文字", current.phrases.get(row).text(), null, false, false,
                value -> current.phrases.set(row, new Phrase(current.phrases.get(row).key(), value)));
        }
        moreRows(group, current.phrases.size());
        addButton(group, "添加一条", current.phrases.size() < PluginEditorPolicy.MAX_PHRASES, () -> {
            shown = Math.max(shown, current.phrases.size() + 1);
            current.phrases.add(PluginEditorPolicy.emptyPhrase());
        });
    }

    private void renderCommands(LinearLayout target, Draft current) {
        GroupCard group = GroupCard.add(target, "指令（" + current.commands.size() + "）");
        group.help("指令是 / 之后输入的小写字母。要插入的文字里可以写 {date}（日期）、{time}（时间）和 {weekday}（星期几），例如「{date} 张三」；日期和时间也可以带格式，例如 {date:%m月%d日}。");
        for (int index = 0; index < current.commands.size(); index++) {
            int row = index;
            LinearLayout block = rowBlock(group, "第 " + (row + 1) + " 条指令", () -> {
                current.commands.remove(row);
                render();
            });
            field(block, "指令", current.commands.get(row).trigger(), "sig", false, true, value -> {
                Command old = current.commands.get(row);
                current.commands.set(row, new Command(value, old.title(), old.template()));
            });
            field(block, "标题", current.commands.get(row).title(), "签名", false, false, value -> {
                Command old = current.commands.get(row);
                current.commands.set(row, new Command(old.trigger(), value, old.template()));
            });
            field(block, "要插入的文字", current.commands.get(row).template(), null, false, false, value -> {
                Command old = current.commands.get(row);
                current.commands.set(row, new Command(old.trigger(), old.title(), value));
            });
        }
        addButton(group, "添加一条", current.commands.size() < PluginEditorPolicy.MAX_COMMANDS,
            () -> current.commands.add(PluginEditorPolicy.emptyCommand()));
    }

    private void renderGroups(LinearLayout target, Draft current) {
        for (int index = 0; index < current.groups.size(); index++) {
            int row = index;
            GroupCard group = GroupCard.add(target, "第 " + (row + 1) + " 组");
            DraftGroup value = current.groups.get(row);
            group.nav("放在", null, PluginEditorPolicy.TAB_KAOMOJI.equals(value.tab()) ? "颜文字" : "符号", () -> {
                OptionSheet sheet = new OptionSheet(requireContext(), "第 " + (row + 1) + " 组放在哪一页", null);
                for (String tab : new String[] {PluginEditorPolicy.TAB_SYMBOLS, PluginEditorPolicy.TAB_KAOMOJI}) {
                    sheet.option(PluginEditorPolicy.TAB_KAOMOJI.equals(tab) ? "颜文字" : "符号",
                        tab.equals(current.groups.get(row).tab()), () -> {
                            DraftGroup old = current.groups.get(row);
                            current.groups.set(row, new DraftGroup(tab, old.title(), old.keywords(), old.items()));
                            changed();
                            render();
                        });
                }
                sheet.show();
            });
            LinearLayout block = padded(group);
            field(block, "标题", value.title(), "箭头", false, false, text -> {
                DraftGroup old = current.groups.get(row);
                current.groups.set(row, new DraftGroup(old.tab(), text, old.keywords(), old.items()));
            });
            field(block, "搜索关键词（可不填）", value.keywords(), "jiantou arrow", false, false, text -> {
                DraftGroup old = current.groups.get(row);
                current.groups.set(row, new DraftGroup(old.tab(), old.title(), text, old.items()));
            });
            field(block, "符号（每行一个，可以直接粘贴）", value.items(), null, true, false, text -> {
                DraftGroup old = current.groups.get(row);
                current.groups.set(row, new DraftGroup(old.tab(), old.title(), old.keywords(), text));
            });
            group.button("删除这一组", null, "删除", () -> {
                current.groups.remove(row);
                changed();
                render();
            });
        }
        // 这一组没有组标题，符号组放在哪里的说明挂在「添加一组」这一行的「?」上。
        GroupCard add = GroupCard.add(target, null);
        addButton(add, "添加一组", current.groups.size() < PluginEditorPolicy.MAX_GROUPS,
            () -> current.groups.add(PluginEditorPolicy.emptyGroup()))
            .help("符号组以插件名为分类排在符号面板的内置符号之后，颜文字组排在「颜文字」之后。");
    }

    private void renderWords(LinearLayout target, Draft current) {
        GroupCard group = GroupCard.add(target, "单词（" + current.words.size() + "）");
        group.help("每个单词要有释义，音标可以不填。单词本的名字最多 " + PluginEditorPolicy.MAX_WORDBOOK_NAME_CHARS
            + " 个字，最多 " + PluginEditorPolicy.MAX_WORDS + " 个单词。");
        for (int index = 0; index < Math.min(current.words.size(), shown); index++) {
            int row = index;
            LinearLayout block = rowBlock(group, "第 " + (row + 1) + " 个单词", () -> {
                current.words.remove(row);
                render();
            });
            field(block, "单词", current.words.get(row).word(), null, false, true, value -> {
                DraftWord old = current.words.get(row);
                current.words.set(row, new DraftWord(value, old.phonetic(), old.gloss()));
            });
            field(block, "音标（可不填）", current.words.get(row).phonetic(), null, false, true, value -> {
                DraftWord old = current.words.get(row);
                current.words.set(row, new DraftWord(old.word(), value, old.gloss()));
            });
            field(block, "释义", current.words.get(row).gloss(), null, false, false, value -> {
                DraftWord old = current.words.get(row);
                current.words.set(row, new DraftWord(old.word(), old.phonetic(), value));
            });
        }
        moreRows(group, current.words.size());
        addButton(group, "添加一个", current.words.size() < PluginEditorPolicy.MAX_WORDS, () -> {
            shown = Math.max(shown, current.words.size() + 1);
            current.words.add(PluginEditorPolicy.emptyWord());
        });
    }

    private void renderBulk(LinearLayout target, Draft current) {
        boolean phrases = PluginEditorPolicy.PHRASE_TABLE.equals(current.kind);
        GroupCard group = GroupCard.add(target, "批量添加");
        group.help(phrases ? "编码在前，文字在后，中间用空格隔开，例如 dz 北京市海淀区。"
            : "从表格复制过来即可：单词、释义两列，或单词、音标、释义三列，列之间用 Tab 隔开。");
        TextView[] issue = new TextView[1];
        TextView[] add = new TextView[1];
        EditText input = field(padded(group), phrases ? "每行一条短语" : "每行一个单词", bulk, null, true, true, value -> {
            bulk = value;
            bulkIssue = null;
            if (issue[0] != null) ViewPolicy.hide(issue[0]);
            if (add[0] != null) add[0].setEnabled(!PluginEditorPolicy.blank(value));
        });
        input.setContentDescription("批量添加的内容");
        issue[0] = warning(group);
        ViewPolicy.setTextAndVisibility(issue[0], bulkIssue);
        GroupCard.Row row = group.button(phrases ? "添加到短语列表" : "添加到单词列表", null, "添加到列表", () -> addBulk(current));
        add[0] = row.button();
        if (add[0] != null) add[0].setEnabled(!PluginEditorPolicy.blank(bulk));
    }

    private void addBulk(Draft current) {
        if (PluginEditorPolicy.PHRASE_TABLE.equals(current.kind)) {
            PluginEditorPolicy.BulkRows<Phrase> parsed = PluginEditorPolicy.parsePhraseLines(bulk);
            bulkIssue = parsed.issue();
            if (bulkIssue == null) {
                List<Phrase> rows = PluginEditorPolicy.appendRows(current.phrases, parsed.rows(), PluginEditorPolicy::isBlankPhrase);
                current.phrases.clear();
                current.phrases.addAll(rows);
            }
        } else {
            PluginEditorPolicy.BulkRows<DraftWord> parsed = PluginEditorPolicy.parseWordLines(bulk);
            bulkIssue = parsed.issue();
            if (bulkIssue == null) {
                List<DraftWord> rows = PluginEditorPolicy.appendRows(current.words, parsed.rows(), PluginEditorPolicy::isBlankWord);
                current.words.clear();
                current.words.addAll(rows);
            }
        }
        if (bulkIssue == null) bulk = "";
        changed();
        render();
    }

    // ---- 控件 ----

    /** 卡片里放输入框的一列，左右与行对齐。 */
    private static LinearLayout padded(GroupCard group) {
        Context context = group.card().getContext();
        LinearLayout column = LayoutPolicy.column(context);
        ViewPolicy.setPaddingDp(context, column, Ui.ROW_PADDING_H, 4, Ui.ROW_PADDING_H, 12);
        group.addView(column);
        return column;
    }

    /** 一行内容的块：标题「第几条」和它的「删除」，下面是这一行的输入框。 */
    private LinearLayout rowBlock(GroupCard group, String title, Runnable remove) {
        Context context = requireContext();
        LinearLayout block = padded(group);
        LinearLayout header = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(header);
        header.addView(ViewPolicy.styledLabel(context, title, 13, 500, ThemeColorPolicy.subText(context)),
            LayoutPolicy.weightedWrapParams(1));
        TextView delete = HomeButtonPolicy.tonalButton(context, "删除", "删除" + title, 500, () -> {
            remove.run();
            changed();
        });
        header.addView(delete, LayoutPolicy.wrapParams());
        block.addView(header, LayoutPolicy.matchWidthWrapParams());
        return block;
    }

    private void moreRows(GroupCard group, int total) {
        if (total <= shown) return;
        group.button("共 " + total + " 条", null, "再显示 " + Math.min(ROW_PAGE, total - shown) + " 条", () -> {
            shown += ROW_PAGE;
            render();
        });
    }

    private GroupCard.Row addButton(GroupCard group, String label, boolean enabled, Runnable add) {
        GroupCard.Row row = group.button(label, null, label, () -> {
            add.run();
            changed();
            render();
        });
        if (row.button() != null) row.button().setEnabled(enabled);
        return row;
    }

    /** 红字的一行说明（问题、保存失败、批量粘贴读不出的那一行），没有内容时不占位置。 */
    private static TextView warning(GroupCard group) {
        Context context = group.card().getContext();
        TextView label = ViewPolicy.styledLabel(context, "", Ui.TEXT_ROW_SUBTITLE, 500, ThemeColorPolicy.danger(context));
        ViewPolicy.setSymmetricPaddingDp(context, label, Ui.ROW_PADDING_H, 6);
        ViewPolicy.setPoliteLiveRegion(label);
        ViewPolicy.hide(label);
        group.addView(label);
        return label;
    }

    /** 一个带标签的输入框；改动直接写进草稿，再更新问题和保存按钮。 */
    private EditText field(LinearLayout parent, String label, String value, @Nullable String hint, boolean multiline,
            boolean plain, Consumer<String> onChange) {
        Context context = requireContext();
        TextView caption = ViewPolicy.styledLabel(context, label, 13, 400, ThemeColorPolicy.subText(context));
        LinearLayout.LayoutParams captionParams = LayoutPolicy.matchWidthWrapParams();
        captionParams.topMargin = DimensionPolicy.pixels(context, 8);
        parent.addView(caption, captionParams);
        EditText input = ViewPolicy.styledInput(context, 15, 400, ThemeColorPolicy.text(context));
        input.setText(value);
        if (hint != null) input.setHint(hint);
        input.setHintTextColor(ThemeColorPolicy.subText(context));
        int type = InputType.TYPE_CLASS_TEXT;
        if (multiline) type |= InputType.TYPE_TEXT_FLAG_MULTI_LINE;
        if (plain) type |= InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
        input.setInputType(type);
        if (multiline) {
            ViewPolicy.setTopStart(input);
            ViewPolicy.setMinLines(input, 3);
        } else {
            ViewPolicy.setSingleLine(input);
        }
        input.setContentDescription(label);
        ViewPolicy.setBackground(input, DrawablePolicy.outlined(ThemeColorPolicy.rowBackground(context),
            DimensionPolicy.pixels(context, 10), DimensionPolicy.atLeastOnePixel(context, 1), ThemeColorPolicy.hairline(context)));
        ViewPolicy.setSymmetricPaddingDp(context, input, 12, 8);
        input.setMinHeight(DimensionPolicy.pixels(context, 44));
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
        params.topMargin = DimensionPolicy.pixels(context, 4);
        parent.addView(input, params);
        ViewPolicy.afterTextChanged(input, text -> {
            onChange.accept(text.toString());
            changed();
        });
        return input;
    }

    // ---- 保存 ----

    private void save() {
        Draft current = draft;
        if (current == null || saving || PluginEditorPolicy.draftIssue(current) != null || !dirty()) return;
        saving = true;
        saveError = null;
        refreshStatus();
        PluginEditorPolicy.AuthoredPack pack = PluginEditorPolicy.packFromDraft(current);
        boolean replace = current.id != null;
        HostTask.run(this, context -> PluginStore.save(context, pack, replace), result -> {
            saving = false;
            if (result == null || !result.succeeded()) {
                saveError = result == null ? "保存失败，请重试。"
                    : PluginCatalogPolicy.errorMessage(result.code(), result.detail(), "保存失败，请重试。");
                refreshStatus();
                return;
            }
            PluginCatalogPolicy.Pack saved = result.value().pack();
            Activity activity = requireActivity();
            MsToast.show(activity, "已保存" + PluginCatalogPolicy.packKindLabel(saved) + "「" + saved.name() + "」。");
            guard.setEnabled(false);
            PluginDetailPage.markSaved(saved.kind(), saved.id());
            boolean editing = targetKind != null;
            FragmentManager manager = getParentFragmentManager();
            if (manager.isStateSaved()) return;
            // 编辑是从这个包的详情进来的：退回去，它重新可见时重读并说明还差哪一步。新建是从插件页进来的：退回插件页再打开新包的详情。
            manager.popBackStackImmediate();
            if (!editing) PluginDetailPage.open(activity, saved.kind(), saved.id(), false, true);
        });
    }

    // ---- 草稿的 saved state ----

    private static String draftJson(Draft draft) {
        try {
            JSONObject value = new JSONObject().put("kind", draft.kind).put("id", draft.id == null ? JSONObject.NULL : draft.id)
                .put("name", draft.name).put("version", draft.version).put("license", draft.license)
                .put("author", draft.author).put("description", draft.description);
            JSONArray phrases = new JSONArray();
            for (Phrase row : draft.phrases) phrases.put(new JSONArray().put(row.key()).put(row.text()));
            JSONArray commands = new JSONArray();
            for (Command row : draft.commands) commands.put(new JSONArray().put(row.trigger()).put(row.title()).put(row.template()));
            JSONArray groups = new JSONArray();
            for (DraftGroup row : draft.groups) {
                groups.put(new JSONArray().put(row.tab()).put(row.title()).put(row.keywords()).put(row.items()));
            }
            JSONArray words = new JSONArray();
            for (DraftWord row : draft.words) words.put(new JSONArray().put(row.word()).put(row.phonetic()).put(row.gloss()));
            return value.put("phrases", phrases).put("commands", commands).put("groups", groups).put("words", words).toString();
        } catch (JSONException error) {
            return "";
        }
    }

    @Nullable private static Draft draftFrom(@Nullable String text) {
        if (text == null || text.isEmpty()) return null;
        try {
            JSONObject value = new JSONObject(text);
            String kind = value.getString("kind");
            if (!PluginEditorPolicy.isAuthoredKind(kind)) return null;
            List<Phrase> phrases = new ArrayList<>();
            JSONArray rows = value.getJSONArray("phrases");
            for (int index = 0; index < rows.length(); index++) {
                JSONArray row = rows.getJSONArray(index);
                phrases.add(new Phrase(row.getString(0), row.getString(1)));
            }
            List<Command> commands = new ArrayList<>();
            rows = value.getJSONArray("commands");
            for (int index = 0; index < rows.length(); index++) {
                JSONArray row = rows.getJSONArray(index);
                commands.add(new Command(row.getString(0), row.getString(1), row.getString(2)));
            }
            List<DraftGroup> groups = new ArrayList<>();
            rows = value.getJSONArray("groups");
            for (int index = 0; index < rows.length(); index++) {
                JSONArray row = rows.getJSONArray(index);
                groups.add(new DraftGroup(row.getString(0), row.getString(1), row.getString(2), row.getString(3)));
            }
            List<DraftWord> words = new ArrayList<>();
            rows = value.getJSONArray("words");
            for (int index = 0; index < rows.length(); index++) {
                JSONArray row = rows.getJSONArray(index);
                words.add(new DraftWord(row.getString(0), row.getString(1), row.getString(2)));
            }
            return new Draft(kind, value.isNull("id") ? null : value.getString("id"), value.getString("name"),
                value.getString("version"), value.getString("license"), value.getString("author"),
                value.getString("description"), phrases, commands, groups, words);
        } catch (JSONException error) {
            return null;
        }
    }

    @Override public void onDestroyView() {
        guard.setEnabled(false);
        super.onDestroyView();
    }

}
