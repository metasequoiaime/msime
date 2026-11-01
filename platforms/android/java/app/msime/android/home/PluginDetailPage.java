package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.HostDeepLink;
import app.msime.android.JsonPolicy;
import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginEditorPolicy;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 一个已安装的插件：信息、内容预览、怎样用上它、编辑和删除，与共享设置页 `plugin-detail-view.tsx` 同样的内容和文字，只保留手机上生效的操作。
 *
 * <p>短语表、指令表的「启用」写 `plugins.phrase_tables` / `command_tables`（启用的排到最后，最多 16 个）；K 模式或 / 指令关着时下面说明并给出就地打开的按钮（手机的「输入」页没有「快捷模式」，桌面上那是一个去输入页的链接）。按键音效包「设为当前音效包」写 `plugins.key_sound.pack` 并打开本机的按键音开关；辅助码表「用于全拼 / 双拼」写 `plugins.helpcode_pack_*`；单词本「去背单词」在背单词里选中它；符号集装上就显示。用户自己的四种文本类包可以「编辑」（{@link PluginEditorPage}），不是内置的包可以删除，删除后偏好里指着它的选择回到默认（{@link PluginStore#withoutRemovedPack}）。
 *
 * <p>参数 `kind`、`id` 选包；`missing` 为真时这是偏好里指着一个本机没有的包的选择，页面只给出去掉它的那个操作。这一页只读和显示，外部深链带来的参数不会触发任何写入：每一次写入都要用户在页面上点。
 */
public final class PluginDetailPage extends ReloadingDetailPage {
    private static final String ARG_KIND = "kind";
    private static final String ARG_ID = "id";
    private static final String ARG_MISSING = "missing";

    /** 刚在编辑器里保存过的包（`kind/id`）：它的详情顶上说明还差哪一步才能用上，离开详情后就不再说。 */
    @Nullable private static volatile String justSaved;

    /** 打开一个包的详情；`missing` 为真时是缺失的选择。 */
    static void open(Context context, String kind, String id, boolean missing, boolean saved) {
        if (saved) markSaved(kind, id);
        Bundle args = new Bundle();
        args.putString(ARG_KIND, kind);
        args.putString(ARG_ID, id);
        args.putBoolean(ARG_MISSING, missing);
        SettingsNavigator.open(context, PageId.PLUGIN_DETAIL, args);
    }

    /** 编辑器保存之后调用：下一次显示这个包的详情时说明还差哪一步。 */
    static void markSaved(String kind, String id) {
        justSaved = kind + "/" + id;
    }

    private record State(@Nullable PluginStore.Entry entry, @Nullable PluginCatalogPolicy.Missing missing,
            @Nullable PluginCatalogPolicy.Selection selection, boolean catalogRead) {}

    private String kind = "";
    private String id = "";
    private boolean working;
    @Nullable private State state;

    @Override protected void onContentReady() {
        Bundle args = getArguments();
        kind = args == null ? "" : args.getString(ARG_KIND, "");
        id = args == null ? "" : args.getString(ARG_ID, "");
    }

    @Override protected void reload() {
        String kind = this.kind;
        String id = this.id;
        HostTask.run(this, context -> read(context, kind, id), result -> {
            state = result;
            render();
        });
    }

    private static State read(Context context, String kind, String id) {
        PluginStore.Catalog catalog = PluginStore.catalog(context);
        JSONObject preferences = KeyboardSheets.preferences(context);
        PluginCatalogPolicy.Selection selection = preferences == null ? null : PluginStore.selection(context, preferences);
        if (catalog == null) return new State(null, null, selection, false);
        PluginStore.Entry entry = catalog.find(kind, id);
        PluginCatalogPolicy.Missing missing = null;
        if (selection != null && (entry == null || !PluginCatalogPolicy.usedOnPhone(entry.pack()))) {
            List<PluginCatalogPolicy.Pack> packs = new java.util.ArrayList<>();
            for (PluginStore.Entry each : catalog.entries()) packs.add(each.pack());
            for (PluginCatalogPolicy.Missing candidate : PluginCatalogPolicy.missingSelections(selection, packs)) {
                if (candidate.kind().equals(kind) && candidate.id().equals(id)) missing = candidate;
            }
        }
        return new State(entry, missing, selection, true);
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        Context context = requireContext();
        State current = state;
        if (current == null || !current.catalogRead() || current.selection() == null) {
            GroupCard.add(target, null).note(current == null ? "正在读取插件…" : "插件列表没有读取成功，重新打开这个页面会再试一次。");
            return;
        }
        PluginCatalogPolicy.Selection selection = current.selection();
        if (current.missing() != null) {
            renderMissing(target, current.missing());
            return;
        }
        PluginStore.Entry entry = current.entry();
        if (entry == null) {
            setTitle(id);
            GroupCard.add(target, null).note("这个插件已不在本机，可能被删除或无法载入。");
            return;
        }
        PluginCatalogPolicy.Pack pack = entry.pack();
        setTitle(pack.name());

        if ((pack.kind() + "/" + pack.id()).equals(justSaved)) {
            boolean phrase = PluginCatalogPolicy.PHRASE_TABLE.equals(pack.kind());
            List<String> tables = phrase ? selection.phraseTables() : selection.commandTables();
            GroupCard.add(target, null).note(PluginEditorPolicy.authoredSaveNote(pack.kind(), tables.contains(pack.id()),
                phrase ? selection.quickPhraseMode() : selection.commandMode()));
        }

        GroupCard info = GroupCard.add(target, "信息");
        info.value("类型", null, PluginCatalogPolicy.packKindLabel(pack));
        info.value("版本", null, pack.version());
        if (!pack.author().isEmpty()) info.value("作者", null, pack.author());
        info.value("许可证", null, pack.license());
        if (pack.builtin()) info.value("来源", null, "内置，不能删除");
        if (!pack.description().isEmpty()) info.note(pack.description());

        renderContent(target, entry);

        GroupCard use = GroupCard.add(target, "使用");
        renderActions(context, use, pack, selection);

        if (!pack.builtin() && PluginEditorPolicy.isAuthoredKind(pack.kind())) {
            GroupCard edit = GroupCard.add(target, null);
            edit.button("编辑", null, "编辑", () -> {
                justSaved = null;
                PluginEditorPage.edit(context, pack.kind(), pack.id());
            }).help("不用写代码，在设置页里修改名字、介绍和内容；保存后替换本机的这一份。");
        }
        if (!pack.builtin()) {
            GroupCard remove = GroupCard.add(target, null);
            GroupCard.Row row = remove.button("删除", "删除后包里的文件会从本机移除；正在使用的话，设置会回到默认选择。", "删除",
                () -> confirmRemove(pack));
            if (row.button() != null) row.button().setEnabled(!working);
        }
    }

    /** 各类型自己的内容：短语表的前几行、指令表的指令、辅助码表的前几条、单词本的前几个单词、符号集的各组。 */
    private void renderContent(LinearLayout target, PluginStore.Entry entry) {
        JSONObject raw = entry.raw();
        switch (entry.pack().kind()) {
            case PluginCatalogPolicy.SYMBOL_SET -> {
                JSONArray groups = raw.optJSONArray("groups");
                for (int index = 0; groups != null && index < groups.length(); index++) {
                    JSONObject group = groups.optJSONObject(index);
                    if (group == null) continue;
                    JSONArray items = group.optJSONArray("items");
                    int count = items == null ? 0 : items.length();
                    StringBuilder shown = new StringBuilder();
                    for (int item = 0; item < Math.min(count, PluginCatalogPolicy.SYMBOL_PREVIEW_ITEMS); item++) {
                        if (item > 0) shown.append(' ');
                        shown.append(JsonPolicy.strictStringOrEmpty(items.opt(item)));
                    }
                    if (count > PluginCatalogPolicy.SYMBOL_PREVIEW_ITEMS) shown.append(" …");
                    String tab = "kaomoji".equals(JsonPolicy.strictStringOrEmpty(group.opt("tab"))) ? "颜文字" : "符号";
                    GroupCard.add(target, tab + " · " + JsonPolicy.strictStringOrEmpty(group.opt("title")) + "（" + count + "）")
                        .note(shown.toString());
                }
            }
            case PluginCatalogPolicy.WORDBOOK -> {
                JSONArray words = raw.optJSONArray("first_words");
                int listed = words == null ? 0 : Math.min(words.length(), PluginCatalogPolicy.WORDBOOK_PREVIEW_WORDS);
                long total = JsonPolicy.nonNegativeLong(raw.opt("word_count"), listed);
                GroupCard group = GroupCard.add(target, "单词（" + total + "）");
                for (int index = 0; index < listed; index++) group.value(JsonPolicy.strictStringOrEmpty(words.opt(index)), null, null);
                if (total > listed) group.note("只显示前 " + listed + " 个单词。");
            }
            case PluginCatalogPolicy.HELPCODE -> {
                JSONArray preview = raw.optJSONArray("preview");
                int listed = preview == null ? 0 : preview.length();
                long total = JsonPolicy.nonNegativeLong(raw.opt("entries"), listed);
                GroupCard group = GroupCard.add(target, "辅助码（" + total + " 个字）");
                for (int index = 0; index < listed; index++) {
                    JSONObject row = preview.optJSONObject(index);
                    if (row != null) {
                        group.value(JsonPolicy.strictStringOrEmpty(row.opt("character")),
                            JsonPolicy.strictStringOrEmpty(row.opt("code")), null);
                    }
                }
                if (total > listed) group.note("只显示前 " + listed + " 个字。");
            }
            case PluginCatalogPolicy.PHRASE_TABLE -> {
                JSONArray phrases = raw.optJSONArray("phrases");
                int count = phrases == null ? 0 : phrases.length();
                GroupCard group = GroupCard.add(target, "短语（" + count + "）");
                for (int index = 0; index < Math.min(count, PluginCatalogPolicy.PHRASE_PREVIEW_ROWS); index++) {
                    JSONObject row = phrases.optJSONObject(index);
                    if (row != null) {
                        group.value(JsonPolicy.strictStringOrEmpty(row.opt("key")),
                            JsonPolicy.strictStringOrEmpty(row.opt("text")), null);
                    }
                }
                if (count > PluginCatalogPolicy.PHRASE_PREVIEW_ROWS) {
                    group.note("只显示前 " + PluginCatalogPolicy.PHRASE_PREVIEW_ROWS + " 条。");
                }
            }
            case PluginCatalogPolicy.COMMAND_TABLE -> {
                JSONArray commands = raw.optJSONArray("commands");
                int count = commands == null ? 0 : commands.length();
                GroupCard group = GroupCard.add(target, "指令（" + count + "）");
                if (count == 0) group.note("没有指令。");
                for (int index = 0; index < count; index++) {
                    JSONObject row = commands.optJSONObject(index);
                    if (row != null) {
                        group.value("/" + JsonPolicy.strictStringOrEmpty(row.opt("trigger")),
                            JsonPolicy.strictStringOrEmpty(row.opt("title")), null);
                    }
                }
            }
            default -> { }
        }
    }

    private void renderActions(Context context, GroupCard use, PluginCatalogPolicy.Pack pack,
            PluginCatalogPolicy.Selection selection) {
        switch (pack.kind()) {
            case PluginCatalogPolicy.SOUND -> {
                boolean selected = pack.id().equals(selection.keySoundPack());
                boolean playing = selected && selection.keySoundOn();
                // 按键音关着是当前状态，留在副标题；音效包和开关的关系收进「?」。
                GroupCard.Row row = use.button("按键音效",
                    selection.keySoundOn() ? null : "按键音现在没有打开。设为当前音效包会同时打开按键音。",
                    selected ? (playing ? "使用中" : "打开按键音") : "设为当前音效包", () -> selectKeySound(pack.id()))
                    .help("按键音取自当前音效包；开关在「键盘 → 按键反馈」里。");
                if (row.button() != null) row.button().setEnabled(!playing && !working);
            }
            case PluginCatalogPolicy.COMMAND_TABLE, PluginCatalogPolicy.PHRASE_TABLE -> {
                boolean phrase = PluginCatalogPolicy.PHRASE_TABLE.equals(pack.kind());
                List<String> tables = phrase ? selection.phraseTables() : selection.commandTables();
                GroupCard.Row toggle = use.toggle("启用", PluginCatalogPolicy.tableSwitchDescription(pack.kind(), tables, pack.id()),
                    tables.contains(pack.id()), checked -> setTable(pack.kind(), pack.id(), checked));
                toggle.setEnabled(PluginCatalogPolicy.tableSwitchEnabled(tables, pack.id()));
                if (phrase && !selection.quickPhraseMode()) {
                    use.button("快捷短语（K 模式）", "快捷短语（K 模式）已关闭，启用的短语表不会出候选。打开后，在键盘的「本地输入」里点「快捷短语」再输入编码即可用到短语表。",
                        "打开", () -> setLocalMode("quick_phrase"));
                } else if (!phrase && !selection.commandMode()) {
                    use.button("/ 指令", "/ 指令已关闭，启用的指令表不会生效。打开后，在中文标点下没有输入时按 / 再输入指令字母即可使用，也可以从键盘的「本地输入」里点「指令」。",
                        "打开 / 指令", () -> setLocalMode("command"));
                }
            }
            case PluginCatalogPolicy.HELPCODE -> {
                use.toggle("用于全拼", null, pack.id().equals(selection.helpcodeQuanpin()),
                    on -> setHelpcode("helpcode_pack_quanpin", on ? pack.id() : "")).help("替换全拼的辅助码方案；关闭后回到原来的方案。");
                use.toggle("用于双拼", null, pack.id().equals(selection.helpcodeShuangpin()),
                    on -> setHelpcode("helpcode_pack_shuangpin", on ? pack.id() : "")).help("替换双拼的辅助码方案；关闭后回到原来的方案。");
                use.button("辅助码设置", null, "前往辅助码设置", () -> {
                    Bundle args = new Bundle();
                    args.putString(HostDeepLink.ARG_SECTION, "辅助码");
                    SettingsNavigator.open(context, PageId.TYPING, args);
                }).help("辅助码的开关和显示方式在「输入 → 辅助码」里。");
            }
            case PluginCatalogPolicy.WORDBOOK -> use.button("背单词", null, "去背单词",
                () -> VocabularyReviewPage.openBook(context, PluginCatalogPolicy.wordbookBookId(pack.id()))).help("这本书出现在背单词的词书里。卸载插件后复习进度仍会保留，重新安装后可以接着复习。");
            case PluginCatalogPolicy.SYMBOL_SET -> use.help(
                "装上即在符号面板里显示：符号组以插件名为分类排在内置符号之后，颜文字组排在「颜文字」之后。卸载后不再显示。");
            default -> use.note("这台设备上不使用这类插件，可以在电脑上使用。");
        }
    }

    private void renderMissing(LinearLayout target, PluginCatalogPolicy.Missing missing) {
        setTitle(PluginCatalogPolicy.missingTitle(missing));
        GroupCard group = GroupCard.add(target, null);
        group.note(PluginCatalogPolicy.missingNote(missing));
        group.button(PluginCatalogPolicy.missingAction(missing), null, PluginCatalogPolicy.missingAction(missing), () ->
            KeyboardSheets.save(this, preferences -> {
                if (missing.mismatched()) {
                    KeyboardSheets.child(KeyboardSheets.child(preferences, "plugins"), "key_sound")
                        .put("pack", PluginCatalogPolicy.DEFAULT_SOUND_PACK);
                } else {
                    PluginStore.withoutRemovedPack(preferences, missing.kind(), missing.id());
                }
            }, this::back, this::reload));
    }

    private void selectKeySound(String pack) {
        working = true;
        HostTask.run(this, context -> PluginStore.selectKeySound(context, pack) ? Boolean.TRUE : null, saved -> {
            working = false;
            if (saved == null) MsToast.show(requireContext(), "保存失败，请重试");
            reload();
        });
    }

    private void setTable(String kind, String pack, boolean enabled) {
        savePreferences(preferences -> {
            JSONObject plugins = preferences.optJSONObject("plugins");
            JSONArray stored = plugins == null ? null
                : plugins.optJSONArray(PluginCatalogPolicy.PHRASE_TABLE.equals(kind) ? "phrase_tables" : "command_tables");
            List<String> tables = new java.util.ArrayList<>();
            for (int index = 0; stored != null && index < stored.length(); index++) {
                String id = JsonPolicy.strictString(stored.opt(index));
                if (id != null && !id.isEmpty()) tables.add(id);
            }
            PluginStore.putTables(preferences, kind, PluginCatalogPolicy.withTable(tables, pack, enabled));
        });
    }

    /** 就地打开快捷模式：`local_modes.quick_phrase`（K 模式）或 `local_modes.command`（/ 指令）。 */
    private void setLocalMode(String key) {
        savePreferences(preferences -> KeyboardSheets.child(preferences, "local_modes").put(key, true));
    }

    private void setHelpcode(String key, String value) {
        savePreferences(preferences -> KeyboardSheets.child(preferences, "plugins").put(key, value));
    }

    private void confirmRemove(PluginCatalogPolicy.Pack pack) {
        MessageDialog.confirm(requireContext(), "删除" + PluginCatalogPolicy.packKindLabel(pack),
            "删除「" + pack.name() + "」？包里的文件会从本机移除。", "删除", () -> {
                working = true;
                render();
                HostTask.run(this, context -> {
                    PluginStore.Result<Boolean> removed = PluginStore.remove(context, pack.kind(), pack.id());
                    if (!removed.succeeded()) return removed;
                    KeyboardSheets.write(context, preferences -> PluginStore.withoutRemovedPack(preferences, pack.kind(), pack.id()));
                    return removed;
                }, result -> {
                    working = false;
                    if (result == null || !result.succeeded()) {
                        MsToast.show(requireContext(), result == null ? "删除失败，请重试。"
                            : PluginCatalogPolicy.errorMessage(result.code(), result.detail(), "删除失败，请重试。"));
                        reload();
                        return;
                    }
                    MsToast.show(requireContext(), "已删除「" + pack.name() + "」。");
                    back();
                });
            });
    }

    private void back() {
        if (getActivity() != null) requireActivity().getOnBackPressedDispatcher().onBackPressed();
    }

    @Override public void onDestroy() {
        // 离开详情就不再提醒刚保存过；编辑器保存后会重新标记。
        if (isRemoving() && (kind + "/" + id).equals(justSaved)) justSaved = null;
        super.onDestroy();
    }
}
