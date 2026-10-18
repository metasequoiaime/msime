package app.msime.android;

import java.io.BufferedReader;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Set;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 从词库快照文件（NDJSON）里读出用户自己的词，按个人词库导入文件的写法换算种类、去重、分批。本地备份恢复走「合并」时用它把备份里的词排进命名词库的待发送队列（{@link DictionaryCollectionsStore#queueUnownedWords}）。
 *
 * <p>云同步不经过这里：快照的上传、下载、合并都在 Rust 的 {@code msime_client_cloud_sync}（#8201 迁移计划 5d），这里只剩本地备份要的那部分读取。
 */
public final class DictionarySnapshotWords {
    /** 与 NativeClient 的快照行上限一致，避免为一行异常数据分配整份快照。 */
    static final int MAX_LINE_CHARS = 65_535;
    /** 与原生快照检查一致，避免把超量记录全部留在 Java 集合中。 */
    static final int MAX_RECORDS = 500_000;

    /** 快照里的一个用户词：`kind` 已换成个人词库导入文件的写法。 */
    public record Word(String kind, String key, String value, long weight) {}

    private DictionarySnapshotWords() {}

    /**
     * 快照里用户自己的词（`type:"entry"`）。被 `overlay` 标成已删除的词不算。词条数不设上限之外的限制，调用方按 {@link #batches} 分批。
     */
    public static List<Word> read(Path file) throws IOException {
        LinkedHashMap<String, Word> words = new LinkedHashMap<>();
        HashSet<String> deleted = new HashSet<>();
        try (BufferedReader reader = Files.newBufferedReader(file, StandardCharsets.UTF_8)) {
            RecordReader records = new RecordReader(reader);
            int countedRecords = 0;
            String line;
            while ((line = records.next()) != null) {
                if (line.isEmpty()) continue;
                JSONObject record;
                try {
                    record = new JSONObject(line);
                } catch (JSONException malformed) {
                    throw new IOException("malformed snapshot line", malformed);
                }
                Object type = record.opt("type");
                if (!"footer".equals(type) && ++countedRecords > MAX_RECORDS)
                    throw new IOException("snapshot has too many records");
                JSONObject data = record.optJSONObject("data");
                if (data == null || !(data.opt("id") instanceof String)) continue;
                String id = (String) data.opt("id");
                if ("overlay".equals(type)) {
                    if (JsonPolicy.strictTrue(record.opt("deleted"))) deleted.add(id);
                    continue;
                }
                if (!"entry".equals(type)) continue;
                Object kind = data.opt("kind");
                Object code = data.opt("code");
                Object word = data.opt("word");
                Object weight = data.opt("weight");
                if (!(kind instanceof String) || !(code instanceof String) || !(word instanceof String)) continue;
                Long strictWeight = strictWeight(weight);
                if (strictWeight == null) throw new IOException("invalid snapshot weight");
                words.put(id, new Word((String) kind, (String) code, (String) word, strictWeight));
            }
        }
        for (String id : deleted) words.remove(id);
        return new ArrayList<>(words.values());
    }

    /** 快照词条的权重是 host-api 范围内的正整数（JSON 整数）。 */
    public static Long strictWeight(Object value) {
        if (!(value instanceof Integer) && !(value instanceof Long)) return null;
        long weight = ((Number) value).longValue();
        return weight >= 1L && weight <= 100_000_000L ? weight : null;
    }

    /** 逐字符读一行并在超过快照契约前失败，不能先调用无界的 {@link BufferedReader#readLine}。 */
    static String readLine(BufferedReader reader) throws IOException {
        StringBuilder line = new StringBuilder(256);
        int value;
        while ((value = reader.read()) != -1) {
            if (value == '\n') {
                int length = line.length();
                if (length > 0 && line.charAt(length - 1) == '\r') line.setLength(length - 1);
                return line.toString();
            }
            if (line.length() >= MAX_LINE_CHARS)
                throw new IOException("snapshot line too large");
            line.append((char) value);
        }
        return line.length() == 0 ? null : line.toString();
    }

    /** 带总记录数上限的快照逐行读取器，避免调用方先把所有记录放进集合。 */
    static final class RecordReader {
        private final BufferedReader reader;
        private int records;

        RecordReader(BufferedReader reader) {
            this.reader = reader;
        }

        String next() throws IOException {
            String line = readLine(reader);
            if (line == null) return null;
            // 原生 records 不包含 footer；预留这一行后，再由 read 按 type 计数。
            if (++records > MAX_RECORDS + 1)
                throw new IOException("snapshot has too many records");
            return line;
        }
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
            // 快照和本机 export_snapshot 写的都是 `quick`；漏掉它的话本地恢复会悄悄丢掉全部快捷短语。
            case "quick":
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
        Set<String> seen = new HashSet<>(words.size());
        List<List<Word>> result = new ArrayList<>((words.size() + size - 1) / size);
        List<Word> current = new ArrayList<>(BoundsPolicy.bounded(words.size(), 0, size));
        for (Word word : words) {
            if (word == null || personalKind(word.kind()) == null || word.key() == null || word.key().isEmpty()
                || word.value() == null || word.value().isEmpty()) continue;
            if (!seen.add(personalKind(word.kind()) + '\t' + word.key() + '\t' + word.value())) continue;
            current.add(new Word(personalKind(word.kind()), word.key(), word.value(), word.weight()));
            if (current.size() == size) {
                result.add(ListPolicy.copyOrEmpty(current));
                current = new ArrayList<>(BoundsPolicy.atMost(
                    words.size() - result.size() * size, size));
            }
        }
        if (!current.isEmpty()) result.add(ListPolicy.copyOrEmpty(current));
        return ListPolicy.copyOrEmpty(result);
    }
}
