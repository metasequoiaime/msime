package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.InputFeatureToggle;
import app.msime.android.TextPolicy;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 表达页：标点（使用英文标点、自动补全成对标点、智能标点）、智能（整句联想三档、英文联想、候选带 emoji、候选带颜文字）和发现短语（社区短语包）。
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
    private HashSet<String> installed = new HashSet<>();
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
            Set<String> packs = new HashSet<>(phrases.ok() ? phrases.document().packs().size() : 0);
            if (phrases.ok()) for (CommonPhrasesStore.Pack pack : phrases.document().packs()) packs.add(pack.id());
            return new Loaded(values, packs);
        }, result -> {
            loaded = true;
            preferences = result == null ? null : result.preferences();
            installed = new HashSet<>(result == null ? 0 : result.packs().size());
            if (result != null) installed.addAll(result.packs());
            render();
        });
    }

    private void loadDiscover() {
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).list(CommunityRequest.Kind.PHRASE, "", 0, null),
            page -> {
                if (page == null || page.failed()) {
                    discover = null;
                    discoverFailure = page == null ? "暂时连不上社区，稍后再试。" : page.failure();
                } else {
                    List<CommunityCatalog.Item> items = page.items();
                    discover = CommunityRequest.limitedCopy(items, DISCOVER_LIMIT);
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
        // 候选里带 emoji / 颜文字（#5667）：Engine 早就会按拼音从随包的 msime-others.db 找匹配的 emoji 和颜文字插进候选（共享偏好 mixed_input.emoji / kaomoji），Android 只是没有开关，默认又是关的。
        JSONObject mixed = values.optJSONObject("mixed_input");
        intelligence.toggle("候选带 emoji", "全拼（26 键或 9 键）、双拼输入时在候选里加入匹配的 emoji，紧跟在它描绘的词后面，例如「美国」后面是 🇺🇸",
            mixed != null && mixed.optBoolean("emoji", false),
            checked -> save(edit -> mixedInput(edit).put("emoji", checked)));
        intelligence.toggle("候选带颜文字", "全拼（26 键或 9 键）、双拼输入时在候选里加入匹配的颜文字，排在同一个词的 emoji 之后",
            mixed != null && mixed.optBoolean("kaomoji", false),
            checked -> save(edit -> mixedInput(edit).put("kaomoji", checked)));

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
        List<String> parts = new ArrayList<>(2);
        if (!item.author().isEmpty()) parts.add("@" + item.author());
        JSONArray list = item.payload() == null ? null : item.payload().optJSONArray("phrases");
        if (list != null) parts.add(list.length() + " 条");
        boolean added = installed.contains(item.id());
        boolean busy = installing.contains(item.id());
        String label = added ? "已添加" : busy ? "添加中" : "添加";
        return KeyboardSheets.pillRow(context, TextPolicy.initial(item.name(), "短"), item.name(),
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

    /**
     * 要改的 `mixed_input`。共享偏好里这个对象的四个字段都必填（`MixedInputPreferences` 拒绝缺字段），快照里缺这个对象或缺字段时按共享默认值补齐再改，否则写回去会被整份拒绝。
     */
    private static JSONObject mixedInput(JSONObject preferences) throws org.json.JSONException {
        JSONObject mixed = KeyboardSheets.child(preferences, "mixed_input");
        if (!mixed.has("english")) mixed.put("english", true);
        if (!mixed.has("minimum_prefix")) mixed.put("minimum_prefix", 5);
        if (!mixed.has("emoji")) mixed.put("emoji", false);
        if (!mixed.has("kaomoji")) mixed.put("kaomoji", false);
        return mixed;
    }

    /** 0 关闭、1 标准、2 增强；缺省（`word_lattice` 默认开、`neural_keyboard` 默认关）是标准。 */
    private static int sentenceLevel(@Nullable JSONObject association) {
        boolean lattice = association == null || association.optBoolean("word_lattice", true);
        boolean neural = association != null && association.optBoolean("neural_keyboard", false);
        if (!lattice) return 0;
        return neural ? 2 : 1;
    }

    private void save(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, this::reload, this::reload);
    }
}
