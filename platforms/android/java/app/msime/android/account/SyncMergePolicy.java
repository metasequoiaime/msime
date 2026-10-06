package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.ToLongFunction;

/**
 * 云同步的纯决策：每个分类这一轮该上传、下载、合并还是不动，常用语怎么合并，设置文档怎么叠加，皮肤库能占多少字节。
 *
 * <p>不碰网络、磁盘和 org.json，所以能在 check-host 的冒烟里直接跑；真正执行的是宿主进程的 home/CloudSync。
 */
public final class SyncMergePolicy {
    /** 整份偏好文档的上限留出余量：服务端 1 MiB，这里只用到 900 KiB。 */
    public static final int DOCUMENT_LIMIT_BYTES = 900 * 1024;
    /** `platform.android.custom_keyboard_skins` 字段在服务端字段表里的 maxLength。 */
    public static final int SKIN_FIELD_LIMIT = 786_432;
    public static final String SKINS_KEY = "platform.android.custom_keyboard_skins";
    /** 常用语接口的条数上限。 */
    public static final int MAX_PHRASES = 500;
    /** 常用语接口每条正文的 UTF-16 单元上限。 */
    public static final int MAX_PHRASE_UNITS = 2000;
    /** 设备本地且涉及隐私、无论如何都不上传的设置，按账号键名里的片段识别（R4 的导出已经排除，这里再拦一次）。 */
    static final List<String> LOCAL_ONLY_FRAGMENTS = List.of(
        "incognito", "developer_options", "diagnostic_log", "contribute_audio");

    /** 一个分类这一轮做什么。 */
    public enum Mode { NONE, UPLOAD, DOWNLOAD, MERGE }

    /** 首次开启同步时用户的选择；云端为空时不用问，直接上传。 */
    public enum Choice { MERGE, USE_CLOUD }

    /** 云端常用语接口里的一条：`group` 可为空串，`position` 从 0 数。 */
    public record Phrase(String id, String text, String group, int position) {}

    private SyncMergePolicy() {}

    /** 首次开启：云端没有任何数据就直接上传，否则要等用户选「合并」或「使用云端」，返回 null 表示需要问。 */
    public static Mode firstRun(boolean cloudHasData, Choice choice) {
        if (!cloudHasData) return Mode.UPLOAD;
        if (choice == null) return null;
        return choice == Choice.MERGE ? Mode.MERGE : Mode.DOWNLOAD;
    }

    /**
     * 之后每一轮：云端自上次同步以来有没有变（游标对不上），本机有没有待上传的改动。
     *
     * <p>两边都没变不动；只有本机变了上传（本机删掉的条目随之从云端消失）；只有云端变了下载；两边都变了合并，同一项以本机这次的写入为准。
     */
    public static Mode incremental(boolean cloudChanged, boolean localDirty) {
        if (cloudChanged && localDirty) return Mode.MERGE;
        if (cloudChanged) return Mode.DOWNLOAD;
        return localDirty ? Mode.UPLOAD : Mode.NONE;
    }

    /** 云端游标是不是和上次同步记下的不同；从没记过（空串）也算不同。 */
    public static boolean cloudChanged(String cursor, long cloudRevision) {
        return cursor == null || cursor.isEmpty() || !cursor.equals(Long.toString(cloudRevision));
    }

    /** PUT 回 409 `revision_conflict` 时的做法：重新拉云端，再按「本机为后写」合并后重试。超过这个次数就放到下一轮。 */
    public static final int MAX_CONFLICT_RETRIES = 2;

    /**
     * 合并两份常用语：先按 id，`preferred` 里的同 id 项胜出；再按正文去重，保留先出现的；最后重排 position 并截到 {@link #MAX_PHRASES} 条。正文为空或超长的项丢掉。
     *
     * @param preferred 后写的一方（本机这次的列表），排在前面
     * @param other 另一方（云端），只补上 `preferred` 里没有的
     */
    public static List<Phrase> mergePhrases(List<Phrase> preferred, List<Phrase> other) {
        LinkedHashMap<String, Phrase> byId = new LinkedHashMap<>();
        for (Phrase phrase : preferred) if (usable(phrase)) byId.putIfAbsent(phrase.id(), phrase);
        for (Phrase phrase : other) if (usable(phrase)) byId.putIfAbsent(phrase.id(), phrase);
        return normalized(new ArrayList<>(byId.values()));
    }

