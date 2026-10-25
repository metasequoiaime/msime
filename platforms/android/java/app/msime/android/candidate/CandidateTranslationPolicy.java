package app.msime.android;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Validates and presents the candidate gloss configuration of the touch keyboard. */
public final class CandidateTranslationPolicy {
    /** 触屏键盘只显示一种候选翻译（主目标语言）：第二种语言让每个候选下多一行，候选栏太高。共享偏好里的第二种候选语言留给桌面端，这里不读。iOS、HarmonyOS 相同。 */
    public static final int MAX_TARGETS = 1;
    private static final Set<String> SUPPORTED = Set.of("en", "fr", "ja", "es", "ru", "de", "ko");
    /** Targets with an offline dictionary format; mirrors OFFLINE_GLOSS_LANGUAGES in crates/host-api. */
    public static final Set<String> OFFLINE_GLOSS_LANGUAGES = Set.of("fr", "ja", "es", "ru", "de", "ko");

    private CandidateTranslationPolicy() {}

    /** 唯一的目标语言就是主目标语言，不认识时退回英语。 */
    public static List<String> targets(String primary) {
        String first = TextPolicy.lowercaseTrimmed(primary);
        return List.of(SUPPORTED.contains(first) ? first : "en");
    }

    /** Keep the two language rows readable in a single Android candidate annotation. */
    public static String joinGlosses(List<String> glosses) {
        if (glosses == null || glosses.isEmpty()) return "";
        return String.join("\n", glosses);
    }

    /** 长按候选最多给出的可上屏义项数，与鸿蒙 {@code TranslationSensePolicy.MAX_SENSES} 相同。 */
    public static final int MAX_INSERTION_SENSES = 9;

    /**
     * 长按候选可以上屏的义项：先按换行切成每种目标语言一行，再把每行按 {@code ;} 或全角 {@code ；} 切成义项，去掉首尾空白、空项和重复项，按目标语言顺序、行内义项顺序排列。
     *
     * <p>分号规则对应 {@code shared/input/GlossSenses.h} 的 {@code gloss_senses}（macOS、Linux、Windows 共用）。换行处有意与它不同：{@code gloss_senses} 把换行当义项内的折行，本宿主的换行是 {@link #joinGlosses} 写进去的语言分隔，所以先按换行切行。账号译文同样会被切开，一句本身含分号的译文会成为几项，桌面和鸿蒙也是这样。
     */
    public static List<String> insertionGlosses(String translation) {
        if (translation == null || translation.isEmpty()) return List.of();
        ArrayList<String> result = new ArrayList<>(MAX_INSERTION_SENSES);
        String[] rows = translation.split("\\R", -1);
        for (int row = 0; row < rows.length && result.size() < MAX_INSERTION_SENSES; row++) {
            for (String value : rows[row].split("[;；]", -1)) {
                if (result.size() >= MAX_INSERTION_SENSES) break;
                String gloss = TextPolicy.trimmed(value);
                if (!insertableGloss(gloss, result)) continue;
                // 只有后面的语言行确实有新的可上屏义项，才为它留一个菜单位置。
                if (result.size() == MAX_INSERTION_SENSES - 1
                        && hasLaterInsertionSense(rows, row, result)) break;
                result.add(gloss);
            }
        }
        return List.copyOf(result);
    }

    private static boolean hasLaterInsertionSense(String[] rows, int row, List<String> selected) {
        for (int next = row + 1; next < rows.length; next++) {
            for (String value : rows[next].split("[;；]", -1)) {
                if (insertableGloss(TextPolicy.trimmed(value), selected)) return true;
            }
        }
        return false;
    }

    private static boolean insertableGloss(String gloss, List<String> selected) {
        return TextPolicy.boundedNonBlank(gloss, 4096) && !selected.contains(gloss);
    }

