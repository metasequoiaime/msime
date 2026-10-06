package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.InputFeatureToggle;
import app.msime.android.KeyboardScheme;
import app.msime.android.NativeClient;
import app.msime.android.SchemePreferences;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 输入页：语言与方案、中文（字符集、拼音纠错、模糊音、云候选）、辅助码方案和翻译（候选词翻译、离线英文释义、目标语言）。
 *
 * <p>语言列表是键盘自己的方案列表 `touch_keyboard_schemes.enabled` 按语言分组后的样子：普通话一直在，其他语言在列表里有它的方案时出现。在语言的面板里选一个方案，经 {@link SchemePreferences#withScheme} 同时写 `scheme` 那几个键和 `touch_keyboard_schemes.selected`（并在缺失时加进 `enabled`），否则键盘会按旧的 `selected` 解析方案；「移除」把这门语言的方案全部移出 `enabled`，键盘正用着它时先切回剩下的第一个方案；「添加语言」把这门语言的第一个方案加进 `enabled`，不切换。粤语只有粤拼（P18：仓颉、速成没有方案和码表）；英语没有自己的触屏方案（中英切换键一直在），所以不出现在列表里。
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

    private record State(JSONObject preferences, String languageDictionaries, List<String[]> helpcodeSchemas) {}

    @Nullable private LinearLayout column;
    private boolean adding;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        if (args.getBoolean(ARG_ADD_LANGUAGE, false)) adding = true;
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
        HostTask.run(this, TypingPage::read, this::render);
    }

    @Nullable private static State read(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        return new State(preferences, HostStore.languageDictionaries(context), helpcodeSchemas());
    }

    /** host capabilities 的 `helpcode_schemas`（字符串 id，或 `{id|schema, name|title}`）；没有或读不懂时是内置列表。 */
    private static List<String[]> helpcodeSchemas() {
        List<String[]> schemas = new ArrayList<>();
        try {
            JSONObject root = new JSONObject(NativeClient.hostCapabilities("android"));
            JSONObject value = root.optBoolean("ok", false) ? root.optJSONObject("value") : null;
            JSONArray listed = value == null ? null : value.optJSONArray("helpcode_schemas");
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
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();
        JSONObject preferences = state.preferences();
        AppEdition edition = AppEdition.current();
        KeyboardScheme applied = applied(preferences, edition, state.languageDictionaries());
        List<KeyboardScheme> enabled = enabled(preferences, edition);

        GroupCard languages = GroupCard.add(target, "语言与方案").withDividers(58);
        List<Language> addable = new ArrayList<>();
        for (Language language : Language.values()) {
            List<KeyboardScheme> offered = offered(language, edition, state.languageDictionaries());
            if (offered.isEmpty()) continue;
            KeyboardScheme shown = shownScheme(language, applied, enabled);
            if (language != Language.MANDARIN && shown == null) {
                addable.add(language);
                continue;
            }
            languages.addView(KeyboardSheets.badgeNavRow(context, language.badge, language.title, null,
                shown == null ? null : schemeLabel(shown, preferences),
                () -> showLanguage(language, offered, applied, preferences)));
        }
        if (!addable.isEmpty()) {
            languages.addView(KeyboardSheets.actionRow(context, adding ? "✓" : "＋", adding ? "完成" : "添加语言", () -> {
                adding = !adding;
                reload();
            }));
            if (adding) {
                for (Language language : addable) {
                    KeyboardScheme first = offered(language, edition, state.languageDictionaries()).get(0);
                    languages.addView(KeyboardSheets.pillRow(context, language.badge, language.title, first.title(),
                        "添加", () -> addLanguage(first)));
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
        translation.toggle("离线英文释义", gloss.description(),
            preferences.optBoolean(gloss.key(), gloss.enabledByDefault()),
            checked -> save(values -> values.put(gloss.key(), checked)));
        String targetLanguage = preferences.optString("translation_target_language", "en");
        translation.nav("翻译目标语言", null, labelOf(List.of(TARGET_LANGUAGES), targetLanguage),
            () -> pickTargetLanguage(targetLanguage));
    }

    // ---- 语言与方案 ----

    /**
     * 键盘实际在用的方案，和输入法按同一规则解析（MSIMEInputService 的 SchemeConfiguration）：在所有词典已安装的方案里找，而不是只在 `enabled` 里找。没存过 `enabled` 列表时默认列表不含注音、粤拼、笔画这类要手动开启的方案，只在里面找会把选中的 注音 9 键 显示成全拼，键盘却在打注音。
     */
    private static KeyboardScheme applied(JSONObject preferences, AppEdition edition, String dictionaries) {
        KeyboardScheme fromScheme = KeyboardScheme.fromPreferences(
            preferences.optString("scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);
        JSONObject schemes = preferences.optJSONObject("touch_keyboard_schemes");
        String selected = schemes == null || schemes.isNull("selected") ? null : schemes.optString("selected", null);
        List<KeyboardScheme> visible = KeyboardScheme.installedOf(List.of(KeyboardScheme.values()), dictionaries, edition);
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
        List<String> ids = new ArrayList<>();
        for (int index = 0; index < enabled.length(); index++) {
            if (!enabled.isNull(index)) ids.add(enabled.optString(index, ""));
        }
        return ids;
    }

    private static List<KeyboardScheme> offered(Language language, AppEdition edition, String dictionaries) {
        List<KeyboardScheme> offered = new ArrayList<>();
        for (KeyboardScheme scheme : language.schemes) {
            if (scheme.offeredBy(edition) && scheme.installed(dictionaries)) offered.add(scheme);
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
            JSONObject preferences) {
        Context context = requireContext();
        OptionSheet sheet = new OptionSheet(context, language.title, offered.size() > 1 ? "选择输入方案" : null);
        if (language == Language.MANDARIN) {
            boolean quanpin = applied == KeyboardScheme.QUANPIN || applied == KeyboardScheme.QUANPIN_NINE_KEY;
            KeyboardScheme quanpinChoice = applied == KeyboardScheme.QUANPIN_NINE_KEY
                ? KeyboardScheme.QUANPIN_NINE_KEY : KeyboardScheme.QUANPIN;
            if (offered.contains(quanpinChoice)) sheet.option("全拼", quanpin, () -> applyScheme(quanpinChoice, null));
            List<KeyboardScheme> shuangpin = new ArrayList<>();
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
            for (KeyboardScheme scheme : List.of(KeyboardScheme.ZHUYIN, KeyboardScheme.ZHUYIN_NINE_KEY,
                    KeyboardScheme.STROKE, KeyboardScheme.HANDWRITING)) {
                if (offered.contains(scheme))
                    sheet.option(mandarinName(scheme), scheme == applied, () -> applyScheme(scheme, null));
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

    /** 把这门语言的第一个方案加进 `enabled`，不切换键盘正在用的方案。 */
    private void addLanguage(KeyboardScheme scheme) {
        AppEdition edition = AppEdition.current();
        save(preferences -> {
            List<String> ids = effectiveIds(preferences, edition);
            if (!ids.contains(scheme.preferenceId())) ids.add(scheme.preferenceId());
            KeyboardSheets.child(preferences, "touch_keyboard_schemes").put("enabled", new JSONArray(ids));
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
                KeyboardScheme current = applied(preferences, edition, HostStore.languageDictionaries(context));
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
        List<String> ids = new ArrayList<>();
        for (KeyboardScheme scheme : KeyboardScheme.enabledFromPreferenceIds(null, edition)) ids.add(scheme.preferenceId());
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

    private void pickTargetLanguage(String selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "翻译目标语言", null);
        for (String[] entry : TARGET_LANGUAGES) {
            String id = entry[0];
            sheet.option(entry[1], id.equals(selected), () -> save(values -> values.put("translation_target_language", id)));
        }
        sheet.show();
    }

    private static String labelOf(List<String[]> entries, String id) {
        Map<String, String> labels = new LinkedHashMap<>();
        for (String[] entry : entries) labels.put(entry[0], entry[1]);
        return labels.getOrDefault(id, id);
    }

    private void save(KeyboardSheets.Edit edit) {
        KeyboardSheets.save(this, edit, this::reload, this::reload);
    }
}
