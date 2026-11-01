package app.msime.android.home;

import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.FirstRunPreparation;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.InputFeatureToggle;
import app.msime.android.KeyboardScheme;
import app.msime.android.NativeClient;
import app.msime.android.NineKeyShuangpinKeypad;
import app.msime.android.JsonPolicy;
import app.msime.android.ResourcePackService;
import app.msime.android.ResourcePacks;
import app.msime.android.SchemePreferences;
import app.msime.android.policy.HostOptionsPolicy;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.core.InputViewValuePolicy;
import java.io.IOException;
import java.io.InputStream;
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
 * 输入页：语言与方案、中文（字符集、中英文之间加空格、拼音纠错、模糊音、云候选）、辅助码方案和翻译（候选词翻译、翻译范围、离线英文释义、目标语言）。
 *
 * <p>语言与方案列表是键盘自己的方案列表 `touch_keyboard_schemes.enabled` 按方案族分组后的样子：全拼、双拼、五笔、注音、笔画、手写和粤拼各自成行，其他语言按语言成行。在一行的面板里选一个方案，经 {@link SchemePreferences#withScheme} 同时写 `scheme` 那几个键和 `touch_keyboard_schemes.selected`（并在缺失时加进 `enabled`）；「移除」把这一行的方案全部移出 `enabled`，键盘正用着它时先切回剩下的第一个方案；「添加方案」把这一行的第一个方案加进 `enabled`，不切换。英语没有自己的触屏方案（中英切换键一直在），所以不出现在列表里。
 *
 * <p>日语的日文词典，粤拼、注音和笔画共用的语言词库，以及英文以外目标语言的离线释义都是按需下载的资源包（{@link ResourcePacks}）。还没下载的方案照样列在「添加方案」里，标出要下载的大小；添加时先把方案写进 `enabled`，再经 {@link ResourcePackService} 下载（按流量计费的网络先确认），词典到达前键盘不列出它，装好后下一次进入输入框就出现。已经添加、词典还没到的方案在列表里显示下载状态：需下载、下载中 x%、校验中、失败和原因，点开可以下载、取消、重试或移除。页面从不自己发起下载，只响应用户的点击。
 *
 * <p>辅助码方案列表先是内置方案，来自 `NativeClient.hostCapabilities` 的 `helpcode_schemas`，没有这个字段时用不含郑码的内置列表（P14）；后面是用户导入的码表（#7176），即 `msime_client_plugins` 的 `catalog` 里 `kind = "helpcode"` 的包。开关和方案都写进当前方案所属的那一份（双拼用 `shuangpin_helpcode`，其余用 `quanpin_helpcode`），选导入的码表写 `plugins.helpcode_pack_*`，规则见 {@link HelpcodeChoicePolicy}；关掉时不再列出方案。「导入辅助码表…」经系统文件选择器（SAF）选一个 `.txt`，严格按 UTF-8 解码后交给 `import_helpcode_table`，成功后自动选中它；「管理已导入的码表」里可以删除，删的是正在用的码表时退回内置方案。共享的辅助码设置没有「部首 / 笔画 / 混合」模式，所以本页不提供模式选择。
 */
public final class TypingPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("语言", "中文", "辅助码", "翻译");
    }

    /** 深链参数：打开时展开「添加语言」（键盘的输入方式面板「+ 添加语言」用）。 */
    public static final String ARG_ADD_LANGUAGE = "add_language";

    private static final String[][] HELPCODE_DEFAULTS = {
        {"ziranma", "自然码"}, {"xiaohe", "小鹤"}, {"lantian", "蓝天小雨点"}, {"shouyou2_0", "首右2.0"},
        {"shouyouplus", "首右plus"}, {"jiajia", "加加"}, {"wubi86", "五笔 86"},
    };
    private static final String[][] TARGET_LANGUAGES = {
        {"en", "英语"}, {"ja", "日语"}, {"ko", "韩语"}, {"fr", "法语"}, {"de", "德语"}, {"es", "西班牙语"}, {"ru", "俄语"},
    };

    private static final List<KeyboardScheme> SHUANGPIN = List.of(KeyboardScheme.XIAOHE, KeyboardScheme.ZIRANMA,
        KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO);

    /** 手机设置页的一行：中文方案族独立成行，其它语言按语言成行。 */
    private enum Language {
        QUANPIN("拼", "全拼", KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_FOURTEEN_KEY,
            KeyboardScheme.QUANPIN_EIGHTEEN_KEY, KeyboardScheme.QUANPIN_NINE_KEY),
        SHUANGPIN("双", "双拼", KeyboardScheme.XIAOHE, KeyboardScheme.ZIRANMA,
            KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO, KeyboardScheme.NINE_KEY_SHUANGPIN),
        WUBI("五", "五笔", KeyboardScheme.WUBI),
        ZHUYIN("注", "注音", KeyboardScheme.ZHUYIN, KeyboardScheme.ZHUYIN_NINE_KEY),
        STROKE("笔", "笔画", KeyboardScheme.STROKE),
        HANDWRITING("写", "手写", KeyboardScheme.HANDWRITING),
        CANTONESE("粤", "粤拼", KeyboardScheme.CANTONESE, KeyboardScheme.CANTONESE_SHUANGPIN),
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
     * @param helpcodePacks 用户导入的辅助码表（已安装的 `helpcode` 插件包）；读不出来时为空
     */
    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local, String languageDictionaries, Set<String> packs,
            Map<String, ResourcePacks.Pack> resourcePacks, Set<String> bundledGlosses, List<String[]> helpcodeSchemas,
            List<HelpcodeChoicePolicy.Pack> helpcodePacks) {}

    /** 进程被杀后恢复「导入辅助码表」是为哪一份方案发起的。 */
    private static final String SAVED_IMPORT_SHUANGPIN = "helpcode_import_shuangpin";

    private boolean adding;
    private boolean editingCloudCustom;
    /** 上一次渲染用的状态：下载进度只重画这一份，不必每个百分点都去读共享存储。 */
    @Nullable private State lastState;
    /** 下载进度变了就重画；某个资源包下载结束时重读一遍，换上新的安装状态。 */
    private final ResourcePackService.Listener packListener = (pack, finished) -> {
        if (contentColumn() == null) return;
        if (finished) reload();
        else if (lastState != null) render(lastState);
    };
    /** 正在进行的码表导入是为双拼（true）还是全拼那一份发起的：导入成功后选中到这一份。 */
    private boolean importShuangpin;
    /** 这个视图上次看到的准备状态；observe() 交付当前状态之前为 null。 */
    @Nullable private FirstRunPreparation.State preparationSeen;
    /** 随包按需组在后台补齐后，已打开的输入页也要显示刚恢复的方案与查码入口。 */
    private final FirstRunPreparation.Listener preparationListener = state -> {
        FirstRunPreparation.State previous = preparationSeen;
        preparationSeen = state;
        // The first callback is observe()'s synchronous replay of the current state; the base class already reloads after onContentReady, so only a later change to READY needs a refresh.
        if (previous != null && previous != FirstRunPreparation.State.READY
                && state == FirstRunPreparation.State.READY && contentColumn() != null) reload();
    };
    private final ActivityResultLauncher<String[]> openTable =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onTablePicked);

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        if (saved != null) importShuangpin = saved.getBoolean(SAVED_IMPORT_SHUANGPIN, false);
    }

    @Override public void onSaveInstanceState(@NonNull Bundle out) {
        super.onSaveInstanceState(out);
        out.putBoolean(SAVED_IMPORT_SHUANGPIN, importShuangpin);
    }

    @Override protected void onContentReady() {
        Bundle args = getArguments();
        if (args != null && args.getBoolean(ARG_ADD_LANGUAGE, false)) adding = true;
        ResourcePackService.addListener(packListener);
        FirstRunPreparation.observe(preparationListener);
    }

    @Override protected void onContentDestroyed() {
        ResourcePackService.removeListener(packListener);
        FirstRunPreparation.stopObserving(preparationListener);
        preparationSeen = null;
        lastState = null;
        editingCloudCustom = false;
    }

    @Override protected void reload() {
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
        return new State(preferences, AndroidLocalSettings.load(context), HostStore.languageDictionaries(context), Set.copyOf(packs),
            Map.copyOf(resourcePacks), Set.copyOf(CandidateTranslationPolicy.offlineTargets(targets, resources)),
            helpcodeSchemas(), helpcodePacks(context));
    }

    /** 已导入的辅助码表：`msime_client_plugins` 的 `catalog` 里 `kind = "helpcode"` 的包。读不出来时为空，方案列表照样列出内置方案。 */
    private static List<HelpcodeChoicePolicy.Pack> helpcodePacks(Context context) {
        List<HelpcodeChoicePolicy.Pack> packs = new ArrayList<>();
        try {
            JSONObject response = plugins(context, new JSONObject().put("operation", "catalog"));
            JSONObject value = JsonPolicy.objectValue(response);
            JSONArray packages = value == null ? null : value.optJSONArray("packages");
            if (packages == null) return List.of();
            for (int index = 0; index < packages.length(); index++) {
                JSONObject pack = packages.optJSONObject(index);
                if (pack == null || !"helpcode".equals(pack.optString("kind", ""))) continue;
                String id = pack.optString("id", "");
                if (id.isEmpty()) continue;
                String name = pack.optString("name", "");
                packs.add(new HelpcodeChoicePolicy.Pack(id, name.isEmpty() ? id : name));
            }
        } catch (JSONException | RuntimeException | LinkageError error) {
            return List.of();
        }
        return List.copyOf(packs);
    }

    /** 一次 `msime_client_plugins` 调用：插件目录是键盘读的那一份 runtime option `preferences_directory`（{@link HostStore#directory}），不自己拼路径。读写文件，要在工作线程上调用。 */
    private static JSONObject plugins(Context context, JSONObject action) throws JSONException {
        JSONObject request = new JSONObject()
            .put("state_root", HostStore.directory(context))
            .put("sound_packs", JSONObject.NULL)
            .put("action", action);
        return new JSONObject(NativeClient.plugins(request.toString()));
    }

    /** host capabilities 的 `helpcode_schemas`（字符串 id，或 `{id|schema, name|title}`）；没有或读不懂时是内置列表。 */
    private static List<String[]> helpcodeSchemas() {
        // catch 和返回都要用到它，所以声明在 try 外面；读到列表后再按它的长度预留容量。
        List<String[]> schemas = new ArrayList<>(HELPCODE_DEFAULTS.length);
        try {
            JSONObject root = new JSONObject(NativeClient.hostCapabilities("android"));
            JSONObject value = JsonPolicy.objectValue(root);
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
        LinearLayout target = contentColumn();
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

        GroupCard languages = GroupCard.add(sectionColumn(target, "语言"), "语言与方案").withDividers(58);
        List<Language> addable = new ArrayList<>(Language.values().length);
        for (Language language : Language.values()) {
            List<KeyboardScheme> provided = provided(language, edition);
            if (provided.isEmpty()) continue;
            List<KeyboardScheme> offered = offered(language, edition, state);
            KeyboardScheme shown = shownScheme(language, applied, enabled);
            if (shown == null) {
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
            if (language == Language.SHUANGPIN && offered.contains(KeyboardScheme.NINE_KEY_SHUANGPIN)) {
                // 九键双拼是一种键盘加一张表：面板里只选中这个键盘，表在这一页里选、编辑、导入和导出，与 iOS 的「九键双拼方案与自定义」相同。
                String table = NineKeyShuangpinKeypad.normalizedProfile(InputViewValuePolicy.textOr(preferences,
                    "nine_key_shuangpin_profile", NineKeyShuangpinKeypad.DEFAULT_PROFILE));
                languages.nav("九键双拼方案与自定义", null, NineKeyShuangpinKeypad.profileTitle(table),
                    () -> SettingsNavigator.open(context, PageId.NINE_KEY_SHUANGPIN, null));
            }
            if (language == Language.JAPANESE && offered.contains(KeyboardScheme.JAPANESE_NINE_KEY)) {
                InputFeatureToggle toggle = InputFeatureToggle.JAPANESE_TOGGLE_INPUT;
                languages.toggle(toggle.title(), null,
                    preferences.optBoolean(toggle.key(), toggle.enabledByDefault()),
                    checked -> savePreferences(values -> values.put(toggle.key(), checked))).help(toggle.description());
            }
        }
        if (!addable.isEmpty()) {
            languages.addView(KeyboardSheets.actionRow(context, adding ? "✓" : "＋", adding ? "完成" : "添加方案", () -> {
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
        languages.nav("联系人", null, null,
            () -> SettingsNavigator.open(context, PageId.CONTACTS, null)).help("导入姓名和手机号，输入姓名时优先显示");

        // 五笔查码（#6789）：本版本的方案列表里启用了五笔、五笔码表也在本机时才列出，与共享设置页「五笔查码」一组出现的条件一致。瘦包还没下载码表时查不出任何编码，五笔那一行给出下载入口。
        if (enabled.contains(KeyboardScheme.WUBI)
                && KeyboardScheme.WUBI.installed(state.languageDictionaries(), state.packs())) {
            GroupCard wubi = GroupCard.add(sectionColumn(target, "中文"), "五笔");
            wubi.nav(PageId.WUBI_LOOKUP.title(), null, null,
                () -> SettingsNavigator.open(context, PageId.WUBI_LOOKUP, null)).help("查汉字的 86、98 五笔编码");
        }

        GroupCard chinese = GroupCard.add(sectionColumn(target, "中文"), "中文");
        InputFeatureToggle traditional = InputFeatureToggle.TRADITIONAL_OUTPUT;
        boolean traditionalOn = preferences.optBoolean(traditional.key(), traditional.enabledByDefault());
        chinese.nav("中文字符集", null, traditionalOn ? "繁体" : "简体", () -> pickCharset(traditionalOn));
        InputFeatureToggle spacing = InputFeatureToggle.CHINESE_ENGLISH_SPACING;
        chinese.toggle(spacing.title(), null,
            preferences.optBoolean(spacing.key(), spacing.enabledByDefault()),
            checked -> savePreferences(values -> values.put(spacing.key(), checked))).help(spacing.description());
        JSONObject quanpin = preferences.optJSONObject("quanpin");
        boolean autocorrect = quanpin == null || (quanpin.optBoolean("autocorrect_transposition", true)
            && quanpin.optBoolean("autocorrect_neighbor", true));
        chinese.toggle("拼音纠错", null, autocorrect, this::saveAutocorrect).help("纠正相邻键误触和字母顺序颠倒");
        JSONObject fuzzy = preferences.optJSONObject("fuzzy_pinyin");
        chinese.toggle("模糊音", null, fuzzy != null && fuzzy.optBoolean("enabled", false),
            checked -> savePreferences(values -> KeyboardSheets.child(values, "fuzzy_pinyin").put("enabled", checked))).help("如 z/zh、an/ang 不分");
        chinese.nav("模糊音规则", null, null,
            () -> SettingsNavigator.open(context, PageId.FUZZY_PINYIN, null)).help("逐项选择需要的声母和韵母");
        InputFeatureToggle single = InputFeatureToggle.SINGLE_CHARACTER_ONLY;
        chinese.toggle(single.title(), null,
            preferences.optBoolean(single.key(), single.enabledByDefault()),
            checked -> savePreferences(values -> values.put(single.key(), checked))).help(single.description());
        InputFeatureToggle backspace = InputFeatureToggle.BACKSPACE_REVERTS_SELECTION;
        chinese.toggle(backspace.title(), null,
            preferences.optBoolean(backspace.key(), backspace.enabledByDefault()),
            on -> savePreferences(values -> values.put(backspace.key(), on))).help(backspace.description());
        InputFeatureToggle cloud = InputFeatureToggle.CLOUD_CANDIDATES;
        chinese.toggle(cloud.title(), null, preferences.optBoolean(cloud.key(), cloud.enabledByDefault()),
            checked -> savePreferences(values -> values.put(cloud.key(), checked))).help("联网获取当前拼写的候选");
        cloudProviderSettings(target, preferences);
        // 临时日语读日文词典；词典没在本机时 host-api 会关掉它，键盘菜单里也置灰，这里给出下载入口。和桌面一样，不会因为它自己去下载。
        if (edition.temporaryJapanese() && !state.packs().contains(KeyboardScheme.JAPANESE_PACK)) {
            packRow(chinese, state, "临时日语", "用 R 键临时输入日语要用日文词典", ResourcePacks.JAPANESE);
        }

        boolean shuangpin = SHUANGPIN.contains(applied);
        String family = shuangpin ? "shuangpin_helpcode" : "quanpin_helpcode";
        JSONObject helpcode = preferences.optJSONObject(family);
        String schema = helpcode == null ? (shuangpin ? "lantian" : "ziranma") : helpcode.optString("schema", "ziranma");
        boolean helpcodeOn = helpcode == null || helpcode.optBoolean("enabled", true);
        GroupCard aux = GroupCard.add(sectionColumn(target, "辅助码"), "辅助码");
        aux.toggle("启用辅助码", null,
            helpcodeOn, checked -> saveHelpcode(family, shuangpin, "enabled", checked)).help((shuangpin ? "双拼" : "全拼") + "组字时，先按 Shift 再输入的字母用来缩小候选");
        if (helpcodeOn) {
            JSONObject plugins = preferences.optJSONObject("plugins");
            String selectedPack = plugins == null ? "" : plugins.optString(HelpcodeChoicePolicy.packKey(shuangpin), "");
            aux.nav("辅助码方案", shuangpin ? "双拼" : "全拼",
                HelpcodeChoicePolicy.rowValue(state.helpcodePacks(), selectedPack, labelOf(state.helpcodeSchemas(), schema)),
                () -> pickHelpcode(family, shuangpin, state, schema, selectedPack));
            // 共享设置页的同一个开关（#6997）：只关显示，Shift 筛选照常可用。缺整个对象时按各自默认（双拼显示、全拼不显示），对象里缺这一项时按 serde 的默认显示。
            boolean shownInCandidates = helpcode == null ? shuangpin : helpcode.optBoolean("show_in_candidate_window", true);
            aux.toggle(shuangpin ? "在候选栏中显示双拼辅助码" : "在候选栏中显示全拼辅助码", null, shownInCandidates,
                checked -> saveHelpcode(family, shuangpin, "show_in_candidate_window", checked)).help("关掉后候选旁不再标注辅助码，Shift 筛选照常可用");
            boolean strictFilter = helpcode != null && helpcode.optBoolean("strict_filter", false);
            aux.toggle(shuangpin ? "双拼一码严格筛选" : "全拼一码严格筛选", null,
                strictFilter, checked -> saveHelpcode(family, shuangpin, "strict_filter", checked)).help("关闭时一码只把匹配项前移；打开后排除辅助码不匹配的候选，两码始终严格筛选");
        }

        GroupCard translation = GroupCard.add(sectionColumn(target, "翻译"), "翻译");
        InputFeatureToggle translations = InputFeatureToggle.CANDIDATE_TRANSLATIONS;
        boolean translationsOn = preferences.optBoolean(translations.key(), translations.enabledByDefault());
        translation.toggle("候选词翻译", null,
            translationsOn, checked -> savePreferences(values -> values.put(translations.key(), checked))).help("为候选词附上译文；联网翻译用哪个服务在下面的「翻译服务」里选");
        // 「翻译范围」紧跟在「候选词翻译」下面，与共享设置页相同；候选词翻译关着时置灰，存的值不动。
        CandidateTranslationPolicy.Scope scope =
            CandidateTranslationPolicy.Scope.from(preferences.opt(CandidateTranslationPolicy.Scope.KEY));
        translation.nav(CandidateTranslationPolicy.Scope.TITLE, null,
            scope.title(), translationsOn ? () -> pickTranslationScope(scope) : null).help(CandidateTranslationPolicy.Scope.DESCRIPTION);
        translation.nav("翻译服务", null, null,
            () -> SettingsNavigator.open(requireContext(), PageId.TRANSLATION_SERVICE, null)).help("离线释义查不到的词用它补上译文");
        InputFeatureToggle gloss = InputFeatureToggle.CANDIDATE_ENGLISH_GLOSS;
        boolean glossOn = preferences.optBoolean(gloss.key(), gloss.enabledByDefault());
        String targetLanguage = InputViewValuePolicy.textOr(preferences, "translation_target_language", "en");
        translation.toggle("离线英文释义", null,
            glossOn, checked -> savePreferences(values -> values.put(gloss.key(), checked),
                () -> { if (checked) ensureGlosses(targetLanguage, state); })).help("给候选词标注离线释义；英语释义随应用自带，其他目标语言要下载离线释义词典");
        InputFeatureToggle pronunciation = InputFeatureToggle.CANDIDATE_PRONUNCIATION;
        boolean pronunciationOn = preferences.optBoolean(pronunciation.key(), pronunciation.enabledByDefault());
        java.io.File phonetics = new java.io.File(requireContext().getFilesDir(),
            "bootstrap/pronunciations/en-phonetic.db");
        boolean phoneticsReady = phonetics.isFile() || state.packs().contains(ResourcePacks.PRONUNCIATIONS);
        // 法、日、西、俄、德、韩语释义的读音在另一个词典里，目标语言是它们之一时才要它。
        boolean targetReadings = app.msime.android.CandidatePronunciationPolicy.LANGUAGES.contains(targetLanguage)
            && !"en".equals(targetLanguage);
        boolean targetReadingsReady = state.packs().contains(ResourcePacks.TARGET_PRONUNCIATIONS);
        // 词典没装好是当前状态，留在副标题；功能本身的说明收进「?」。
        String pronunciationMissing = phoneticsReady && (!targetReadings || targetReadingsReady)
            ? null : "离线读音词典未安装；打开后点下方「下载」，安装完成再进入输入框";
        translation.toggle(pronunciation.title(), pronunciationMissing, pronunciationOn,
            checked -> savePreferences(values -> values.put(pronunciation.key(), checked)))
            .help(pronunciation.description());
        if (pronunciationOn && !phoneticsReady)
            packRow(translation, state, "离线英文音标词典", "仅供本机候选显示英文音标",
                ResourcePacks.PRONUNCIATIONS);
        if (pronunciationOn && targetReadings && !targetReadingsReady)
            packRow(translation, state, "离线外语读音词典", "仅供本机候选显示法、日、西、俄、德、韩语释义的读音",
                ResourcePacks.TARGET_PRONUNCIATIONS);
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

    /** 一行资源包状态：没下载时「下载」，下载中「取消」，校验中按钮置灰，失败「重试」。副标题只放状态（需下载多大、进度），这个包是做什么的收进「?」。 */
    private void packRow(GroupCard group, State state, String title, String description, String pack) {
        ResourcePackService.Status status = ResourcePackService.status(pack);
        String subtitle = packState(state, pack);
        if (status != null && status.phase() == ResourcePackService.Phase.VERIFYING) {
            group.button(title, subtitle, "校验中", () -> {}).help(description).setEnabled(false);
            return;
        }
        boolean running = status != null && status.running();
        group.button(title, subtitle, running ? "取消" : status == null ? "下载" : "重试", () -> {
            if (running) ResourcePackService.cancel(pack);
            else ResourcePackService.request(requireContext(), pack, packSize(state, pack));
        }).help(description);
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
        KeyboardScheme fromScheme = SchemePreferences.storedScheme(preferences, edition);
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
            case QUANPIN, QUANPIN_FOURTEEN_KEY, QUANPIN_EIGHTEEN_KEY, QUANPIN_NINE_KEY -> scheme.title();
            case CANTONESE -> "粤拼";
            case CANTONESE_SHUANGPIN -> "粤语双拼";
            case WUBI -> scheme.title(InputViewValuePolicy.textOr(preferences, "wubi_profile", KeyboardScheme.WUBI_86));
            case JAPANESE -> "26 键";
            case JAPANESE_NINE_KEY -> "9 键";
            default -> scheme.title();
        };
    }

    private void showLanguage(Language language, List<KeyboardScheme> offered, KeyboardScheme applied,
            JSONObject preferences, State state) {
        Context context = requireContext();
        OptionSheet sheet = new OptionSheet(context, language.title, offered.size() > 1 ? "选择输入方案" : null);
        // 每行的方案直接列在这一层：双拼列小鹤、自然码、微软、首道和九键双拼，五笔列 86 和 98。再套一层子菜单，「双拼」行、方案面板、双拼面板就成了三层（双拼布局原先还在第四层）。双拼的 26／14／18 键布局在键盘页「中文键盘」里选，九键双拼的表在「九键双拼方案与自定义」页里选，与 iOS、鸿蒙相同。
        if (language == Language.SHUANGPIN) {
            for (KeyboardScheme scheme : offered)
                sheet.option(SHUANGPIN.contains(scheme) ? shuangpinName(scheme) : scheme.title(), scheme == applied,
                    () -> applyScheme(scheme, null));
        } else if (language == Language.WUBI) {
            if (offered.contains(KeyboardScheme.WUBI)) {
                String profile = KeyboardScheme.normalizedWubiProfile(
                    InputViewValuePolicy.textOr(preferences, "wubi_profile", KeyboardScheme.WUBI_86));
                boolean wubi = applied == KeyboardScheme.WUBI;
                for (String option : List.of(KeyboardScheme.WUBI_86, KeyboardScheme.WUBI_98))
                    sheet.option(wubiName(option), wubi && option.equals(profile),
                        () -> applyScheme(KeyboardScheme.WUBI, option));
            }
        } else {
            KeyboardScheme shown = language.schemes.contains(applied) ? applied : null;
            for (KeyboardScheme scheme : offered) {
                sheet.option(schemeLabel(scheme, preferences), scheme == shown || (shown == null && offered.size() == 1),
                    () -> applyScheme(scheme, null));
            }
        }
        sheet.destructive("移除" + language.title, () -> removeLanguage(language));
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
        savePreferences(preferences -> {
            List<String> ids = effectiveIds(preferences, edition);
            if (!ids.contains(scheme.preferenceId())) ids.add(scheme.preferenceId());
            KeyboardSheets.child(preferences, "touch_keyboard_schemes").put("enabled", new JSONArray(ids));
        }, () -> {
            if (pack != null) ResourcePackService.request(requireContext(), pack, size);
        });
    }

    /** 服务配置写入共享偏好，所有校验由 Rust 执行；每个输入框确认后原子保存。 */
    private void cloudProviderSettings(LinearLayout target, JSONObject preferences) {
        JSONObject provider = preferences.optJSONObject("cloud_candidate_provider");
        if (provider == null) return;
        GroupCard group = GroupCard.add(sectionColumn(target, "中文"), "云候选服务")
            .withDividers(Ui.ROW_PADDING_H);
        String source = provider.optString("source", "google");
        group.nav("来源", null, cloudSourceLabel(source), () -> {
            OptionSheet sheet = new OptionSheet(requireContext(), "云候选来源", null);
            sheet.option("Google 输入工具", "google".equals(source),
                () -> savePreferences(values -> KeyboardSheets.child(values, "cloud_candidate_provider").put("source", "google")));
            sheet.option("自定义 HTTP 接口", "custom".equals(source), () -> {
                if (provider.optString("endpoint").isEmpty()) editCloudField(provider, "endpoint", "接口地址", true);
                else savePreferences(values -> KeyboardSheets.child(values, "cloud_candidate_provider").put("source", "custom"));
            });
            sheet.show();
        });
        if (!"custom".equals(source)) {
            group.nav("自定义接口设置", null, editingCloudCustom ? "收起" : null, () -> {
                editingCloudCustom = !editingCloudCustom;
                if (lastState != null) render(lastState);
            });
        }
        if ("custom".equals(source) || editingCloudCustom) {
            group.nav("接口地址", null, cloudEndpointLabel(provider),
                () -> editCloudField(provider, "endpoint", "接口地址", false));
            group.nav("请求方法", null, provider.optString("method", "GET"), () -> {
                OptionSheet sheet = new OptionSheet(requireContext(), "请求方法", null);
                for (String method : List.of("GET", "POST")) sheet.option(method, method.equals(provider.optString("method")),
                    () -> savePreferences(values -> KeyboardSheets.child(values, "cloud_candidate_provider").put("method", method)));
                sheet.show();
            });
            group.nav("请求头", null, cloudConfiguredLabel(provider.optString("headers_json")),
                () -> editCloudField(provider, "headers_json", "请求头（JSON）", false));
            if ("POST".equals(provider.optString("method"))) {
                group.nav("请求体模板", null, cloudConfiguredLabel(provider.optString("body_template")),
                    () -> editCloudField(provider, "body_template", "请求体模板（JSON）", false));
            }
            group.nav("响应格式", null, cloudResponseFormatLabel(provider.optString("response_format")), () -> {
                OptionSheet sheet = new OptionSheet(requireContext(), "响应格式", null);
                String[][] formats = {{"candidates", "标准候选数组"}, {"google", "Google 格式"}, {"json_path", "JSON 路径"}};
                for (String[] format : formats) sheet.option(format[1], format[0].equals(provider.optString("response_format")),
                    () -> savePreferences(values -> KeyboardSheets.child(values, "cloud_candidate_provider").put("response_format", format[0])));
                sheet.show();
            });
            if ("json_path".equals(provider.optString("response_format"))) {
                group.nav("JSON 路径", null, provider.optString("response_path", "data.0"),
                    () -> editCloudField(provider, "response_path", "JSON 路径", false));
            }
        }
        group.nav("超时", null, provider.optInt("timeout_ms", 2000) + " 毫秒",
            () -> editCloudField(provider, "timeout_ms", "超时（毫秒）", false));
        group.toggle("缓存云候选", null, provider.optBoolean("cache_enabled"),
            enabled -> savePreferences(values -> KeyboardSheets.child(values, "cloud_candidate_provider").put("cache_enabled", enabled))).help("仅保存在本机内存，无结果缓存 60 秒");
        group.help("当前拼写发送至 " + cloudRecipient(provider) + "；不发送已上屏内容、词库或历史。");
    }

    private void editCloudField(JSONObject provider, String key, String title, boolean selectCustom) {
        InputDialog dialog = new InputDialog(requireContext(), title, cloudFieldHelp(key));
        dialog.addField(title, "timeout_ms".equals(key) ? String.valueOf(provider.optInt(key)) : provider.optString(key),
            "timeout_ms".equals(key) ? android.text.InputType.TYPE_CLASS_NUMBER : 0);
        dialog.setValidator(values -> !"timeout_ms".equals(key) || values.get(0).matches("[0-9]{1,5}"));
        dialog.setPrimary("保存", values -> savePreferences(preferences -> {
            JSONObject config = KeyboardSheets.child(preferences, "cloud_candidate_provider");
            config.put(key, "timeout_ms".equals(key) ? Integer.valueOf(values.get(0)) : values.get(0));
            if (selectCustom) config.put("source", "custom");
        }));
        dialog.show();
    }

    private static String cloudSourceLabel(String source) {
        return "custom".equals(source) ? "自定义接口" : "Google";
    }

    private static String cloudRecipient(JSONObject provider) {
        return "custom".equals(provider.optString("source")) ? cloudEndpointLabel(provider) : "inputtools.google.com";
    }

    private static String cloudEndpointLabel(JSONObject provider) {
        String host = Uri.parse(provider.optString("endpoint").replace("{query}", "query")).getHost();
        return host == null ? "未设置" : host;
    }

    private static String cloudConfiguredLabel(String value) {
        return value == null || value.isBlank() || "{}".equals(value) ? "未设置" : "已设置";
    }

    private static String cloudResponseFormatLabel(String format) {
        return switch (format) {
            case "google" -> "Google 格式";
            case "json_path" -> "JSON 路径";
            default -> "标准数组";
        };
    }

    private static String cloudFieldHelp(String key) {
        return switch (key) {
            case "endpoint" -> "支持 HTTPS 或本机 HTTP。GET 地址用 {query} 填入当前拼写。地址仅保存在本机。";
            case "headers_json" -> "JSON 对象，例如 {\"Authorization\":\"Bearer …\"}。请求头可能含凭据，仅保存在本机。";
            case "body_template" -> "POST 请求使用 JSON 对象；字符串值中的 {query} 自动替换为当前拼写。";
            case "response_path" -> "点分对象键或数组下标，最终指向候选文本，例如 data.0 或 choices.0.message.content。";
            case "timeout_ms" -> "等待云候选的最长时间，可填写 100–10000 毫秒。";
            default -> "仅保存在本机";
        };
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
                // 词典还没到的方案（例如瘦包里码表没下载的五笔）不在可用列表里，`current` 已经解析成了别的方案，但偏好里存的 `scheme` 仍是它：也要切走，否则移除之后偏好还指着一个不在列表里的方案。
                if (language.schemes.contains(current)
                        || language.schemes.contains(SchemePreferences.storedScheme(preferences, edition))) {
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
        sheet.option("简体", !traditional, () -> savePreferences(values -> values.put(key, false)));
        sheet.option("繁体", traditional, () -> savePreferences(values -> values.put(key, true)));
        sheet.show();
    }

    private void saveAutocorrect(boolean checked) {
        savePreferences(values -> {
            JSONObject quanpin = KeyboardSheets.child(values, "quanpin");
            quanpin.put("autocorrect_transposition", checked);
            quanpin.put("autocorrect_neighbor", checked);
        });
    }

    /** 方案列表：内置方案，已导入的码表，「导入辅助码表…」，有导入的码表时再加「管理已导入的码表」。 */
    private void pickHelpcode(String family, boolean shuangpin, State state, String schema, String selectedPack) {
        Context context = requireContext();
        String selected = HelpcodeChoicePolicy.selectedValue(schema, selectedPack);
        OptionSheet sheet = new OptionSheet(context, "辅助码方案", shuangpin ? "双拼" : "全拼");
        for (String[] entry : state.helpcodeSchemas()) {
            String id = entry[0];
            sheet.option(entry[1], id.equals(selected), () -> chooseHelpcode(family, shuangpin, id, selectedPack));
        }
        for (HelpcodeChoicePolicy.Option option : HelpcodeChoicePolicy.packOptions(state.helpcodePacks(), selectedPack)) {
            sheet.option(option.label(), option.value().equals(selected),
                () -> chooseHelpcode(family, shuangpin, option.value(), selectedPack));
        }
        sheet.option("导入辅助码表…", false, () -> {
            importShuangpin = shuangpin;
            // SAF 给 .txt 报的 MIME 不一定是 text/plain，所以一并放行 octet-stream，内容由 client-services 校验。
            openTable.launch(new String[] {"text/plain", "application/octet-stream"});
        });
        List<HelpcodeChoicePolicy.Pack> packs = state.helpcodePacks();
        if (!packs.isEmpty()) {
            sheet.submenu("管理已导入的码表", false, () -> {
                OptionSheet next = new OptionSheet(context, "已导入的码表", "删除后，正在用它的方案改回内置方案");
                for (HelpcodeChoicePolicy.Pack pack : packs) next.destructive("删除「" + pack.name() + "」", () -> removeHelpcodePack(pack));
                return next;
            });
        }
        sheet.show();
    }

    /** 在方案列表里选了 `value`（内置方案 id 或 `pack:<id>`），规则见 {@link HelpcodeChoicePolicy#choose}；只改 `plugins` 里这一个键，按键音包等其他键保留。 */
    private void chooseHelpcode(String family, boolean shuangpin, String value, String selectedPack) {
        HelpcodeChoicePolicy.Change change = HelpcodeChoicePolicy.choose(value, selectedPack);
        savePreferences(values -> {
            if (change.schema() != null) helpcodeObject(values, family, shuangpin).put("schema", change.schema());
            if (change.pack() != null) {
                KeyboardSheets.child(values, "plugins").put(HelpcodeChoicePolicy.packKey(shuangpin), change.pack());
            }
        });
    }

    private void onTablePicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        boolean shuangpin = importShuangpin;
        HostTask.run(this, context -> importTable(context, uri, shuangpin), message -> {
            MsToast.show(requireContext(), message == null ? HelpcodeChoicePolicy.importFailure("", null) : message);
            reload();
        });
    }

    /** 读文件、严格解码、导入，成功后选中到发起导入的那一份方案；返回给用户看的提示。要在工作线程上调用。 */
    private static String importTable(Context context, Uri uri, boolean shuangpin) {
        String fileName = LexiconPage.displayName(context, uri);
        byte[] bytes;
        try (InputStream input = context.getContentResolver().openInputStream(uri)) {
            if (input == null) return "读取文件失败，请重新选择。";
            bytes = HttpBodyPolicy.readBounded(input, HelpcodeChoicePolicy.MAX_TABLE_BYTES);
        } catch (IOException | SecurityException error) {
            return "读取文件失败，请重新选择。";
        }
        HelpcodeChoicePolicy.Decoded decoded = HelpcodeChoicePolicy.decode(bytes);
        if (decoded.text() == null) return decoded.failure();
        JSONObject value;
        try {
            JSONObject response = plugins(context, new JSONObject()
                .put("operation", "import_helpcode_table")
                .put("file_name", fileName)
                .put("text", decoded.text()));
            if (!JsonPolicy.strictTrue(response.opt("ok"))) {
                String detail = response.optString("detail", "");
                return HelpcodeChoicePolicy.importFailure(response.optString("error", ""), detail.isEmpty() ? null : detail);
            }
            value = response.optJSONObject("value");
        } catch (JSONException error) {
            return null;
        }
        String id = value == null ? "" : value.optString("id", "");
        if (id.isEmpty()) return null;
        String name = value.optString("name", id);
        String key = HelpcodeChoicePolicy.packKey(shuangpin);
        JSONObject saved = KeyboardSheets.write(context, values -> KeyboardSheets.child(values, "plugins").put(key, id));
        return HelpcodeChoicePolicy.importedMessage(name, value.optLong("entries", 0), saved != null);
    }

    /** 删除一个导入的码表；全拼或双拼正选着它时把那一份选择清成空字符串，退回内置方案。 */
    private void removeHelpcodePack(HelpcodeChoicePolicy.Pack pack) {
        HostTask.run(this, context -> {
            try {
                JSONObject response = plugins(context, new JSONObject()
                    .put("operation", "remove").put("kind", "helpcode").put("id", pack.id()));
                if (!JsonPolicy.strictTrue(response.opt("ok"))) return null;
            } catch (JSONException error) {
                return null;
            }
            boolean[] cleared = {false};
            JSONObject saved = KeyboardSheets.write(context, values -> {
                JSONObject plugins = values.optJSONObject("plugins");
                if (plugins == null) return;
                for (String key : HelpcodeChoicePolicy.PACK_KEYS) {
                    if (HelpcodeChoicePolicy.clearsSelection(pack.id(), plugins.optString(key, ""))) {
                        plugins.put(key, "");
                        cleared[0] = true;
                    }
                }
            });
            // 清选择没写成时，选择仍指着已删的包：键盘照样退回内置方案（host-api 载不入包就用 `schema`），本页把它显示成「未找到」，用户可以再选一次。
            return HelpcodeChoicePolicy.removedMessage(pack.name(), cleared[0] && saved != null);
        }, message -> {
            MsToast.show(requireContext(), message == null ? "删除失败，请重试" : message);
            reload();
        });
    }

    /** `*_helpcode` 拒绝未知字段且 `schema` 必填，缺这个对象时按各自的默认值补齐再交出去改。 */
    private static JSONObject helpcodeObject(JSONObject values, String family, boolean shuangpin) throws JSONException {
        JSONObject helpcode = values.optJSONObject(family);
        if (helpcode == null) {
            helpcode = new JSONObject()
                .put("enabled", true)
                .put("schema", shuangpin ? "lantian" : "ziranma")
                .put("show_in_candidate_window", shuangpin)
                .put("strict_filter", false);
            values.put(family, helpcode);
        }
        return helpcode;
    }

    private void saveHelpcode(String family, boolean shuangpin, String member, Object value) {
        savePreferences(values -> helpcodeObject(values, family, shuangpin).put(member, value));
    }

    private void pickTranslationScope(CandidateTranslationPolicy.Scope selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), CandidateTranslationPolicy.Scope.TITLE,
            CandidateTranslationPolicy.Scope.DESCRIPTION);
        for (CandidateTranslationPolicy.Scope scope : CandidateTranslationPolicy.Scope.values()) {
            sheet.option(scope.title(), scope == selected,
                () -> savePreferences(values -> values.put(CandidateTranslationPolicy.Scope.KEY, scope.id())));
        }
        sheet.show();
    }

    private void pickTargetLanguage(String selected, boolean glossOn, State state) {
        OptionSheet sheet = new OptionSheet(requireContext(), "翻译目标语言", null);
        for (String[] entry : TARGET_LANGUAGES) {
            String id = entry[0];
            sheet.option(entry[1], id.equals(selected), () -> savePreferences(values -> values.put("translation_target_language", id),
                () -> { if (glossOn) ensureGlosses(id, state); }));
        }
        sheet.show();
    }

    private static String labelOf(List<String[]> entries, String id) {
        Map<String, String> labels = new LinkedHashMap<>(entries.size());
        for (String[] entry : entries) labels.put(entry[0], entry[1]);
        return labels.getOrDefault(id, id);
    }

}
