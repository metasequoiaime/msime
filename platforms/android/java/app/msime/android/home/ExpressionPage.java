package app.msime.android.home;

import android.content.Context;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.InputFeatureToggle;
import app.msime.android.NumberPolicy;
import app.msime.android.PredictionButtonsPolicy;
import app.msime.android.TextPolicy;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 表达页：标点（使用英文标点、引号样式、方头括号样式、自动补全成对标点、智能标点）、智能（整句候选三档、上屏后联想及联想时候选栏右端的按键样式、英文联想、中英混输、触发字母数、候选带符号、候选带 emoji、候选带颜文字）和发现短语（社区里的前几个短语包，末尾「浏览更多短语」进社区短语的完整列表）。
 *
 * <p>「使用英文标点」是 `chinese_punctuation` 取反显示、写入时取反；整句候选三档写 `sentence_association`：关闭是 `word_lattice=false`，标准是 `word_lattice`，增强是 `word_lattice` 加 `neural_keyboard`。发现短语列社区里 `phrase` 类的资源，「添加」经 {@link CommonPhrasesStore#installPack} 装进本机无编码常用语（它会标记云同步的「常用语」），已经装过的包显示「已添加」。
 */
public final class ExpressionPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("标点", "智能");
    }

    private static final int DISCOVER_LIMIT = 8;
    private static final String[] SENTENCE_LABELS = {"关闭", "标准", "增强"};
    private static final String SENTENCE_DESCRIPTION =
        "输入多音节拼音时组合整句候选；标准使用词库，增强增加神经模型重排。下一词推荐由「上屏后联想」单独控制";
    /** 共享偏好 `next_word_prediction_buttons` 的显示文字，顺序与 {@link PredictionButtonsPolicy.Style} 相同，缺省是只有展开键（#10078）。 */
    private static final String[] PREDICTION_BUTTON_LABELS = {"只有展开 ∨", "关闭 × 和展开 ∨", "只有关闭 ×"};
    private static final String PREDICTION_BUTTONS_DESCRIPTION =
        "联想出现时候选栏右端的按键：× 关闭联想，不输入任何字符；∨ 展开更多联想。只有关闭时左右滑动候选栏选词";
    /** 共享偏好 `quote_style` 的取值与显示文字，缺省（文档里没有这个键）是弯引号。 */
    private static final String[] QUOTE_STYLES = {"curly", "corner", "corner_when_traditional"};
    private static final String[] QUOTE_STYLE_LABELS = {"弯引号 “ ” ‘ ’", "直角引号 「 」『 』", "繁体输出时用直角引号"};
    private static final String QUOTE_STYLE_DESCRIPTION = "中文标点下输入 ' 和 \" 时的引号。直角引号：' 出「」，\" 出『』";
    /** 共享偏好 `bracket_style` 的取值与显示文字，缺省是实心。 */
    private static final String[] BRACKET_STYLES = {"solid", "hollow"};
    private static final String[] BRACKET_STYLE_LABELS = {"实心 【 】", "空心 〖 〗"};
    private static final String BRACKET_STYLE_DESCRIPTION = "中文标点下 [ ] 键输出的括号";
    /** 中英混输的触发长度：共享偏好 `mixed_input.minimum_prefix` 的默认值和保存时校验的范围（client-core `MixedInputPreferences`）。 */
    private static final int MIXED_PREFIX_MIN = 1;
    private static final int MIXED_PREFIX_MAX = 8;
    private static final int MIXED_PREFIX_DEFAULT = 5;

    /** 偏好与已经装过的短语包 id（就是社区资源的 id）。 */
    private record Loaded(@Nullable JSONObject preferences, Set<String> packs) {}

    @Nullable private JSONObject preferences;
    private boolean loaded;
    @Nullable private List<CommunityCatalog.Item> discover;
    @Nullable private String discoverFailure;
    private HashSet<String> installed = new HashSet<>();
    private final Set<String> installing = new HashSet<>();

    @Override protected void onContentReady() {
        if (loaded) render();
    }

    @Override protected void onInitialReloadScheduled() {
        // 保留发现缓存的预取契约；常用语入口已移到设置首页，不再在本页渲染。
        loadDiscover();
    }

    @Override protected void onContentDestroyed() {
        installing.clear();
    }

    @Override protected void reload() {
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
                CommunityCatalog.Discovery result = CommunityCatalog.discovery(page, DISCOVER_LIMIT);
                discover = result.items();
                discoverFailure = result.failure();
                if (loaded) render();
            });
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        JSONObject values = preferences;
        if (values == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();

        GroupCard punctuation = GroupCard.add(sectionColumn(target, "标点"), "标点");
        InputFeatureToggle chinese = InputFeatureToggle.CHINESE_PUNCTUATION;
        punctuation.toggle("使用英文标点", null,
            !values.optBoolean(chinese.key(), chinese.enabledByDefault()),
            checked -> savePreferences(edit -> edit.put(chinese.key(), !checked))).help("中文模式下也上屏半角标点");
        // 两项只在中文标点下起作用，紧跟在中文标点的开关之后。
        int quote = styleIndex(QUOTE_STYLES, values.optString("quote_style", QUOTE_STYLES[0]));
        punctuation.nav("引号样式", null, QUOTE_STYLE_LABELS[quote],
            () -> pickStyle("引号样式", QUOTE_STYLE_DESCRIPTION, "quote_style", QUOTE_STYLES,
                QUOTE_STYLE_LABELS, quote)).help(QUOTE_STYLE_DESCRIPTION);
        int bracket = styleIndex(BRACKET_STYLES, values.optString("bracket_style", BRACKET_STYLES[0]));
        punctuation.nav("方头括号样式", null, BRACKET_STYLE_LABELS[bracket],
            () -> pickStyle("方头括号样式", BRACKET_STYLE_DESCRIPTION, "bracket_style", BRACKET_STYLES,
                BRACKET_STYLE_LABELS, bracket)).help(BRACKET_STYLE_DESCRIPTION);
        InputFeatureToggle paired = InputFeatureToggle.PAIRED_PUNCTUATION;
        punctuation.toggle("自动补全成对标点", null,
            values.optBoolean(paired.key(), paired.enabledByDefault()),
            checked -> savePreferences(edit -> edit.put(paired.key(), checked))).help(paired.description());
        InputFeatureToggle smart = InputFeatureToggle.SMART_PUNCTUATION;
        punctuation.toggle(smart.title(), null, values.optBoolean(smart.key(), smart.enabledByDefault()),
            checked -> savePreferences(edit -> edit.put(smart.key(), checked))).help(smart.description());

        GroupCard intelligence = GroupCard.add(sectionColumn(target, "智能"), "智能");
        int sentence = sentenceLevel(values.optJSONObject("sentence_association"));
        intelligence.nav("整句候选", null, SENTENCE_LABELS[sentence], () -> pickSentence(sentence)).help(SENTENCE_DESCRIPTION);
        intelligence.toggle("上屏后联想", null,
            values.optBoolean("next_word_prediction", false),
            checked -> savePreferences(edit -> edit.put("next_word_prediction", checked))).help("选词上屏后推荐后续文字，点选才上屏；使用现有词库，开启学习时也参考个人词序，隐私输入不显示");
        // 联想按键的样式只在开启上屏后联想后可选，关着时整行变灰。
        PredictionButtonsPolicy.Style buttons = PredictionButtonsPolicy.Style.parse(
            values.optString(PredictionButtonsPolicy.PREFERENCE_KEY, ""));
        intelligence.nav("联想栏按键", null, PREDICTION_BUTTON_LABELS[buttons.ordinal()],
            values.optBoolean("next_word_prediction", false) ? () -> pickStyle("联想栏按键",
                PREDICTION_BUTTONS_DESCRIPTION, PredictionButtonsPolicy.PREFERENCE_KEY, predictionButtonValues(),
                PREDICTION_BUTTON_LABELS, buttons.ordinal()) : null).help(PREDICTION_BUTTONS_DESCRIPTION);
        InputFeatureToggle english = InputFeatureToggle.ENGLISH_SUGGESTIONS;
        intelligence.toggle(english.title(), null,
            values.optBoolean(english.key(), english.enabledByDefault()),
            checked -> savePreferences(edit -> edit.put(english.key(), checked))).help(english.description());
        if (AppEdition.current().offers("quanpin")) {
            multilingualInputRows(intelligence, values);
        }
        JSONObject mixed = values.optJSONObject("mixed_input");
        // 中英混输（#7081）：Engine 和 Android 运行时早就按共享偏好 mixed_input.english / minimum_prefix 在中文候选里补英文词，桌面和 iOS 都能开关、调触发长度，Android 只是没有设置项。滑块行没有副标题，触发长度的说明只能写进开关的副标题；九键、14 键在拼音一行候选都没有且没有锁定拼音时、或处在英文模式时不看这两项照样给英文词，所以副标题不说关闭后完全没有英文。
        boolean mixedEnabled = mixed == null || mixed.optBoolean("enabled", true);
        intelligence.toggle("候选混输", null,
            mixedEnabled, checked -> savePreferences(edit -> mixedInput(edit).put("enabled", checked))).help("在中文候选中补充英文、符号、表情和快捷内容；关闭后保留各项设置");
        boolean mixedEnglish = mixed == null || mixed.optBoolean("english", true);
        intelligence.toggle("中英混输", null,
            mixedEnglish, checked -> savePreferences(edit -> mixedInput(edit).put("english", checked))).help("中文候选里补充英文；达到下方字母或按键数后补全，26 键中更短的完整词仍可选").setEnabled(mixedEnabled);
        GroupCard.Row prefixRow = intelligence.slider("触发字母数", MIXED_PREFIX_MIN, MIXED_PREFIX_MAX, 1,
            mixed == null ? MIXED_PREFIX_DEFAULT : NumberPolicy.strictInt(mixed, "minimum_prefix", MIXED_PREFIX_DEFAULT),
            Integer::toString, value -> savePreferences(edit -> mixedInput(edit).put("minimum_prefix", value)));
        prefixRow.setEnabled(mixedEnabled && mixedEnglish);
        intelligence.slider("首页英文候选数", 1, 3, 1,
            mixed == null ? 1 : NumberPolicy.strictInt(mixed, "first_page_count", 1),
            Integer::toString, value -> savePreferences(edit -> mixedInput(edit).put("first_page_count", value)))
            .setEnabled(mixedEnabled && mixedEnglish);
        // 候选带符号（打拼音出符号）：整段打出符号的名称时，Engine 在候选里接着那个词给出符号（共享偏好 mixed_input.symbols）。后加的可选字段，默认开，开着时文档里没有这个键，所以缺键读作开。
        intelligence.toggle("候选带符号", null,
            mixed == null || mixed.optBoolean("symbols", true),
            checked -> savePreferences(edit -> mixedInput(edit).put("symbols", checked))).help("全拼（26 键或 9 键）、双拼输入时，打出符号的名称就在候选里加入对应的符号，紧跟在那个词后面，例如「正负」后面是 ±").setEnabled(mixedEnabled);
        // 候选里带 emoji / 颜文字（#5667）：Engine 早就会按拼音从随包的 msime-others.db 找匹配的 emoji 和颜文字插进候选（共享偏好 mixed_input.emoji / kaomoji），Android 只是没有开关，默认又是关的。
        intelligence.toggle("候选带 emoji", null,
            mixed != null && mixed.optBoolean("emoji", false),
            checked -> savePreferences(edit -> mixedInput(edit).put("emoji", checked))).help("全拼（26 键或 9 键）、双拼输入时在候选里加入匹配的 emoji，紧跟在它描绘的词后面，例如「美国」后面是 🇺🇸").setEnabled(mixedEnabled);
        intelligence.toggle("候选带颜文字", null,
            mixed != null && mixed.optBoolean("kaomoji", false),
            checked -> savePreferences(edit -> mixedInput(edit).put("kaomoji", checked))).help("全拼（26 键或 9 键）、双拼输入时在候选里加入匹配的颜文字，排在同一个词的 emoji 之后").setEnabled(mixedEnabled);

    }

    /** 只编辑共享偏好，至少两种可用语言与日文资源门槛由 Host API 决定。 */
    private void multilingualInputRows(GroupCard group, JSONObject preferences) {
        JSONObject stored = preferences.optJSONObject("multilingual_input");
        boolean enabled = stored != null && stored.optBoolean("enabled", false);
        group.toggle("连续多语言输入（实验）", null,
            enabled, checked -> savePreferences(edit -> KeyboardSheets.child(edit, "multilingual_input").put("enabled", checked))).help("开启候选混输后，在全拼 26 键连续输入所选语言；完整中文优先、混合结果作为备选，至少需要两种可用语言");
        String[] values = {"chinese", "english", "japanese"};
        String[] labels = {"中文", "英文", "日文"};
        JSONArray selected = stored == null ? null : stored.optJSONArray("languages");
        for (int index = 0; index < values.length; index++) {
            String language = values[index];
            if ("japanese".equals(language) && !AppEdition.current().temporaryJapanese()) continue;
            boolean checked = selected == null;
            if (selected != null) for (int item = 0; item < selected.length(); item++) {
                if (language.equals(selected.optString(item))) checked = true;
            }
            group.toggle("连续输入：" + labels[index], null,
                checked, on -> savePreferences(edit -> {
                    JSONObject section = KeyboardSheets.child(edit, "multilingual_input");
                    JSONArray previous = section.optJSONArray("languages");
                    if (previous == null) previous = new JSONArray().put("chinese").put("english").put("japanese");
                    JSONArray next = new JSONArray();
                    for (int item = 0; item < previous.length(); item++) {
                        String value = previous.getString(item);
                        if (!language.equals(value)) next.put(value);
                    }
                    if (on) next.put(language);
                    section.put("languages", next);
                })).help("japanese".equals(language)
                ? "使用已有日文词典，词典未安装或准备中时暂不参与" : null).setEnabled(enabled);
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

    /** 写入引号或方头括号样式。存的就是偏好文档里的取值；写下缺省值也没关系，保存时 client-core 会把等于缺省的键省掉。 */
    private void pickStyle(String title, String description, String key, String[] values, String[] labels,
            int selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), title, description);
        for (int index = 0; index < values.length; index++) {
            String value = values[index];
            sheet.option(labels[index], index == selected, () -> savePreferences(edit -> edit.put(key, value)));
        }
        sheet.show();
    }

    /** 联想按键样式在偏好文档里的写法，顺序与 {@link #PREDICTION_BUTTON_LABELS} 相同。 */
    private static String[] predictionButtonValues() {
        PredictionButtonsPolicy.Style[] styles = PredictionButtonsPolicy.Style.values();
        String[] values = new String[styles.length];
        for (int index = 0; index < styles.length; index++) values[index] = styles[index].wire();
        return values;
    }

    /** 取值在列表里的位置；不认识的取值（更新的版本写的）按第一个即缺省显示。 */
    private static int styleIndex(String[] values, String value) {
        for (int index = 0; index < values.length; index++) if (values[index].equals(value)) return index;
        return 0;
    }

    private void pickSentence(int selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "整句候选", SENTENCE_DESCRIPTION);
        for (int index = 0; index < SENTENCE_LABELS.length; index++) {
            int level = index;
            sheet.option(SENTENCE_LABELS[index], index == selected, () -> savePreferences(edit -> {
                JSONObject association = KeyboardSheets.child(edit, "sentence_association");
                association.put("word_lattice", level > 0);
                association.put("neural_keyboard", level > 1);
            }));
        }
        sheet.show();
    }

    /**
     * 要改的 `mixed_input`。共享偏好里这个对象有四个必填字段（`MixedInputPreferences` 拒绝缺字段），快照里缺这个对象或缺字段时按共享默认值补齐再改，否则写回去会被整份拒绝。`symbols`、`japanese` 是后加的可选字段，缺省时 client-core 分别按开、关处理，这里不补。
     */
    private static JSONObject mixedInput(JSONObject preferences) throws org.json.JSONException {
        JSONObject mixed = KeyboardSheets.child(preferences, "mixed_input");
        if (!mixed.has("english")) mixed.put("english", true);
        if (!mixed.has("minimum_prefix")) mixed.put("minimum_prefix", MIXED_PREFIX_DEFAULT);
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

}