    /** 按正文去重、重排 position 并截断；上传前对任何一份列表都要过这一步。 */
    public static List<Phrase> normalized(List<Phrase> phrases) {
        Set<String> texts = new HashSet<>();
        List<Phrase> result = new ArrayList<>(Math.min(MAX_PHRASES, phrases.size()));
        for (Phrase phrase : phrases) {
            if (!usable(phrase) || !texts.add(phrase.text())) continue;
            if (result.size() == MAX_PHRASES) break;
            String group = phrase.group() == null ? "" : phrase.group();
            result.add(new Phrase(phrase.id(), phrase.text(), group, result.size()));
        }
        return Collections.unmodifiableList(result);
    }

    /**
     * 只有本机改动时要上传的整份常用语：本机列表在前，再接上云端里正文属于 `unheld` 的那些。
     *
     * <p>`unheld` 是上次应用云端结果时本机收不下的正文（本机自己添加的上限比服务端小）。它们从没进过本机列表，所以不会被当成本机删除而从云端抹掉；本机真正删掉的常用语从来不在 `unheld` 里，照常随这次上传从云端消失。
     *
     * @param local 本机列表（已换成云端格式）
     * @param cloud 这一轮读到的云端列表
     * @param unheld 上次应用时本机收不下的正文
     */
    public static List<Phrase> uploadPhrases(List<Phrase> local, List<Phrase> cloud, Set<String> unheld) {
        List<Phrase> combined = new ArrayList<>(local);
        if (unheld != null && !unheld.isEmpty()) {
            for (Phrase phrase : cloud) if (phrase != null && unheld.contains(phrase.text())) combined.add(phrase);
        }
        return normalized(combined);
    }

    private static boolean usable(Phrase phrase) {
        return phrase != null && phrase.id() != null && !phrase.id().isEmpty() && phrase.text() != null
            && !phrase.text().isBlank() && phrase.text().length() <= MAX_PHRASE_UNITS
            && phrase.text().indexOf('\0') < 0;
    }

    /**
     * 本机要变成的常用语正文清单与要删掉的本机 id：`target` 是合并或云端的结果，`local` 是本机现有的（id → 正文）。
     *
     * <p>本机 id 由 client-core 生成，和云端 id 不通用，所以按正文对齐：`target` 里有、本机没有的正文要新增；本机有、`target` 里没有的要删掉（只有「使用云端」和下载才会走到删除）。
     */
    public record LocalPlan(List<String> add, List<String> remove) {}

    public static LocalPlan localPlan(Map<String, String> local, List<Phrase> target) {
        Set<String> wanted = new HashSet<>();
        for (Phrase phrase : target) wanted.add(phrase.text());
        Set<String> present = new HashSet<>(local.values());
        List<String> add = new ArrayList<>(target.size());
        for (Phrase phrase : target) if (!present.contains(phrase.text())) add.add(phrase.text());
        List<String> remove = new ArrayList<>(local.size());
        for (Map.Entry<String, String> entry : local.entrySet()) {
            if (!wanted.contains(entry.getValue())) remove.add(entry.getKey());
        }
        return new LocalPlan(Collections.unmodifiableList(add), Collections.unmodifiableList(remove));
    }

    /** 设置文档的叠加：以 `base`（云端）为底，`later`（本机导出）同名键覆盖；结果去掉本地专属的键。 */
    public static Map<String, Object> overlaySettings(Map<String, Object> base, Map<String, Object> later) {
        LinkedHashMap<String, Object> merged = new LinkedHashMap<>(base);
        merged.putAll(later);
        return withoutLocalOnly(merged);
    }

    /** 去掉设备本地且涉及隐私的键（隐私模式、开发者选项、诊断日志、语音贡献）。 */
    public static Map<String, Object> withoutLocalOnly(Map<String, Object> settings) {
        LinkedHashMap<String, Object> kept = new LinkedHashMap<>();
        for (Map.Entry<String, Object> entry : settings.entrySet()) {
            if (!localOnly(entry.getKey())) kept.put(entry.getKey(), entry.getValue());
        }
        return kept;
    }

    public static boolean localOnly(String key) {
        if (key == null) return true;
        for (String fragment : LOCAL_ONLY_FRAGMENTS) if (key.contains(fragment)) return true;
        return false;
    }

    /**
     * 皮肤库最多能占的字节数：整份文档 900 KiB 减去其余键已经用掉的，再不超过字段本身的上限。
     *
     * @param otherDocumentBytes 不含皮肤库键时整份偏好文档编码后的 UTF-8 字节数
     */
    public static long skinBudget(long otherDocumentBytes) {
        long overhead = SKINS_KEY.length() + 8L;
        long room = DOCUMENT_LIMIT_BYTES - Math.max(0L, otherDocumentBytes) - overhead;
        return Math.max(0L, Math.min(SKIN_FIELD_LIMIT, room));
    }