    /**
     * 候选词这一次发给哪个在线服务，逐条照共享层 `crates/host-api/src/ffi/providers.rs` 的 `selected_translation_services`：
     *
     * <ul>
     * <li>候选翻译关闭时什么都不发。共享层在这种情况下只排除水杉账号，用户自己的服务由宿主按这个开关拦下（Linux 的 provider 也是这样）。</li>
     * <li>水杉账号（api.msime.app）只在用户明确选了它、并且小牛和自定义都没启用、腾讯也不是「启用且两项凭据都可用」时才用，从不作为回落。</li>
     * <li>其余按 小牛 > 自定义 > 腾讯 选。选中的服务不可用时结果是 {@link CandidateTranslationRoute#NONE}，不改用别的服务：小牛要两项凭据都可用；自定义只要求接口地址去掉首尾空白后不为空，API Key 可以为空（DeepLX 允许不带密钥），DeepL 地址缺密钥这类情况由共享描述器在请求时拒绝；腾讯的 `enabled` 默认就是开着的，所以只有两项凭据都可用才算选中。</li>
     * </ul>
     *
     * <p>参数都是基本类型，因为 JVM 冒烟加载不了 org.json。凭据去掉首尾的空格、制表符和换行，与共享层的 `trim_matches` 相同。
     */
    public static CandidateTranslationRoute route(boolean candidateTranslations, boolean translationAccount,
            boolean niutransEnabled, String niutransAppId, String niutransApiKey,
            boolean customEnabled, String customEndpoint, String customApiKey,
            boolean tencentEnabled, String tencentSecretId, String tencentSecretKey, String tencentRegion) {
        if (!candidateTranslations) return CandidateTranslationRoute.NONE;
        String secretId = credentialTrimmed(tencentSecretId);
        String secretKey = credentialTrimmed(tencentSecretKey);
        boolean tencentUsable = tencentEnabled && usableCredential(secretId) && usableCredential(secretKey);
        if (translationAccount && !niutransEnabled && !customEnabled && !tencentUsable)
            return CandidateTranslationRoute.ACCOUNT;
        if (niutransEnabled) {
            String appId = credentialTrimmed(niutransAppId);
            String apiKey = credentialTrimmed(niutransApiKey);
            return usableCredential(appId) && usableCredential(apiKey)
                ? CandidateTranslationRoute.niutrans(appId, apiKey) : CandidateTranslationRoute.NONE;
        }
        if (customEnabled) {
            String endpoint = credentialTrimmed(customEndpoint);
            return endpoint.isEmpty() ? CandidateTranslationRoute.NONE
                : CandidateTranslationRoute.custom(endpoint, credentialTrimmed(customApiKey));
        }
        if (tencentUsable)
            return CandidateTranslationRoute.tencent(secretId, secretKey, credentialTrimmed(tencentRegion));
        return CandidateTranslationRoute.NONE;
    }

    /** 在已有翻译服务都未选中时，才使用复用的 AI 助手配置；旧路由优先级保持不变。 */
    public static CandidateTranslationRoute route(boolean candidateTranslations, boolean translationAccount,
            boolean niutransEnabled, String niutransAppId, String niutransApiKey,
            boolean customEnabled, String customEndpoint, String customApiKey,
            boolean tencentEnabled, String tencentSecretId, String tencentSecretKey, String tencentRegion,
            boolean aiEnabled, String aiProvider, String aiEndpoint, String aiModel, String aiToken) {
        CandidateTranslationRoute existing = route(candidateTranslations, translationAccount,
            niutransEnabled, niutransAppId, niutransApiKey, customEnabled, customEndpoint, customApiKey,
            tencentEnabled, tencentSecretId, tencentSecretKey, tencentRegion);
        if (existing.provider() != CandidateTranslationRoute.Provider.NONE || !aiEnabled)
            return existing;
        String provider = credentialTrimmed(aiProvider);
        String endpoint = credentialTrimmed(aiEndpoint);
        String model = credentialTrimmed(aiModel);
        String token = credentialTrimmed(aiToken);
        return !provider.isEmpty() && !endpoint.isEmpty() && !model.isEmpty()
                && usableCredential(token) && AiEndpointPolicy.allowed(endpoint)
            ? CandidateTranslationRoute.ai(provider, endpoint, model, token)
            : CandidateTranslationRoute.NONE;
    }

    /** 与共享层的 source_gets_no_gloss 保持一致：这些来源既不请求也不显示候选释义。 */
    public static boolean sourceGetsNoGloss(int source) {
        return source == 6 || source == 7 || source == 13 || source == 14 || source == 15 || source == 16 || source == 18;
    }

    /** 与共享层的 source_gets_no_gloss 保持一致：表情、颜文字、日文混输、插件、占位及联系人候选不送用户翻译服务。 */
    public static boolean userTranslationSourceAllowed(int source) {
        return !sourceGetsNoGloss(source);
    }

    /** 与共享层 `translation::usable_credential` 相同：去掉首尾空白后不为空，不是 `<...>` 这样的占位符，也不以 `FAKESECRET_` 开头。 */
    public static boolean usableCredential(String value) {
        String trimmed = credentialTrimmed(value);
        return !trimmed.isEmpty() && !(trimmed.startsWith("<") && trimmed.endsWith(">"))
            && !trimmed.startsWith("FAKESECRET_");
    }

    /** 去掉首尾的空格、制表符、回车和换行，与共享层的 `trim_matches([' ', '\t', '\r', '\n'])` 相同。 */
    static String credentialTrimmed(String value) {
        if (value == null) return "";
        int start = 0;
        int end = value.length();
        while (start < end && trimmable(value.charAt(start))) start++;
        while (end > start && trimmable(value.charAt(end - 1))) end--;
        return value.substring(start, end);
    }

    private static boolean trimmable(char value) {
        return value == ' ' || value == '\t' || value == '\r' || value == '\n';
    }

    /** A delayed account translation may be applied only while the same binding snapshot is current. */
    public static boolean acceptsBinding(String requested, String current) {
        return requested != null && !requested.isEmpty() && requested.equals(current);
    }

