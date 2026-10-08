package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.InputFeatureToggle;
import app.msime.android.KeyboardScheme;
import app.msime.android.NativeClient;
import app.msime.android.JsonPolicy;
import app.msime.android.ResourcePackService;
import app.msime.android.ResourcePacks;
import app.msime.android.SchemePreferences;
import app.msime.android.policy.HostOptionsPolicy;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 输入页：语言与方案、中文（字符集、拼音纠错、模糊音、云候选）、辅助码方案和翻译（候选词翻译、离线英文释义、目标语言）。
 *
 * <p>语言列表是键盘自己的方案列表 `touch_keyboard_schemes.enabled` 按语言分组后的样子：普通话一直在，其他语言在列表里有它的方案时出现。在语言的面板里选一个方案，经 {@link SchemePreferences#withScheme} 同时写 `scheme` 那几个键和 `touch_keyboard_schemes.selected`（并在缺失时加进 `enabled`），否则键盘会按旧的 `selected` 解析方案；「移除」把这门语言的方案全部移出 `enabled`，键盘正用着它时先切回剩下的第一个方案；「添加语言」把这门语言的第一个方案加进 `enabled`，不切换。粤语只有粤拼（P18：仓颉、速成没有方案和码表）；英语没有自己的触屏方案（中英切换键一直在），所以不出现在列表里。
 *
 * <p>日语的日文词典，粤拼、注音和笔画共用的语言词库，以及英文以外目标语言的离线释义都是按需下载的资源包（{@link ResourcePacks}）。还没下载的语言照样列在「添加语言」里，标出要下载的大小；添加时先把方案写进 `enabled`，再经 {@link ResourcePackService} 下载（按流量计费的网络先确认），词典到达前键盘不列出它，装好后下一次进入输入框就出现。已经添加、词典还没到的语言在列表里显示下载状态：需下载、下载中 x%、校验中、失败和原因，点开可以下载、取消、重试或移除。页面从不自己发起下载，只响应用户的点击。
 *
 * <p>辅助码方案列表来自 `NativeClient.hostCapabilities` 的 `helpcode_schemas`，没有这个字段时用不含郑码的内置列表（P14）；方案写进当前方案所属的那一份（双拼用 `shuangpin_helpcode`，其余用 `quanpin_helpcode`）。共享的辅助码设置没有「部首 / 笔画 / 混合」模式，所以本页不提供模式选择。
 */
public final class TypingPage extends DetailPage {
    /** 深链参数：打开时展开「添加语言」（键盘的输入方式面板「+ 添加语言」用）。 */
    public static final String ARG_ADD_LANGUAGE = "add_language";

    private static final String[][] HELPCODE_DEFAULTS = {
        {"ziranma", "自然码"}, {"xiaohe", "小鹤"}, {"lantian", "蓝天小雨点"}, {"shouyou2_0", "首右2.0"},
        {"shouyouplus", "首右plus"}, {"jiajia", "加加"},
    };
    private static final String[][] TARGET_LANGUAGES = {
        {"en", "英语"}, {"ja", "日语"}, {"ko", "韩语"}, {"fr", "法语"}, {"de", "德语"}, {"es", "西班牙语"}, {"ru", "俄语"},
    };

    private static final List<KeyboardScheme> SHUANGPIN = List.of(KeyboardScheme.XIAOHE, KeyboardScheme.ZIRANMA,
        KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO);

    /** 一门语言：徽标、名字和它的触屏方案（按面板里的顺序）。 */
    private enum Language {
        MANDARIN("汉", "普通话", KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.XIAOHE,
            KeyboardScheme.ZIRANMA, KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO, KeyboardScheme.WUBI,
            KeyboardScheme.ZHUYIN, KeyboardScheme.ZHUYIN_NINE_KEY, KeyboardScheme.STROKE, KeyboardScheme.HANDWRITING),
        CANTONESE("粤", "粤语", KeyboardScheme.CANTONESE),
        JAPANESE("あ", "日语", KeyboardScheme.JAPANESE, KeyboardScheme.JAPANESE_NINE_KEY),
        KOREAN("한", "韩语", KeyboardScheme.KOREAN),
        VIETNAMESE("越", "越南语", KeyboardScheme.VIETNAMESE),
        // 徽标用汉字「藏」：Java 源码里不写藏文字符（计划 G10）。
        TIBETAN("藏", "藏语", KeyboardScheme.TIBETAN);

        final String badge;
        final String title;
        final List<KeyboardScheme> schemes;

        Language(String badge, String title, KeyboardScheme... schemes) {
            this.badge = badge;
            this.title = title;
            this.schemes = List.of(schemes);
        }
    }

    /**
     * 页面渲染时读到的状态。
     *
     * @param packs 本机已具备的资源包 id：方案用的那两个按 {@link KeyboardScheme#availablePacks} 算（含随包的日文词典），其余按 {@link ResourcePacks} 报告已安装的
     * @param resourcePacks 共享层列出的每个资源包（状态和下载大小）；读不出来时为空
     * @param bundledGlosses 随安装包解压在资源目录旁的离线释义覆盖的目标语言（不带离线释义的安装包为空）
     */
    private record State(JSONObject preferences, String languageDictionaries, Set<String> packs,
            Map<String, ResourcePacks.Pack> resourcePacks, Set<String> bundledGlosses, List<String[]> helpcodeSchemas) {}

    @Nullable private LinearLayout column;
    private boolean adding;
    /** 上一次渲染用的状态：下载进度只重画这一份，不必每个百分点都去读共享存储。 */
    @Nullable private State lastState;
    /** 下载进度变了就重画；某个资源包下载结束时重读一遍，换上新的安装状态。 */
    private final ResourcePackService.Listener packListener = (pack, finished) -> {
        if (column == null) return;
        if (finished) reload();
        else if (lastState != null) render(lastState);
    };

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        if (args.getBoolean(ARG_ADD_LANGUAGE, false)) adding = true;
        ResourcePackService.addListener(packListener);
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        ResourcePackService.removeListener(packListener);
        column = null;
        lastState = null;
        super.onDestroyView();
    }

    private void reload() {
        HostTask.run(this, TypingPage::read, this::render);
    }

    @Nullable private static State read(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        Map<String, ResourcePacks.Pack> resourcePacks = new LinkedHashMap<>();
        try {
            for (ResourcePacks.Pack pack : ResourcePacks.list(context)) resourcePacks.put(pack.id(), pack);
        } catch (ResourcePacks.Failure error) {
            // 读不出资源包状态时按都没装处理：语言照样列出，标「需下载」。
            resourcePacks.clear();
        }
        String resources = HostOptionsPolicy.readOption(context.getFilesDir(), "resources");
        Set<String> packs = new HashSet<>(KeyboardScheme.availablePacks(
            id -> resourcePacks.containsKey(id) && resourcePacks.get(id).installed(), resources));
        for (ResourcePacks.Pack pack : resourcePacks.values()) if (pack.installed()) packs.add(pack.id());
        List<String> targets = new ArrayList<>(TARGET_LANGUAGES.length);
        for (String[] entry : TARGET_LANGUAGES) targets.add(entry[0]);
        return new State(preferences, HostStore.languageDictionaries(context), Set.copyOf(packs),
            Map.copyOf(resourcePacks), Set.copyOf(CandidateTranslationPolicy.offlineTargets(targets, resources)),
            helpcodeSchemas());
    }

    /** host capabilities 的 `helpcode_schemas`（字符串 id，或 `{id|schema, name|title}`）；没有或读不懂时是内置列表。 */
    private static List<String[]> helpcodeSchemas() {
        // catch 和返回都要用到它，所以声明在 try 外面；读到列表后再按它的长度预留容量。
        List<String[]> schemas = new ArrayList<>(HELPCODE_DEFAULTS.length);
        try {
            JSONObject root = new JSONObject(NativeClient.hostCapabilities("android"));
            JSONObject value = JsonPolicy.strictTrue(root.opt("ok"))
                ? root.optJSONObject("value") : null;
            JSONArray listed = value == null ? null : value.optJSONArray("helpcode_schemas");
            if (listed != null) schemas = new ArrayList<>(listed.length());
            if (listed != null) {
                for (int index = 0; index < listed.length(); index++) {
                    Object entry = listed.opt(index);
                    if (entry instanceof String id) {
                        schemas.add(new String[] {id, helpcodeLabel(id)});
                    } else if (entry instanceof JSONObject object) {
                        String id = object.optString("id", object.optString("schema", ""));
                        if (id.isEmpty()) continue;
                        String name = object.optString("name", object.optString("title", helpcodeLabel(id)));
                        schemas.add(new String[] {id, name});
                    }
                }
            }
        } catch (JSONException | RuntimeException | LinkageError error) {
            schemas.clear();
        }
        if (schemas.isEmpty()) for (String[] entry : HELPCODE_DEFAULTS) schemas.add(entry);
        return schemas;
    }

    private static String helpcodeLabel(String id) {
        for (String[] entry : HELPCODE_DEFAULTS) if (entry[0].equals(id)) return entry[1];
        return "zhengma".equals(id) ? "郑码" : id;
    }

    private void render(@Nullable State state) {
        LinearLayout target = column;
        if (target == null) return;
        lastState = state;
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();
        JSONObject preferences = state.preferences();
        AppEdition edition = AppEdition.current();
        KeyboardScheme applied = applied(preferences, edition, state.languageDictionaries(), state.packs());
        List<KeyboardScheme> enabled = enabled(preferences, edition);

        GroupCard languages = GroupCard.add(target, "语言与方案").withDividers(58);
        List<Language> addable = new ArrayList<>(Language.values().length);
        for (Language language : Language.values()) {
            List<KeyboardScheme> provided = provided(language, edition);
            if (provided.isEmpty()) continue;
            List<KeyboardScheme> offered = offered(language, edition, state);
            KeyboardScheme shown = shownScheme(language, applied, enabled);
            if (language != Language.MANDARIN && shown == null) {
                addable.add(language);
                continue;
            }
            if (offered.isEmpty()) {
                // 已经添加、词典还没到：行里显示下载状态，点开下载、取消、重试或移除。
                String pack = provided.get(0).resourcePack();
                languages.addView(KeyboardSheets.badgeNavRow(context, language.badge, language.title,
                    packState(state, pack), null, () -> showPending(language, pack, state)));
                continue;
            }
            languages.addView(KeyboardSheets.badgeNavRow(context, language.badge, language.title, null,
                shown == null ? null : schemeLabel(shown, preferences),
                () -> showLanguage(language, offered, applied, preferences, state)));
        }
        if (!addable.isEmpty()) {
            languages.addView(KeyboardSheets.actionRow(context, adding ? "✓" : "＋", adding ? "完成" : "添加语言", () -> {
                adding = !adding;
                reload();
            }));
            if (adding) {
                for (Language language : addable) {
                    KeyboardScheme first = provided(language, edition).get(0);
                    // 还没下载的语言照样列出，副标题写明要下载多少（或正在下载的进度）。
                    String subtitle = first.installed(state.languageDictionaries(), state.packs())
                        ? first.title() : first.title() + " · " + packState(state, first.resourcePack());
                    languages.addView(KeyboardSheets.pillRow(context, language.badge, language.title, subtitle,
                        "添加", () -> addLanguage(first, state)));
                }
            }
        }

        GroupCard chinese = GroupCard.add(target, "中文");
        InputFeatureToggle traditional = InputFeatureToggle.TRADITIONAL_OUTPUT;
        boolean traditionalOn = preferences.optBoolean(traditional.key(), traditional.enabledByDefault());
        chinese.nav("中文字符集", null, traditionalOn ? "繁体" : "简体", () -> pickCharset(traditionalOn));
        JSONObject quanpin = preferences.optJSONObject("quanpin");
        boolean autocorrect = quanpin == null || (quanpin.optBoolean("autocorrect_transposition", true)
            && quanpin.optBoolean("autocorrect_neighbor", true));
        chinese.toggle("拼音纠错", "纠正相邻键误触和字母顺序颠倒", autocorrect, this::saveAutocorrect);
        JSONObject fuzzy = preferences.optJSONObject("fuzzy_pinyin");
        chinese.toggle("模糊音", "如 z/zh、an/ang 不分", fuzzy != null && fuzzy.optBoolean("enabled", false),
            checked -> save(values -> KeyboardSheets.child(values, "fuzzy_pinyin").put("enabled", checked)));
        InputFeatureToggle cloud = InputFeatureToggle.CLOUD_CANDIDATES;
        chinese.toggle(cloud.title(), cloud.description(), preferences.optBoolean(cloud.key(), cloud.enabledByDefault()),
            checked -> save(values -> values.put(cloud.key(), checked)));
        // 临时日语读日文词典；词典没在本机时 host-api 会关掉它，键盘菜单里也置灰，这里给出下载入口。和桌面一样，不会因为它自己去下载。
        if (edition.temporaryJapanese() && !state.packs().contains(KeyboardScheme.JAPANESE_PACK)) {
            packRow(chinese, state, "临时日语", "用 R 键临时输入日语要用日文词典", ResourcePacks.JAPANESE);
        }

        boolean shuangpin = SHUANGPIN.contains(applied);
        String family = shuangpin ? "shuangpin_helpcode" : "quanpin_helpcode";
        JSONObject helpcode = preferences.optJSONObject(family);
        String schema = helpcode == null ? (shuangpin ? "lantian" : "ziranma") : helpcode.optString("schema", "ziranma");
        GroupCard aux = GroupCard.add(target, "辅助码");
        aux.nav("辅助码方案", shuangpin ? "双拼" : "全拼", labelOf(state.helpcodeSchemas(), schema),
            () -> pickHelpcode(family, shuangpin, state.helpcodeSchemas(), schema));

        GroupCard translation = GroupCard.add(target, "翻译");
        InputFeatureToggle translations = InputFeatureToggle.CANDIDATE_TRANSLATIONS;
        translation.toggle("候选词翻译", "为候选词附上译文；联网翻译在「我的 → 隐私」里开启",
            preferences.optBoolean(translations.key(), translations.enabledByDefault()),
            checked -> save(values -> values.put(translations.key(), checked)));
        InputFeatureToggle gloss = InputFeatureToggle.CANDIDATE_ENGLISH_GLOSS;
        boolean glossOn = preferences.optBoolean(gloss.key(), gloss.enabledByDefault());
        String targetLanguage = preferences.optString("translation_target_language", "en");
        translation.toggle("离线英文释义", "给候选词标注离线释义；英语释义随应用自带，其他目标语言要下载离线释义词典",
            glossOn, checked -> save(values -> values.put(gloss.key(), checked),
                () -> { if (checked) ensureGlosses(targetLanguage, state); }));
        translation.nav("翻译目标语言", null, labelOf(List.of(TARGET_LANGUAGES), targetLanguage),
            () -> pickTargetLanguage(targetLanguage, glossOn, state));
        if (glossOn && needsGlosses(targetLanguage, state)) {
            packRow(translation, state, "离线释义词典", "日、韩、法、德、西、俄语的离线释义", ResourcePacks.OFFLINE_GLOSSES);
        }
    }

    // ---- 按需资源包 ----

    /** 资源包的下载大小（字节）；共享层没报出来时为 0。 */
    private static long packSize(State state, String pack) {
        ResourcePacks.Pack listed = state.resourcePacks().get(pack);
        return listed == null ? 0 : listed.size();
    }

    /** 还没装好的资源包给用户看的状态：下载中 x%、校验中、下载失败和原因，或者要下载多少。 */
    private static String packState(State state, String pack) {
        String running = ResourcePackService.describe(ResourcePackService.status(pack));
        if (running != null) return running;
        long size = packSize(state, pack);
        ResourcePacks.Pack listed = state.resourcePacks().get(pack);
        String verb = listed != null && "outdated".equals(listed.state()) ? "需更新" : "需下载";
        return size > 0 ? verb + " " + ResourcePackService.size(size) : verb;
    }

    /** 一行资源包状态：没下载时「下载」，下载中「取消」，校验中按钮置灰，失败「重试」。 */
    private void packRow(GroupCard group, State state, String title, String description, String pack) {
        ResourcePackService.Status status = ResourcePackService.status(pack);
        String subtitle = status == null ? description + " · " + packState(state, pack) : packState(state, pack);
        if (status != null && status.phase() == ResourcePackService.Phase.VERIFYING) {
            group.button(title, subtitle, "校验中", () -> {}).setEnabled(false);
            return;
        }
        boolean running = status != null && status.running();
        group.button(title, subtitle, running ? "取消" : status == null ? "下载" : "重试", () -> {
            if (running) ResourcePackService.cancel(pack);
            else ResourcePackService.request(requireContext(), pack, packSize(state, pack));
        });
    }

    /** 已经添加、词典还没到的语言：下载、取消或重试，也可以移除。 */
    private void showPending(Language language, String pack, State state) {
        Context context = requireContext();
        OptionSheet sheet = new OptionSheet(context, language.title, packState(state, pack));
        ResourcePackService.Status status = ResourcePackService.status(pack);
        if (status != null && status.running()) {
            sheet.option("取消下载", false, () -> ResourcePackService.cancel(pack));
        } else {
            long size = packSize(state, pack);
            String label = status == null ? "下载" + ResourcePackService.title(pack)
                + (size > 0 ? "（" + ResourcePackService.size(size) + "）" : "") : "重试";
            sheet.option(label, false, () -> ResourcePackService.request(context, pack, size));
        }
        sheet.destructive("移除" + language.title, () -> removeLanguage(language));
        sheet.show();
    }

    /** 离线释义词典只给英语以外的目标语言用：英语释义读随包的英文词库；安装包自带这门语言的离线释义时也不用下载。 */
    private static boolean needsGlosses(String targetLanguage, State state) {
        return !"en".equals(targetLanguage) && !state.packs().contains(ResourcePacks.OFFLINE_GLOSSES)
            && !state.bundledGlosses().contains(targetLanguage);
    }

    /** 打开离线释义、或在打开时换成英语以外的目标语言：离线释义词典还没装就下载。 */
    private void ensureGlosses(String targetLanguage, State state) {
        if (!isAdded() || !needsGlosses(targetLanguage, state)) return;
        ResourcePackService.request(requireContext(), ResourcePacks.OFFLINE_GLOSSES,
            packSize(state, ResourcePacks.OFFLINE_GLOSSES));
    }

    // ---- 语言与方案 ----

    /**
     * 键盘实际在用的方案，和输入法按同一规则解析（MSIMEInputService 的 SchemeConfiguration）：在所有词典已安装的方案里找，而不是只在 `enabled` 里找。没存过 `enabled` 列表时默认列表不含注音、粤拼、笔画这类要手动开启的方案，只在里面找会把选中的 注音 9 键 显示成全拼，键盘却在打注音。
     */
    private static KeyboardScheme applied(JSONObject preferences, AppEdition edition, String dictionaries,
            Set<String> packs) {
        KeyboardScheme fromScheme = KeyboardScheme.fromPreferences(
            preferences.optString("scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);
        JSONObject schemes = preferences.optJSONObject("touch_keyboard_schemes");
        String selected = schemes == null || schemes.isNull("selected") ? null : schemes.optString("selected", null);
        List<KeyboardScheme> visible = KeyboardScheme.installedOf(List.of(KeyboardScheme.values()), dictionaries, packs, edition);
        return KeyboardScheme.resolveEnabledSelection(fromScheme, selected, visible, edition);
    }

    private static List<KeyboardScheme> enabled(JSONObject preferences, AppEdition edition) {
        return KeyboardScheme.enabledFromPreferenceIds(enabledIds(preferences), edition);
    }

    /** `touch_keyboard_schemes.enabled` 的原样内容；没有存过列表时为 null。 */
    @Nullable private static List<String> enabledIds(JSONObject preferences) {
        JSONObject schemes = preferences.optJSONObject("touch_keyboard_schemes");
        JSONArray enabled = schemes == null ? null : schemes.optJSONArray("enabled");
        if (enabled == null) return null;
        List<String> ids = new ArrayList<>(enabled.length());
        for (int index = 0; index < enabled.length(); index++) {
            if (!enabled.isNull(index)) ids.add(enabled.optString(index, ""));
        }
        return ids;
    }

    /** 本版本提供的这门语言的方案，不管词典装没装。 */
    private static List<KeyboardScheme> provided(Language language, AppEdition edition) {
        List<KeyboardScheme> provided = new ArrayList<>(language.schemes.size());
        for (KeyboardScheme scheme : language.schemes) {
            if (scheme.offeredBy(edition)) provided.add(scheme);
        }
        return provided;
    }

    /** 本版本提供、词典也已在本机的方案：键盘只列出这些。 */
    private static List<KeyboardScheme> offered(Language language, AppEdition edition, State state) {
        List<KeyboardScheme> offered = new ArrayList<>(language.schemes.size());
        for (KeyboardScheme scheme : provided(language, edition)) {
            if (scheme.installed(state.languageDictionaries(), state.packs())) offered.add(scheme);
        }
        return offered;
    }

    /** 行尾显示的方案：键盘正用着这门语言时是那个方案，否则是列表里这门语言的第一个；这门语言不在列表里时为 null。 */
    @Nullable private static KeyboardScheme shownScheme(Language language, KeyboardScheme applied,
            List<KeyboardScheme> enabled) {
        if (language.schemes.contains(applied)) return applied;
        for (KeyboardScheme scheme : enabled) if (language.schemes.contains(scheme)) return scheme;
        return null;
    }

    private static String schemeLabel(KeyboardScheme scheme, JSONObject preferences) {
        return switch (scheme) {
            case QUANPIN, QUANPIN_NINE_KEY -> "全拼";
            case CANTONESE -> "粤拼";
            case WUBI -> scheme.title(preferences.optString("wubi_profile", KeyboardScheme.WUBI_86));
            case JAPANESE -> "26 键";
            case JAPANESE_NINE_KEY -> "9 键";
            default -> scheme.title();
        };
    }

    private void showLanguage(Language language, List<KeyboardScheme> offered, KeyboardScheme applied,
            JSONObject preferences, State state) {
        Context context = requireContext();
        OptionSheet sheet = new OptionSheet(context, language.title, offered.size() > 1 ? "选择输入方案" : null);
        if (language == Language.MANDARIN) {
            boolean quanpin = applied == KeyboardScheme.QUANPIN || applied == KeyboardScheme.QUANPIN_NINE_KEY;
            KeyboardScheme quanpinChoice = applied == KeyboardScheme.QUANPIN_NINE_KEY
                ? KeyboardScheme.QUANPIN_NINE_KEY : KeyboardScheme.QUANPIN;
            if (offered.contains(quanpinChoice)) sheet.option("全拼", quanpin, () -> applyScheme(quanpinChoice, null));
            List<KeyboardScheme> shuangpin = new ArrayList<>(SHUANGPIN.size());
            for (KeyboardScheme scheme : SHUANGPIN) {
                if (offered.contains(scheme)) shuangpin.add(scheme);
            }
            if (!shuangpin.isEmpty()) {
                KeyboardScheme shown = shuangpin.contains(applied) ? applied : shuangpin.get(0);
                sheet.submenu("双拼（" + shuangpinName(shown) + "）", shuangpin.contains(applied), () -> {
                    OptionSheet next = new OptionSheet(context, "双拼", "选择双拼方案");
                    for (KeyboardScheme scheme : shuangpin)
                        next.option(shuangpinName(scheme), scheme == applied, () -> applyScheme(scheme, null));
                    return next;
                });
            }
            if (offered.contains(KeyboardScheme.WUBI)) {
                String profile = KeyboardScheme.normalizedWubiProfile(
                    preferences.optString("wubi_profile", KeyboardScheme.WUBI_86));
                boolean wubi = applied == KeyboardScheme.WUBI;
                sheet.submenu("五笔（" + wubiName(profile) + "）", wubi, () -> {
                    OptionSheet next = new OptionSheet(context, "五笔", "选择五笔方案");
                    for (String option : List.of(KeyboardScheme.WUBI_86, KeyboardScheme.WUBI_98))
                        next.option(wubiName(option), wubi && option.equals(profile),
                            () -> applyScheme(KeyboardScheme.WUBI, option));
                    return next;
                });
            }
            AppEdition edition = AppEdition.current();
            for (KeyboardScheme scheme : List.of(KeyboardScheme.ZHUYIN, KeyboardScheme.ZHUYIN_NINE_KEY,
                    KeyboardScheme.STROKE, KeyboardScheme.HANDWRITING)) {
                if (offered.contains(scheme)) {
                    sheet.option(mandarinName(scheme), scheme == applied, () -> applyScheme(scheme, null));
                } else if (scheme.offeredBy(edition) && scheme.resourcePack() != null) {
                    // 注音和笔画的词库还没到：照样列出并标明下载状态，选了先加进列表再下载，不切换。
                    sheet.option(mandarinName(scheme) + "（" + packState(state, scheme.resourcePack()) + "）", false,
                        () -> addLanguage(scheme, state));
                }
            }
        } else {
            KeyboardScheme shown = language.schemes.contains(applied) ? applied : null;
            for (KeyboardScheme scheme : offered) {
                sheet.option(schemeLabel(scheme, preferences), scheme == shown || (shown == null && offered.size() == 1),
                    () -> applyScheme(scheme, null));
            }
            sheet.destructive("移除" + language.title, () -> removeLanguage(language));
        }
        sheet.show();
    }

    private static String shuangpinName(KeyboardScheme scheme) {
        return switch (scheme) {
            case ZIRANMA -> "自然码";
            case MICROSOFT -> "微软";
            case SHOUDAO -> "首道";
            default -> "小鹤";
        };
    }

    private static String wubiName(String profile) {
        return KeyboardScheme.WUBI_98.equals(profile) ? "五笔 98" : "五笔 86";
    }

    private static String mandarinName(KeyboardScheme scheme) {
        return switch (scheme) {
            case ZHUYIN -> "注音";
            case ZHUYIN_NINE_KEY -> "注音 9 键";
            case STROKE -> "笔画";
            default -> "手写";
        };
    }

    private void applyScheme(KeyboardScheme scheme, @Nullable String wubiProfile) {
        HostTask.run(this, context -> {
            JSONObject pending = SchemePreferences.withScheme(HostStore.loadPreferences(context), scheme, wubiProfile);
            return pending == null ? null : HostStore.savePreferences(context, pending);
        }, saved -> {
            if (saved == null) MsToast.show(requireContext(), "切换失败，保留当前方案");
            reload();
        });
    }

    /**
     * 把这门语言的第一个方案加进 `enabled`，不切换键盘正在用的方案；词典还没到时，写好之后再下载它的资源包。
     *
     * <p>先写偏好再下载，与桌面「先存偏好再后台下载」一致：下载被拒绝、失败或取消时这门语言仍在列表里，行里显示下载状态，随时可以再下；词典到达前键盘不列出它。
     */
    private void addLanguage(KeyboardScheme scheme, State state) {
        AppEdition edition = AppEdition.current();
        String pack = scheme.installed(state.languageDictionaries(), state.packs()) ? null : scheme.resourcePack();
        long size = pack == null ? 0 : packSize(state, pack);
        save(preferences -> {
            List<String> ids = effectiveIds(preferences, edition);
            if (!ids.contains(scheme.preferenceId())) ids.add(scheme.preferenceId());
            KeyboardSheets.child(preferences, "touch_keyboard_schemes").put("enabled", new JSONArray(ids));
        }, () -> {
            if (pack != null) ResourcePackService.request(requireContext(), pack, size);
        });
    }

    /** 把这门语言的方案移出 `enabled`；键盘正用着它时先切回剩下的第一个方案。 */
    private void removeLanguage(Language language) {
        AppEdition edition = AppEdition.current();
        HostTask.run(this, context -> {
            JSONObject snapshot = HostStore.loadPreferences(context);
            JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            if (preferences == null) return null;
            try {
                KeyboardScheme current = applied(preferences, edition, HostStore.languageDictionaries(context),
                    KeyboardScheme.availablePacks(ResourcePacks.installedIds(context)::contains,
                        HostOptionsPolicy.readOption(context.getFilesDir(), "resources")));
                List<String> ids = effectiveIds(preferences, edition);
                for (KeyboardScheme scheme : language.schemes) ids.remove(scheme.preferenceId());
                List<KeyboardScheme> remaining = KeyboardScheme.enabledFromPreferenceIds(ids, edition);
                if (ids.isEmpty() || remaining.isEmpty()) return null;
                JSONObject schemes = KeyboardSheets.child(preferences, "touch_keyboard_schemes");
                schemes.put("enabled", new JSONArray(ids));
                JSONObject pending = snapshot;
                if (language.schemes.contains(current)) {
                    schemes.remove("selected");
                    pending = SchemePreferences.withScheme(snapshot, remaining.get(0), null);
                    if (pending == null) return null;
                }
                return HostStore.savePreferences(context, pending);
            } catch (JSONException error) {
                return null;
            }
        }, saved -> {
            if (saved == null) MsToast.show(requireContext(), "移除失败，请重试");
            reload();
        });
    }

    /** 当前生效的方案 id 列表（没存过列表时是默认启用的那些），可以修改。 */
    private static List<String> effectiveIds(JSONObject preferences, AppEdition edition) {
        List<String> stored = enabledIds(preferences);
        if (stored != null) return new ArrayList<>(stored);
        List<KeyboardScheme> enabled = KeyboardScheme.enabledFromPreferenceIds(null, edition);
        List<String> ids = new ArrayList<>(enabled.size());
        for (KeyboardScheme scheme : enabled) ids.add(scheme.preferenceId());
        return ids;
    }

    // ---- 中文、辅助码、翻译 ----

    private void pickCharset(boolean traditional) {
        OptionSheet sheet = new OptionSheet(requireContext(), "中文字符集", null);
        String key = InputFeatureToggle.TRADITIONAL_OUTPUT.key();
        sheet.option("简体", !traditional, () -> save(values -> values.put(key, false)));
        sheet.option("繁体", traditional, () -> save(values -> values.put(key, true)));
        sheet.show();
    }

    private void saveAutocorrect(boolean checked) {
        save(values -> {
            JSONObject quanpin = KeyboardSheets.child(values, "quanpin");
            quanpin.put("autocorrect_transposition", checked);
            quanpin.put("autocorrect_neighbor", checked);
        });
    }

    private void pickHelpcode(String family, boolean shuangpin, List<String[]> schemas, String selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "辅助码方案", shuangpin ? "双拼" : "全拼");
        for (String[] entry : schemas) {
            String id = entry[0];
            sheet.option(entry[1], id.equals(selected), () -> saveHelpcode(family, shuangpin, "schema", id));
        }
        sheet.show();
    }

    /** `*_helpcode` 拒绝未知字段且 `schema` 必填，缺这个对象时按各自的默认值补齐再改。 */
    private void saveHelpcode(String family, boolean shuangpin, String member, String value) {
        save(values -> {
            JSONObject helpcode = values.optJSONObject(family);
            if (helpcode == null) {
                helpcode = new JSONObject()
                    .put("enabled", true)
                    .put("schema", shuangpin ? "lantian" : "ziranma")
                    .put("show_in_candidate_window", shuangpin);
                values.put(family, helpcode);
            }
            helpcode.put(member, value);
        });
    }

    private void pickTargetLanguage(String selected, boolean glossOn, State state) {
        OptionSheet sheet = new OptionSheet(requireContext(), "翻译目标语言", null);
        for (String[] entry : TARGET_LANGUAGES) {
            String id = entry[0];
            sheet.option(entry[1], id.equals(selected), () -> save(values -> values.put("translation_target_language", id),
                () -> { if (glossOn) ensureGlosses(id, state); }));
        }
        sheet.show();
    }

    private static String labelOf(List<String[]> entries, String id) {
        Map<String, String> labels = new LinkedHashMap<>(entries.size());
        for (String[] entry : entries) labels.put(entry[0], entry[1]);
        return labels.getOrDefault(id, id);
    }

    private void save(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, this::reload, this::reload);
    }

    /** 写成功后先做 `then`（例如开始下载），再重读页面。 */
    private void save(KeyboardSheets.Edit edit, Runnable then) {
        KeyboardSheets.save(this, edit, () -> {
            then.run();
            reload();
        }, this::reload);
    }
}
