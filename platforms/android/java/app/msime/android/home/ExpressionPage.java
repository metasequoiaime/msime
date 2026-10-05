package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.InputFeatureToggle;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 表达页：标点（使用英文标点、自动补全成对标点、智能标点）、智能（整句联想三档、英文联想）和发现短语（社区短语包）。
 *
 * <p>「使用英文标点」是 `chinese_punctuation` 取反显示、写入时取反；整句联想三档写 `sentence_association`：关闭是 `word_lattice=false`，标准是 `word_lattice`，增强是 `word_lattice` 加 `neural_keyboard`。发现短语列社区里 `phrase` 类的资源，「添加」经 {@link CommonPhrasesStore#installPack} 装进本机无编码常用语（它会标记云同步的「常用语」），已经装过的包显示「已添加」。
 */
public final class ExpressionPage extends DetailPage {
    private static final int DISCOVER_LIMIT = 8;
    private static final String[] SENTENCE_LABELS = {"关闭", "标准", "增强"};

    /** 偏好与已经装过的短语包 id（就是社区资源的 id）。 */
    private record Loaded(@Nullable JSONObject preferences, Set<String> packs) {}

    @Nullable private LinearLayout column;
    @Nullable private JSONObject preferences;
    private boolean loaded;
    @Nullable private List<CommunityCatalog.Item> discover;
    @Nullable private String discoverFailure;
    private final Set<String> installed = new HashSet<>();
    private final Set<String> installing = new HashSet<>();

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        if (loaded) render();
        reload();
        loadDiscover();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void reload() {
        HostTask.run(this, context -> {
            JSONObject values = KeyboardSheets.preferences(context);
            CommonPhrasesStore.Result phrases = CommonPhrasesStore.load(context);
            Set<String> packs = new HashSet<>();
            if (phrases.ok()) for (CommonPhrasesStore.Pack pack : phrases.document().packs()) packs.add(pack.id());
            return new Loaded(values, packs);
        }, result -> {
            loaded = true;
            preferences = result == null ? null : result.preferences();
            installed.clear();
            if (result != null) installed.addAll(result.packs());
            render();
        });
    }

    private void loadDiscover() {
        HostTask.run(this, context -> new CommunityCatalog(context).list(CommunityRequest.Kind.PHRASE, "", 0, null),
            page -> {
                if (page == null || page.failed()) {
                    discover = null;
                    discoverFailure = page == null ? "暂时连不上社区，稍后再试。" : page.failure();
                } else {
                    List<CommunityCatalog.Item> items = page.items();
                    discover = new ArrayList<>(items.subList(0, Math.min(DISCOVER_LIMIT, items.size())));
                    discoverFailure = null;
                }
                if (loaded) render();
            });
    }

    private void render() {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        JSONObject values = preferences;
        if (values == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();

        GroupCard punctuation = GroupCard.add(target, "标点");
        InputFeatureToggle chinese = InputFeatureToggle.CHINESE_PUNCTUATION;
        punctuation.toggle("使用英文标点", "中文模式下也上屏半角标点",
            !values.optBoolean(chinese.key(), chinese.enabledByDefault()),
            checked -> save(edit -> edit.put(chinese.key(), !checked)));
        InputFeatureToggle paired = InputFeatureToggle.PAIRED_PUNCTUATION;
        punctuation.toggle("自动补全成对标点", paired.description(),
            values.optBoolean(paired.key(), paired.enabledByDefault()),
            checked -> save(edit -> edit.put(paired.key(), checked)));
        InputFeatureToggle smart = InputFeatureToggle.SMART_PUNCTUATION;
        punctuation.toggle(smart.title(), smart.description(), values.optBoolean(smart.key(), smart.enabledByDefault()),
            checked -> save(edit -> edit.put(smart.key(), checked)));

        GroupCard intelligence = GroupCard.add(target, "智能");
        int sentence = sentenceLevel(values.optJSONObject("sentence_association"));
        intelligence.nav("整句联想", "更准确，但更耗电", SENTENCE_LABELS[sentence], () -> pickSentence(sentence));
        InputFeatureToggle english = InputFeatureToggle.ENGLISH_SUGGESTIONS;
        intelligence.toggle(english.title(), english.description(),
            values.optBoolean(english.key(), english.enabledByDefault()),
            checked -> save(edit -> edit.put(english.key(), checked)));

        GroupCard phrases = GroupCard.add(target, "发现短语").withDividers(58);
        List<CommunityCatalog.Item> items = discover;
        if (items == null) {
            phrases.note(discoverFailure == null ? "正在读取社区短语…" : discoverFailure);
        } else if (items.isEmpty()) {
            phrases.note("社区里还没有短语包。");
        } else {
            for (CommunityCatalog.Item item : items) phrases.addView(phraseRow(context, item));
        }
    }

    private android.view.View phraseRow(Context context, CommunityCatalog.Item item) {
        List<String> parts = new ArrayList<>();
        if (!item.author().isEmpty()) parts.add("@" + item.author());
        JSONArray list = item.payload() == null ? null : item.payload().optJSONArray("phrases");
        if (list != null) parts.add(list.length() + " 条");
        boolean added = installed.contains(item.id());
        boolean busy = installing.contains(item.id());
        String label = added ? "已添加" : busy ? "添加中" : "添加";
        return KeyboardSheets.pillRow(context, initial(item.name()), item.name(),
            parts.isEmpty() ? null : String.join(" · ", parts), label, added || busy ? null : () -> install(item));
    }

    private void install(CommunityCatalog.Item item) {
        installing.add(item.id());
        render();
        HostTask.run(this, context -> CommonPhrasesStore.installPack(context, item.raw()), result -> {
            installing.remove(item.id());
            if (result == null || !result.ok()) {
                MsToast.show(requireContext(), result == null ? "添加失败，请重试" : result.failure());
            } else {
                installed.add(item.id());
                MsToast.show(requireContext(), "已添加「" + item.name() + "」");
            }
            render();
        });
    }

    private void pickSentence(int selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "整句联想", "更准确，但更耗电");
        for (int index = 0; index < SENTENCE_LABELS.length; index++) {
            int level = index;
            sheet.option(SENTENCE_LABELS[index], index == selected, () -> save(edit -> {
                JSONObject association = KeyboardSheets.child(edit, "sentence_association");
                association.put("word_lattice", level > 0);
                association.put("neural_keyboard", level > 1);
            }));
        }
        sheet.show();
    }

    /** 0 关闭、1 标准、2 增强；缺省（`word_lattice` 默认开、`neural_keyboard` 默认关）是标准。 */
    private static int sentenceLevel(@Nullable JSONObject association) {
        boolean lattice = association == null || association.optBoolean("word_lattice", true);
        boolean neural = association != null && association.optBoolean("neural_keyboard", false);
        if (!lattice) return 0;
        return neural ? 2 : 1;
    }

    private static String initial(String name) {
        if (name == null || name.isEmpty()) return "短";
        return new String(Character.toChars(name.codePointAt(0)));
    }

    private void save(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, this::reload, this::reload);
    }
}