    /** 偏好变化后当前候选行中的译文必须先清空，再等待新配置的回答。换了翻译服务或凭据（路由变了）也算。 */
    public static boolean displayInvalidated(boolean previousEnglishGloss,
            boolean nextEnglishGloss, boolean previousTranslations,
            boolean nextTranslations, boolean previousAccount, boolean nextAccount,
            List<String> previousTargets, List<String> nextTargets,
            CandidateTranslationRoute previousRoute, CandidateTranslationRoute nextRoute) {
        return previousEnglishGloss != nextEnglishGloss
            || previousTranslations != nextTranslations
            || previousAccount != nextAccount
            || !java.util.Objects.equals(previousTargets, nextTargets)
            || !java.util.Objects.equals(previousRoute, nextRoute);
    }

    /** Count rows that can actually be filled by the enabled offline/online paths. The offline switch covers English and every target in {@code offlineTargets}. */
    public static int glossLines(List<String> targets, boolean offline, boolean online,
            Collection<String> offlineTargets) {
        if (targets == null || targets.isEmpty()) return 0;
        int lines = 0;
        for (String target : targets) {
            String code = TextPolicy.lowercaseTrimmed(target);
            if (online || (offline && ("en".equals(code)
                    || (offlineTargets != null && offlineTargets.contains(code))))) lines++;
        }
        return lines;
    }

    /** The non-English targets whose dictionary is installed in the {@code offline-glosses} directory beside {@code resources}, in target order. */
    public static List<String> offlineTargets(List<String> targets, String resources) {
        return offlineTargets(targets, resources, null);
    }

    /**
     * 有离线释义词典的非英文目标语言，按目标顺序：先看 {@code resources} 旁随包的 {@code offline-glosses} 目录，没有时再看 {@code stateRoot}（HostOptions 的 {@code preferences_directory}）下已下载的 {@code resource-packs/offline-glosses} 资源包，与 host-api 查找释义词典的顺序一致。{@code stateRoot} 为 null 或空时只看随包的那份。
     */
    public static List<String> offlineTargets(List<String> targets, String resources, String stateRoot) {
        if (targets == null || resources == null || resources.isEmpty()) return List.of();
        File parent = new File(resources).getParentFile();
        if (parent == null) return List.of();
        File pack = stateRoot == null || stateRoot.isEmpty() || !new File(stateRoot).isAbsolute()
            ? null : new File(stateRoot, "resource-packs/offline-glosses");
        ArrayList<String> result = new ArrayList<>(MAX_TARGETS);
        for (String target : targets) {
            String code = TextPolicy.lowercaseTrimmed(target);
            if (!OFFLINE_GLOSS_LANGUAGES.contains(code)) continue;
            String name = "zh-" + code + ".db";
            if (Files.isRegularFile(new File(parent, "offline-glosses/" + name).toPath(), LinkOption.NOFOLLOW_LINKS)
                    || (pack != null && Files.isRegularFile(new File(pack, name).toPath(), LinkOption.NOFOLLOW_LINKS)))
                result.add(code);
        }
        return List.copyOf(result);
    }

    /** 一个候选按目标语言顺序排列的各行：某个目标有离线释义就用离线释义，没有时才用在线译文（水杉账号或用户自己的服务）。鸿蒙相反，在线优先、离线补缺。 */
    public static String mergeGlosses(List<String> targets, Map<String, String> offline,
            Map<String, String> online) {
        if (targets == null) return "";
        ArrayList<String> glosses = new ArrayList<>(MAX_TARGETS);
        for (String target : targets) {
            String gloss = offline == null ? null : offline.get(target);
            if (gloss == null || gloss.isEmpty()) gloss = online == null ? null : online.get(target);
            if (gloss != null && !gloss.isEmpty()) glosses.add(gloss);
        }
        return joinGlosses(glosses);
    }

    /** Number of rows needed by one rendered candidate label, based on actual annotation text. */
    public static int renderedGlossLines(String annotation) {
        return annotation != null && annotation.indexOf('\n') >= 0 ? 2 : 1;
    }

    /** Rows of a candidate label whose secondary text starts on its own row under the candidate, as a Korean Hanja row does: the candidate, then one row per annotation line. */
    public static int renderedOwnRowLines(String annotation) {
        if (annotation == null || annotation.isEmpty()) return 1;
        int lines = 2;
        for (int index = annotation.indexOf('\n'); index >= 0; index = annotation.indexOf('\n', index + 1))
            lines++;
        return lines;
    }

    /**
     * 候选条在基础高度之上为 {@code glossLines} 行释义预留的行数。
     *
     * <p>每行释义都在候选下面另起一行，所以有几行释义就预留几行。韩语汉字行的 훈음 和第一行释义同在一行（{@link CandidateGlossPolicy#hanjaAnnotation}），所以只保证至少一行，不再多占一行：多占时韩语的候选条和空闲工具栏都比其他方案高，切换布局键盘高度就跳。不论汉字列表是否展开都一样，列表展开时候选条不会变高。
     */
    public static int reservedGlossRows(int glossLines, boolean hanjaRows) {
        int lines = BoundsPolicy.nonNegative(glossLines);
        return hanjaRows ? BoundsPolicy.bounded(lines, 1, Integer.MAX_VALUE) : lines;
    }
}