    /** 放不下全部设计时只留最近的：按更新时间从新到旧排，导出时按这个顺序装到预算为止。 */
    public static <T> List<T> newestFirst(List<T> items, ToLongFunction<T> updatedAt) {
        List<T> sorted = new ArrayList<>(items);
        sorted.sort((left, right) -> Long.compare(updatedAt.applyAsLong(right), updatedAt.applyAsLong(left)));
        return sorted;
    }

    /** 有设计因为超出预算没有同步，需要在界面上提示一句。 */
    public static boolean skinsTrimmed(int total, int kept) {
        return kept < total;
    }

    // ---- 个人词库 ----

    /** 个人词库队列一次导入最多的词条数，与 client-core 的 `MAX_QUEUED_IMPORT` 一致。 */
    public static final int PERSONAL_IMPORT_BATCH = 128;

    /** 云端快照里的一个用户词：`kind` 已换成个人词库导入文件的写法。 */
    public record Word(String kind, String key, String value, long weight) {}

    /**
     * 首次开启时个人词库怎么做。快照是整份替换，所以「合并」要分情况：本机没有自己的词就直接下载云端快照；两边都有词时把云端的词导入本机队列（{@link Mode#MERGE}），等键盘应用后再整份上传。
     */
    public static Mode dictionaryFirstRun(boolean cloudHasData, boolean localHasWords, Choice choice) {
        Mode mode = firstRun(cloudHasData, choice);
        if (mode == Mode.MERGE && !localHasWords) return Mode.DOWNLOAD;
        return mode;
    }

    /** 快照里的词库种类换成个人词库导入文件的写法；不认识的种类返回 null，调用方跳过这个词。 */
    public static String personalKind(String snapshotKind) {
        if (snapshotKind == null) return null;
        switch (snapshotKind) {
            case "pinyin":
            case "wubi":
            case "wubi98":
            case "english":
                return snapshotKind;
            case "quick_phrase":
            case "quickPhrase":
                return "quickPhrase";
            default:
                return null;
        }
    }

    /** 把要导入的词按 种类 + 编码 + 词 去重，再切成每批不超过 `size` 条（导入文件里有重复会整批失败）。 */
    public static List<List<Word>> batches(List<Word> words, int size) {
        if (size <= 0) throw new IllegalArgumentException("batch size");
        Set<String> seen = new HashSet<>();
        List<List<Word>> result = new ArrayList<>();
        List<Word> current = new ArrayList<>();
        for (Word word : words) {
            if (word == null || personalKind(word.kind()) == null || word.key() == null || word.key().isEmpty()
                || word.value() == null || word.value().isEmpty()) continue;
            if (!seen.add(personalKind(word.kind()) + '\t' + word.key() + '\t' + word.value())) continue;
            current.add(new Word(personalKind(word.kind()), word.key(), word.value(), word.weight()));
            if (current.size() == size) {
                result.add(Collections.unmodifiableList(current));
                current = new ArrayList<>();
            }
        }
        if (!current.isEmpty()) result.add(Collections.unmodifiableList(current));
        return Collections.unmodifiableList(result);
    }

    /** 上传本机快照之前，个人词库队列里不能还有没被键盘应用的词，否则导出会漏掉它们、上传后云端也跟着丢。 */
    public static boolean uploadBlocked(int pendingQueueCount) {
        return pendingQueueCount > 0;
    }

    /** 下一次自动同步能不能跑：距上次尝试满 {@link #THROTTLE_MILLIS}；有分类带着待上传标记时只要满 {@link #DIRTY_DEBOUNCE_MILLIS}。时钟往回拨时当作已到期。 */
    public static boolean due(long now, long lastAttempt, boolean anyDirty) {
        if (lastAttempt <= 0 || now < lastAttempt) return true;
        long elapsed = now - lastAttempt;
        return elapsed >= THROTTLE_MILLIS || anyDirty && elapsed >= DIRTY_DEBOUNCE_MILLIS;
    }

    public static final long THROTTLE_MILLIS = 5L * 60L * 1000L;
    /** 本机有改动时两次上传之间至少隔这么久，连续改几项设置只上传一次。 */
    public static final long DIRTY_DEBOUNCE_MILLIS = 30L * 1000L;
}
