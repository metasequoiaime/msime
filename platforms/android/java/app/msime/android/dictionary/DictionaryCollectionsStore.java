package app.msime.android;

import android.content.Context;
import app.msime.android.policy.HostOptionsPolicy;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 词库页的数据入口：命名词库（`msime_client_dictionary_collections`）、内置词库的条数与词条（`msime_client_dictionary`），以及经个人词库队列写入的词条（`msime_client_personal_dictionary_request`）。
 *
 * <p>键盘活着的时候 `msime_client_dictionary` 的编辑会返回 busy，所以这里所有写词条的路径都走队列：命名词库的增删由 client-core 自己分批送进队列，内置词库的新词经 {@link #queueWord} 入队，键盘在下一次会话开始时应用。列表、条数和导出只读，直接问 Engine。
 *
 * <p>所有方法都读写磁盘或等词库锁，只能在工作线程调用；失败不抛异常，而是一个带可直接展示原因的 {@link Result}。写词条成功后标记「个人词库」同步分类有本机改动（命名词库的元数据不同步，P9）。
 */
public final class DictionaryCollectionsStore {
    /** 内置主词库在集合请求里的 id 前缀，例如 `builtin:pinyin`。它常开，不能停用或改名。 */
    public static final String BUILTIN_PREFIX = "builtin:";
    /** 内置拼音词库的 id。 */
    public static final String BUILTIN_PINYIN = BUILTIN_PREFIX + "pinyin";
    /** 集合名最多的字数，与 client-core 一致。 */
    public static final int MAX_NAME_CHARS = 32;
    /** 导入文件读进内存的上限，与 client-core 的导入文本上限一致。 */
    public static final int MAX_IMPORT_BYTES = 16 * 1024 * 1024;
    /** 单次导出上限，与 iOS 个人词库导出一致，避免分页结果在 Java 堆中无限累积。 */
    public static final int MAX_EXPORT_BYTES = 8 * 1024 * 1024;
    /** 新词的默认权重，与导入时省略权重的默认值一致。 */
    public static final long DEFAULT_WEIGHT = 10000;
    private static final int EXPORT_PAGE = 1000;

    /** 一个命名词库；`sourceType` 是 `user`、`import` 或 `community`，社区词库带 `resourceId`。 */
    public record Collection(String id, String name, String kind, String sourceType, String resourceId,
                             boolean enabled, int entryCount, int pending) {}

    /** 一次导入的结果。 */
    public record ImportReport(int imported, int duplicates, int failed, boolean truncated) {}

    /** 集合操作返回的整份视图；`formats` 是 client-core 接受的导入格式，`importReport` 只在导入后非空。 */
    public record View(List<Collection> collections, List<String> formats, ImportReport importReport) {
        /** 按 id 找集合，找不到时为 null。 */
        public Collection find(String id) {
            for (Collection collection : collections) if (collection.id().equals(id)) return collection;
            return null;
        }

        /** 某个社区资源是否已经装成了词库。 */
        public boolean installed(String resourceId) {
            for (Collection collection : collections) {
                if ("community".equals(collection.sourceType()) && collection.resourceId().equals(resourceId)) return true;
            }
            return false;
        }
    }

    /** 一个词条：`key` 是编码（拼音带撇号分隔），`source` 是 `user` 或 `bundled`。 */
    public record Word(String kind, String key, String value, long weight, String source) {}

    /** 一页词条。 */
    public record WordPage(List<Word> words, boolean hasMore) {}

    /** 一个导入来源：client-core 的格式名、展示文字和文件选择器用的 MIME 类型。 */
    public record ImportSource(String format, String label, String[] mimeTypes) {}

    /** 一次操作的结果：成功时 `value` 非空、`failure` 为空串；失败时 `value` 为空、`failure` 是可直接展示的原因。 */
    public record Result<T>(T value, String failure) {
        public boolean ok() { return value != null; }

        static <T> Result<T> failed(String failure) { return new Result<>(null, failure); }

        static <T> Result<T> of(T value) { return new Result<>(value, ""); }
    }

    private DictionaryCollectionsStore() {}

    public static Result<View> load(Context context) {
        try {
            return collections(context, action("load"), false);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 把还没交给个人词库队列的增删再送一批，刷新时用。 */
    public static Result<View> flush(Context context) {
        try {
            return collections(context, action("flush"), false);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /**
     * 送一批待发送的增删，返回这次实际送出的条数。
     *
     * <p>个人词库队列一次只收 128 条，导入的大词库要分很多批。键盘每处理完一批就调这里送下一批，送出 0 条（全部送完，或队列还没空出来）时停下，用户不用去词库页手动刷新。
     */
    public static Result<Integer> flushSent(Context context) {
        String options = hostOptions(context);
        if (options.isEmpty()) return Result.failed(failureMessage("unavailable"));
        final String response;
        try {
            response = NativeClient.dictionaryCollections(new JSONObject()
                .put("options", new JSONObject(options)).put("action", action("flush")).toString());
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed(failureMessage(""));
        }
        JSONObject value = value(response);
        if (value == null) return Result.failed(failureMessage(errorOf(response)));
        Integer sent = nonNegativeInteger(value.opt("sent"));
        return Result.of(sent == null ? 0 : sent);
    }

    /** 新建一个空的拼音词库。 */
    public static Result<View> create(Context context, String name) {
        if (!validName(name)) return Result.failed(failureMessage("collections_name_invalid"));
        try {
            return collections(context, action("create").put("name", name).put("kind", "pinyin"), false);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    public static Result<View> rename(Context context, String id, String name) {
        if (!validName(name)) return Result.failed(failureMessage("collections_name_invalid"));
        try {
            return collections(context, action("rename").put("id", id).put("name", name), false);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    public static Result<View> delete(Context context, String id) {
        try {
            return collections(context, action("delete").put("id", id), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    public static Result<View> setEnabled(Context context, String id, boolean enabled) {
        try {
            return collections(context, action("set_enabled").put("id", id).put("enabled", enabled), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 往命名词库里加词，client-core 负责去重和分批入队。 */
    public static Result<View> addWords(Context context, String id, List<Word> words) {
        try {
            return collections(context, action("add_words").put("id", id).put("entries", personalWords(words)), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    public static Result<View> removeWords(Context context, String id, List<Word> words) {
        try {
            return collections(context, action("remove_words").put("id", id).put("entries", personalWords(words)), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /**
     * 把一个文件导入成新的拼音词库。
     *
     * @param format client-core 的格式名（来自 {@link View#formats}）
     * @param name 新词库的名字，已经按 {@link #nameFromFile} 收好
     * @param bytes 文件内容；`scel` 这类二进制格式按 base64 传，其余按 UTF-8 文本传
     */
    public static Result<View> importFile(Context context, String format, String name, byte[] bytes) {
        if (bytes == null || bytes.length == 0) return Result.failed(failureMessage("import_empty"));
        if (bytes.length > MAX_IMPORT_BYTES) return Result.failed(failureMessage("collections_too_large"));
        try {
            JSONObject request = action("import").put("name", name).put("kind", "pinyin").put("format", format);
            if (binaryFormat(format)) {
                request.put("bytes_base64", java.util.Base64.getEncoder().encodeToString(bytes));
            } else {
                request.put("text", new String(bytes, java.nio.charset.StandardCharsets.UTF_8));
            }
            return collections(context, request, true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /**
     * 把一个社区词库装成命名词库。
     *
     * @param resource 社区条目的原始 JSON（`CommunityCatalog.Item#raw`），原样交给 client-core 校验
     */
    public static Result<View> installCommunity(Context context, JSONObject resource) {
        try {
            return collections(context, action("install_community").put("resource", resource), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 内置词库（含用户词）的总条数，`dictionary` 的 `count` 操作。 */
    public static Result<Long> builtinCount(Context context, String kind) {
        try {
            JSONObject value = dictionary(context, action("count").put("kind", kind));
            if (value == null) return Result.failed(failureMessage(""));
            Long count = strictLong(value.opt("count"));
            return Result.of(count == null || count < 0 ? 0L : count);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /**
     * 列出词条。`query` 为空时只列用户自己的词；不为空时按编码前缀查，连内置词一起返回，用户词在前。
     *
     * @param kind `pinyin` 之类的词库种类（`msime_client_dictionary` 的写法）
     * @param query 编码前缀，不带撇号
     */
    public static Result<WordPage> words(Context context, String kind, String query, int offset, int limit) {
        try {
            JSONObject request = action("list").put("kind", kind).put("offset", BoundsPolicy.nonNegative(offset))
                .put("limit", KeyboardGeometry.bounded(limit, 1, 1000));
            if (query != null && !query.isEmpty()) request.put("query", query);
            JSONObject value = dictionary(context, request);
            if (value == null) return Result.failed(failureMessage(""));
            return Result.of(parseWords(value));
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 整个词库按 `standard`（词、编码、权重，制表符分隔）导出成文本，分页读完再交给调用方写文件。 */
    public static Result<String> export(Context context, String kind) {
        StringBuilder text = null;
        int bytes = 0;
        int offset = 0;
        try {
            while (true) {
                JSONObject value = dictionary(context, action("export").put("kind", kind).put("format", "standard")
                    .put("offset", offset).put("limit", EXPORT_PAGE));
                if (value == null) return Result.failed(failureMessage(""));
                String page = exportPage(value.opt("text"));
                if (page == null) return Result.failed(failureMessage(""));
                int nextBytes = exportBytesAfterPage(bytes, page);
                if (nextBytes < 0) return Result.failed(failureMessage("collections_too_large"));
                if (text == null) text = new StringBuilder(Math.max(16, page.length()));
                text.append(page);
                bytes = nextBytes;
                Object rawHasMore = value.opt("has_more");
                if (rawHasMore == null || rawHasMore == JSONObject.NULL) break;
                Boolean hasMore = JsonPolicy.strictBoolean(rawHasMore);
                if (hasMore == null) return Result.failed(failureMessage(""));
                if (!hasMore) break;
                offset += EXPORT_PAGE;
            }
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
        return Result.of(text == null ? "" : text.toString());
    }

    /** 返回追加一页后的 UTF-8 字节数；超出导出上限时返回负数。 */
    public static int exportBytesAfterPage(int currentBytes, String page) {
        if (currentBytes < 0 || page == null) return -1;
        long next = (long) currentBytes + page.getBytes(java.nio.charset.StandardCharsets.UTF_8).length;
        return next > MAX_EXPORT_BYTES ? -1 : (int) next;
    }

    /** 把一个新词放进个人词库队列（键盘活着时直接编辑会返回 busy），返回队列里还没应用的条数。 */
    public static Result<Integer> queueWord(Context context, Word word) {
        String options = hostOptions(context);
        if (options.isEmpty()) return Result.failed(failureMessage("unavailable"));
        try {
            JSONObject action = action("edit").put("previous", JSONObject.NULL)
                .put("replacement", entry(word)).put("request_id", UUID.randomUUID().toString());
            String response = NativeClient.personalDictionaryRequest(new JSONObject()
                .put("options", new JSONObject(options)).put("action", action).toString());
            JSONObject value = value(response);
            if (value == null) return Result.failed(failureMessage(errorOf(response)));
            SyncSignals.markDirty(context, SyncSwitch.DICTIONARY);
            Integer pending = nonNegativeInteger(value.opt("pending_count"));
            return Result.of(pending == null ? 0 : pending);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 集合名能否使用，与 client-core 一致：1–32 个字，首尾没有空白，不含控制字符和换行。 */
    public static boolean validName(String name) {
        if (name == null || name.isEmpty() || !name.equals(TextPolicy.stripped(name))) return false;
        if (!TextPolicy.withinCodePoints(name, MAX_NAME_CHARS)) return false;
        for (int index = 0; index < name.length(); index++) {
            if (Character.isISOControl(name.charAt(index))) return false;
        }
        return true;
    }

    /** 拼音编码能否作为新词的编码：小写字母，音节之间可以用撇号分隔，不以撇号开头或结尾。 */
    public static boolean validPinyin(String code) {
        if (code == null || code.isEmpty() || code.length() > 64) return false;
        if (code.charAt(0) == '\'' || code.charAt(code.length() - 1) == '\'') return false;
        boolean previousApostrophe = false;
        for (int index = 0; index < code.length(); index++) {
            char character = code.charAt(index);
            boolean apostrophe = character == '\'';
            if (!apostrophe && (character < 'a' || character > 'z')) return false;
            if (apostrophe && previousApostrophe) return false;
            previousApostrophe = apostrophe;
        }
        return true;
    }

    /** 把用户输入的拼音收成编码：去掉空白、转小写，空格和中文撇号都当作音节分隔。 */
    public static String normalizePinyin(String input) {
        if (input == null) return "";
        String lower = TextPolicy.lowercase(TextPolicy.stripped(input))
            .replace('’', '\'').replace('‘', '\'');
        return lower.replaceAll("\\s+", "'");
    }

    /** 编码的展示写法：音节之间用排版撇号（`pin’yin`），与设计一致。 */
    public static String displayCode(String code) {
        return code == null ? "" : code.replace('\'', '’');
    }

    /** 条数的展示写法，例如 `128,406 条`。 */
    public static String countLabel(long count) {
        return NumberPolicy.grouped(BoundsPolicy.nonNegative(count)) + " 条";
    }

    /** 从文件名得到新词库的名字：去掉扩展名（`.dict.yaml` 算一个），截到 32 个字，收不出来时用「导入的词库」。 */
    public static String nameFromFile(String displayName) {
        String name = TextPolicy.stripped(displayName);
        String lower = TextPolicy.lowercase(name);
        if (lower.endsWith(".dict.yaml")) {
            name = name.substring(0, name.length() - ".dict.yaml".length());
        } else {
            int dot = name.lastIndexOf('.');
            if (dot >= 0) name = name.substring(0, dot);
        }
        StringBuilder kept = new StringBuilder(MAX_NAME_CHARS);
        int count = 0;
        for (int index = 0; index < name.length() && count < MAX_NAME_CHARS; ) {
            int codePoint = name.codePointAt(index);
            index += Character.charCount(codePoint);
            if (Character.isISOControl(codePoint)) continue;
            kept.appendCodePoint(codePoint);
            count++;
        }
        String result = TextPolicy.stripped(kept.toString());
        return validName(result) ? result : "导入的词库";
    }

    /** 按文件名推断格式；推不出来时用调用方在来源对话框里选的格式。 */
    public static String formatForFile(String displayName, String chosen) {
        String lower = TextPolicy.lowercase(displayName);
        if (lower.endsWith(".scel")) return "scel";
        if (lower.endsWith(".yaml") || lower.endsWith(".yml")) return "rime";
        return chosen;
    }

    /** 二进制格式按 base64 传。 */
    public static boolean binaryFormat(String format) {
        return "scel".equals(format);
    }

    /**
     * 导入来源对话框的选项：只列 client-core 实际接受的格式，按「文本、搜狗、Rime、纯汉字」排；`standard` 和 `windows` 是文本文件的两种列顺序，由 client-core 在 `txt` 里自动识别，不单独列出。
     */
    public static List<ImportSource> importSources(List<String> formats) {
        List<ImportSource> sources = new ArrayList<>(formats.size());
        if (formats.contains("txt")) {
            sources.add(new ImportSource("txt", "文本文件（.txt）", new String[] {"text/plain"}));
        }
        if (formats.contains("scel")) {
            sources.add(new ImportSource("scel", "搜狗细胞词库（.scel）", new String[] {"application/octet-stream", "*/*"}));
        }
        if (formats.contains("rime")) {
            sources.add(new ImportSource("rime", "Rime 词典（.dict.yaml）", new String[] {"application/x-yaml", "text/yaml", "text/plain", "*/*"}));
        }
        if (formats.contains("hans")) {
            sources.add(new ImportSource("hans", "纯汉字词表（每行一个词）", new String[] {"text/plain"}));
        }
        return Collections.unmodifiableList(sources);
    }

    /** 把 client-core 的错误码换成可直接展示的话；不认识的码一律按通用失败处理。 */
    public static String failureMessage(String code) {
        if (code == null) code = "";
        switch (code) {
            case "collections_name_invalid": return "词库名需要 1–32 个字，首尾不能有空格。";
            case "collections_limit": return "词库数量或词条数已达上限。";
            case "collections_not_found": return "这个词库已经不在了，列表已刷新。";
            case "builtin_locked": return "内置词库始终启用，不能停用或改名。";
            case "unsupported_format": return "暂不支持这种文件格式。";
            case "import_empty": return "文件是空的。";
            case "collections_too_large": return "文件太大，词库存不下。";
            case "import_control_characters": return "文件里有无法识别的字符，请确认是文本格式。";
            case "import_no_usable_rows": return "没有读到可用的词条，请检查文件格式。";
            case "collections_corrupt": return "词库文件已损坏，为避免丢失没有改动它。";
            case "personal_dictionary_busy": return "键盘正在整理词库，请稍后再试。";
            case "unavailable": return "词库还没准备好，请先完成首次设置。";
            default: return "词库操作失败，请稍后重试。";
        }
    }

    /** 解析集合视图（`value`）。 */
    static View parseView(JSONObject value) {
        JSONArray raw = value.optJSONArray("collections");
        List<Collection> collections = new ArrayList<>(raw == null ? 0 : raw.length());
        if (raw != null) {
            for (int index = 0; index < raw.length(); index++) {
                JSONObject item = raw.optJSONObject(index);
                if (item == null) continue;
                JSONObject source = item.optJSONObject("source");
                String id = JsonPolicy.strictString(item.opt("id"));
                String name = JsonPolicy.strictString(item.opt("name"));
                String kind = JsonPolicy.strictString(item.opt("kind"));
                Boolean enabled = JsonPolicy.strictBoolean(item.opt("enabled"));
                Integer entryCount = nonNegativeInteger(item.opt("entry_count"));
                Integer pending = nonNegativeInteger(item.opt("pending"));
                String type = source == null ? "user" : JsonPolicy.strictString(source.opt("type"));
                String resource = source == null || !source.has("resource_id")
                    ? "" : JsonPolicy.strictString(source.opt("resource_id"));
                if (id == null || name == null || kind == null || enabled == null
                        || entryCount == null || pending == null || type == null
                        || resource == null) continue;
                collections.add(new Collection(id, name, kind, type, resource, enabled,
                    entryCount, pending));
            }
        }
        JSONArray rawFormats = value.optJSONArray("formats");
        List<String> formats = new ArrayList<>(rawFormats == null ? 0 : rawFormats.length());
        if (rawFormats != null) {
            for (int index = 0; index < rawFormats.length(); index++) {
                String format = JsonPolicy.strictString(rawFormats.opt(index));
                if (format != null && !format.isEmpty()) formats.add(format);
            }
        }
        JSONObject report = value.optJSONObject("import");
        ImportReport importReport = report == null ? null : new ImportReport(
            nonNegativeInteger(report.opt("imported"), 0), nonNegativeInteger(report.opt("duplicates"), 0),
            nonNegativeInteger(report.opt("failed"), 0), JsonPolicy.strictTrue(report.opt("truncated")));
        return new View(Collections.unmodifiableList(collections), Collections.unmodifiableList(formats), importReport);
    }

    /** 解析一页词条（`value`）。 */
    static WordPage parseWords(JSONObject value) {
        JSONArray raw = value.optJSONArray("entries");
        List<Word> words = new ArrayList<>(raw == null ? 0 : raw.length());
        if (raw != null) {
            for (int index = 0; index < raw.length(); index++) {
                JSONObject item = raw.optJSONObject(index);
                if (item == null) continue;
                String kind = JsonPolicy.strictString(item.opt("kind"));
                String key = JsonPolicy.strictString(item.opt("key"));
                String word = JsonPolicy.strictString(item.opt("value"));
                Long weight = strictLong(item.opt("weight"));
                String source = JsonPolicy.strictString(item.opt("source"));
                if (kind == null || key == null || word == null || weight == null || source == null) continue;
                words.add(new Word(kind, key, word, weight, source));
            }
        }
        return new WordPage(Collections.unmodifiableList(words), JsonPolicy.strictTrue(value.opt("has_more")));
    }

    private static Result<View> collections(Context context, JSONObject action, boolean changesWords) {
        String options = hostOptions(context);
        if (options.isEmpty()) return Result.failed(failureMessage("unavailable"));
        final String response;
        try {
            response = NativeClient.dictionaryCollections(new JSONObject()
                .put("options", new JSONObject(options)).put("action", action).toString());
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed(failureMessage(""));
        }
        JSONObject value = value(response);
        if (value == null) return Result.failed(failureMessage(errorOf(response)));
        if (changesWords) SyncSignals.markDirty(context, SyncSwitch.DICTIONARY);
        return Result.of(parseView(value));
    }

    private static JSONObject dictionary(Context context, JSONObject action) throws JSONException {
        String options = hostOptions(context);
        if (options.isEmpty()) return null;
        try {
            return value(NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(options)).put("action", action).toString()));
        } catch (RuntimeException | LinkageError error) {
            return null;
        }
    }

    /** 个人词库队列和集合接口用的词条写法（`PersonalWord`：kind 用驼峰写法）。 */
    private static JSONArray personalWords(List<Word> words) throws JSONException {
        JSONArray entries = new JSONArray();
        for (Word word : words) {
            entries.put(new JSONObject().put("kind", personalKind(word.kind())).put("key", word.key())
                .put("value", word.value()).put("weight", word.weight()));
        }
        return entries;
    }

    /** `msime_client_dictionary` 的 Entry 写法（kind 用下划线写法）。 */
    private static JSONObject entry(Word word) throws JSONException {
        return new JSONObject().put("kind", word.kind()).put("key", word.key())
            .put("value", word.value()).put("weight", word.weight());
    }

    private static String personalKind(String kind) {
        return "quick_phrase".equals(kind) ? "quickPhrase" : kind;
    }

    private static JSONObject action(String operation) throws JSONException {
        return new JSONObject().put("operation", operation);
    }

    private static JSONObject value(String response) {
        if (response == null) return null;
        try {
            JSONObject root = new JSONObject(response);
            return JsonPolicy.strictTrue(root.opt("ok"))
                ? root.optJSONObject("value") : null;
        } catch (JSONException error) {
            return null;
        }
    }

    /** JSON response flags must remain booleans; org.json otherwise coerces strings. */
    static Boolean strictBoolean(Object value) {
        return JsonPolicy.strictBoolean(value);
    }

    public static String strictString(Object value) {
        return JsonPolicy.strictString(value);
    }

    /** Export pages are text from the native response; do not let org.json coerce malformed values. */
    public static String exportPage(Object value) {
        return strictString(value);
    }

    public static Integer strictInteger(Object value) {
        return JsonPolicy.strictInteger(value);
    }

    /** 词库计数必须是非负 JSON 整数；非法值按调用方的缺省值处理。 */
    public static Integer nonNegativeInteger(Object value) {
        Integer parsed = strictInteger(value);
        return parsed == null || parsed < 0 ? null : parsed;
    }

    static int nonNegativeInteger(Object value, int fallback) {
        Integer parsed = nonNegativeInteger(value);
        return parsed == null ? fallback : parsed;
    }

    public static Long strictLong(Object value) {
        return JsonPolicy.strictLong(value);
    }

    private static String errorOf(String response) {
        if (response == null) return "";
        try {
            return new JSONObject(response).optString("error", "");
        } catch (JSONException error) {
            return "";
        }
    }

    /** runtime-options.json 原文，就是 `msime_client_dictionary` 要的 HostOptions；首次设置之前为空串。 */
    static String hostOptions(Context context) {
        return HostOptionsPolicy.readRuntimeOptions(context.getFilesDir());
    }
}
