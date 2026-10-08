package app.msime.android;

import android.content.Context;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.LinkOption;
import java.nio.file.NoSuchFileException;
import java.nio.file.OpenOption;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.function.UnaryOperator;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 无编码常用语的唯一读写入口：设置里的常用语页、键盘的「常用语」面板和云同步都经过这里，不各自拼请求。
 *
 * <p>数据存在偏好目录下的 `CommonPhrases.json`，由 client-core 加文件锁原子读写（`msime_client_common_phrases`），设置进程和 `:ime` 进程共用同一份。每次操作都返回整份列表，调用方直接重画。每次写成功后经 {@link SyncSignals#markDirty} 标记「常用语」分类有本机改动，同步关闭时那一步什么也不做。
 *
 * <p>所有方法都会读写磁盘并等文件锁，只能在工作线程调用。失败不抛异常，而是一个带可直接展示原因的 {@link Result}。
 */
public final class CommonPhrasesStore {
    /** 自己添加的常用语最多条数，与 client-core 的上限一致。 */
    public static final int MAX_OWN_PHRASES = 200;
    /** 每条常用语最多的 UTF-16 单元数，与 client-core 的上限一致。 */
    public static final int MAX_PHRASE_UNITS = 1000;

    /** 一条常用语；`pack` 为空串表示用户自己添加的，否则是它所属社区短语包的 id。 */
    public record Phrase(String id, String text, String pack) {
        public boolean own() { return pack.isEmpty(); }
    }

    /** 一个已安装的社区短语包。 */
    public record Pack(String id, String name, int revision) {}

    /** 整份常用语；`skipped` 只在安装短语包时可能大于 0，是因为过长或重复而没有收下的条数。 */
    public record Document(List<Phrase> phrases, List<Pack> packs, int skipped) {
        public static final Document EMPTY = new Document(List.of(), List.of(), 0);

        /** 用户自己添加的条数，用来在界面上提前拦住超过上限的添加。 */
        public int ownCount() {
            int count = 0;
            for (Phrase phrase : phrases) if (phrase.own()) count++;
            return count;
        }
    }

    /** 一次操作的结果：成功时 `document` 非空、`failure` 为空串；失败时 `document` 为空、`failure` 是可直接展示的原因。 */
    public record Result(Document document, String failure) {
        public boolean ok() { return document != null; }

        static Result failed(String failure) { return new Result(null, failure); }
    }

    /** 第一次读到空的常用语时预置的示例，取自设计稿；只放一次，用户删光以后不会再冒出来。每条都不含换行，标记文件按行记录它们。 */
    static final List<String> STARTER_PHRASES = List.of(
        "好的，收到", "我在开会，稍后回复你", "马上到", "辛苦了，谢谢！",
        "稍等，我马上回来", "方便的时候回个电话", "周末一起吃饭吗？", "已处理，请查收");
    /**
     * 放过示例的标记，与 `CommonPhrases.json` 同目录；有它就不再预置。内容是还没被用户认领的示例正文，一行一条。
     *
     * <p>示例只属于这台设备：云同步读本机列表时跳过这些正文（{@link #untouchedStarters}），所以它们既不会被上传，也不会在合并时被带到用户已经删掉它们的别的设备上。用户自己添加或改成某条示例的正文、或者云端本来就有这条正文时，它就被认领（{@link #adoptStarters}），从此和其他常用语一样同步。
     */
    static final String STARTER_MARKER = "CommonPhrases.seeded";
    /** 标记文件的读取上限；八条示例远用不到，超过就当作文件已坏、按没有记录处理。 */
    private static final int MAX_STARTER_RECORD_BYTES = 64 * 1024;
    /** 标记文件的读改写在本进程内排队；跨进程（设置主进程与 `:ime`）靠文件锁。文件锁属于整个 JVM，同一进程的另一个线程已持有时会抛 OverlappingFileLockException 而不是等待，所以两把都要。 */
    private static final Object STARTER_LOCK = new Object();

    private CommonPhrasesStore() {}

    /**
     * 读整份常用语。第一次读到的是空列表（没有常用语也没有短语包）且常用语同步没有打开时，先按 {@link #STARTER_PHRASES} 逐条添加，再把放进去的正文写进标记文件；之后只读不补。
     *
     * <p>同步已经打开时不放示例，只写一个空标记：这台设备的常用语来自云端，示例混进去只会让人以为是别处同步来的。放示例不标记「常用语有本机改动」，因为它们本来就不参与同步。
     *
     * <p>设置进程和 `:ime` 进程可能同时第一次读：重复的文字会被 client-core 拒收，所以不会预置出两份；标记文件按并集写入，两边各自放进去的都记得住。
     */
    public static Result load(Context context) {
        Result result = perform(context, action("load"), false);
        if (!result.ok()) return result;
        String directory = preferencesDirectory(context);
        if (directory.isEmpty()) return result;
        File marker = new File(directory, STARTER_MARKER);
        if (marker.exists()) return result;
        List<String> seeded = new ArrayList<>(STARTER_PHRASES.size());
        if (result.document().phrases().isEmpty() && result.document().packs().isEmpty()
                && !SyncSignals.state(context).enabled()) {
            for (String text : STARTER_PHRASES) {
                Result added = seed(context, text);
                if (added.ok()) {
                    result = added;
                    seeded.add(text);
                }
            }
        }
        try {
            editStarters(marker, true, current -> {
                Set<String> next = new LinkedHashSet<>(current);
                next.addAll(seeded);
                return next;
            });
        } catch (IOException error) {
            // 写不下标记时下次再试；最坏是对仍然为空的列表再补一次示例，不会覆盖用户自己的常用语。
            android.util.Log.w("MSIMEPhrases", "Starter marker was not written", error);
        }
        return result;
    }

    public static Result add(Context context, String text) {
        if (!validText(text)) return Result.failed(failureMessage("common_phrases_invalid"));
        try {
            return adopted(context, text, perform(context, action("add").put("text", text), true));
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 放一条示例：和 {@link #add} 一样交给 client-core，但不标记同步改动、也不认领。 */
    private static Result seed(Context context, String text) {
        try {
            return perform(context, action("add").put("text", text), false);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 用户自己写下的正文（新增或改成的）即使和某条示例相同，也是用户的常用语了，从示例记录里认领出来。认领失败只记日志：这条照样存下了，最坏是它暂时不参与同步。 */
    private static Result adopted(Context context, String text, Result result) {
        if (!result.ok()) return result;
        try {
            adoptStarters(context, List.of(text));
        } catch (IOException error) {
            android.util.Log.w("MSIMEPhrases", "Starter record was not updated", error);
        }
        return result;
    }

    /**
     * 本机放过、还没被认领的示例正文。云同步读本机列表时跳过这些正文，所以示例不会被上传，也不会在合并时出现在别的设备上。没有放过示例或首次设置之前是空集合。
     *
     * @throws IOException 标记文件读不了；调用方应当放弃这一轮常用语同步，而不是把示例当成用户的常用语上传
     */
    public static Set<String> untouchedStarters(Context context) throws IOException {
        String directory = preferencesDirectory(context);
        if (directory.isEmpty()) return Set.of();
        return editStarters(new File(directory, STARTER_MARKER), false, UnaryOperator.identity());
    }

    /**
     * 认领 `texts` 里的示例正文：它们从此和用户自己的常用语一样参与同步。云同步拿到云端或合并结果后要先调用这一步，云端已有的正文即使和本机的示例相同，也不能在之后的上传里被当成本机删除。
     *
     * @throws IOException 标记文件写不了；云同步此时应当放弃这一轮，免得下次上传把云端的同一条正文删掉
     */
    public static void adoptStarters(Context context, Collection<String> texts) throws IOException {
        String directory = preferencesDirectory(context);
        if (directory.isEmpty() || texts.isEmpty()) return;
        editStarters(new File(directory, STARTER_MARKER), false, current -> {
            Set<String> next = new LinkedHashSet<>(current);
            next.removeAll(texts);
            return next;
        });
    }

    /**
     * 在锁里读出标记文件记录的示例正文，交给 `edit` 改，有变化时整份写回；返回改后的集合。文件不存在且 `create` 为假时什么也不做，返回空集合（没有放过示例）。
     */
    static Set<String> editStarters(File marker, boolean create, UnaryOperator<Set<String>> edit) throws IOException {
        List<OpenOption> options = new ArrayList<>(List.of(StandardOpenOption.READ, StandardOpenOption.WRITE,
            LinkOption.NOFOLLOW_LINKS));
        if (create) options.add(StandardOpenOption.CREATE);
        synchronized (STARTER_LOCK) {
            if (!create && !marker.exists()) return Set.of();
            try (FileChannel channel = FileChannel.open(marker.toPath(), options.toArray(new OpenOption[0]))) {
                FileLock lock = channel.lock();
                try {
                    Set<String> current = decodeStarters(readAll(channel));
                    Set<String> next = Collections.unmodifiableSet(new LinkedHashSet<>(edit.apply(current)));
                    if (!next.equals(current)) {
                        channel.truncate(0);
                        ByteBuffer bytes = ByteBuffer.wrap(encodeStarters(next));
                        while (bytes.hasRemaining()) channel.write(bytes, bytes.position());
                        channel.force(false);
                    }
                    return next;
                } finally {
                    lock.release();
                }
            } catch (NoSuchFileException removed) {
                return Set.of();
            }
        }
    }

    private static byte[] readAll(FileChannel channel) throws IOException {
        long size = channel.size();
        if (size > MAX_STARTER_RECORD_BYTES) return new byte[0];
        ByteBuffer buffer = ByteBuffer.allocate((int) size);
        while (buffer.hasRemaining()) {
            // 返回 -1 说明文件在读的过程中变短（锁只拦得住守规矩的写入方），就用已经读到的部分。
            if (channel.read(buffer, buffer.position()) < 0) break;
        }
        return java.util.Arrays.copyOf(buffer.array(), buffer.position());
    }

    /** 标记文件的格式：UTF-8，一行一条正文；空行忽略。旧版本写下的空标记解出来是空集合。 */
    static Set<String> decodeStarters(byte[] bytes) {
        Set<String> texts = new LinkedHashSet<>(STARTER_PHRASES.size());
        for (String line : new String(bytes, StandardCharsets.UTF_8).split("\n")) {
            if (!line.isEmpty()) texts.add(line);
        }
        return Collections.unmodifiableSet(texts);
    }

    static byte[] encodeStarters(Set<String> texts) {
        int capacity = 0;
        for (String text : texts) capacity += text.length() + 1;
        StringBuilder builder = new StringBuilder(capacity);
        for (String text : texts) builder.append(text).append('\n');
        return builder.toString().getBytes(StandardCharsets.UTF_8);
    }

    public static Result remove(Context context, String id) {
        try {
            return perform(context, action("remove").put("id", id), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    public static Result replace(Context context, String id, String text) {
        if (!validText(text)) return Result.failed(failureMessage("common_phrases_invalid"));
        try {
            return adopted(context, text, perform(context, action("replace").put("id", id).put("text", text), true));
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 把一条移到第 `index` 位（从 0 数）。 */
    public static Result move(Context context, String id, int index) {
        try {
            return perform(context, action("move").put("id", id).put("index", BoundsPolicy.nonNegative(index)), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /**
     * 安装一个社区短语包。
     *
     * @param resource 社区条目的原始 JSON（`CommunityCatalog.Item#raw`），原样交给 client-core 校验
     */
    public static Result installPack(Context context, JSONObject resource) {
        try {
            return perform(context, action("install_pack").put("resource", resource), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 删除一个短语包，连同来自它的全部常用语。 */
    public static Result removePack(Context context, String id) {
        try {
            return perform(context, action("remove_pack").put("id", id), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /**
     * 常用语正文能否保存，与 client-core 的 `valid_phrase_text` 一致：不全是空白，1–1000 个 UTF-16 单元，除换行以外不含控制字符。界面用它决定确认按钮是否可点，最终仍以 client-core 的校验为准。
     */
    public static boolean validText(String text) {
        if (text == null || text.isBlank() || text.length() > MAX_PHRASE_UNITS) return false;
        for (int index = 0; index < text.length(); index++) {
            char character = text.charAt(index);
            if (character != '\n' && Character.isISOControl(character)) return false;
        }
        return true;
    }

    /** 把 client-core 的错误码换成可直接展示的话；不认识的码一律按通用失败处理，不把内部信息漏给用户。 */
    public static String failureMessage(String code) {
        if (code == null) code = "";
        switch (code) {
            case "common_phrases_invalid": return "常用语不能为空，最多 1000 字。";
            case "common_phrases_duplicate": return "已经有这条常用语了。";
            case "common_phrases_limit": return "常用语已达上限（自己添加的最多 200 条），请先删掉一些。";
            case "common_phrases_too_large": return "常用语太多，存不下了，请先删掉一些。";
            case "common_phrases_not_found": return "这条常用语已经不在了，列表已刷新。";
            case "common_phrases_corrupt": return "常用语文件已损坏，为避免丢失没有改动它。";
            case "unavailable": return "词库还没准备好，请先完成首次设置。";
            default: return "常用语保存失败，请稍后重试。";
        }
    }

    /** 解析一次答复的 `value`。 */
    static Document parse(JSONObject value) {
        JSONArray rawPhrases = value.optJSONArray("phrases");
        List<Phrase> phrases = new ArrayList<>(rawPhrases == null ? 0 : rawPhrases.length());
        if (rawPhrases != null) {
            for (int index = 0; index < rawPhrases.length(); index++) {
                JSONObject phrase = rawPhrases.optJSONObject(index);
                if (phrase == null) continue;
                String id = strictString(phrase.opt("id"));
                String text = strictString(phrase.opt("text"));
                Object rawPack = phrase.opt("pack");
                String pack = rawPack == null || rawPack == JSONObject.NULL ? "" : strictString(rawPack);
                if (id == null || text == null || pack == null) continue;
                phrases.add(new Phrase(id, text, pack));
            }
        }
        JSONArray rawPacks = value.optJSONArray("packs");
        List<Pack> packs = new ArrayList<>(rawPacks == null ? 0 : rawPacks.length());
        if (rawPacks != null) {
            for (int index = 0; index < rawPacks.length(); index++) {
                JSONObject pack = rawPacks.optJSONObject(index);
                if (pack == null) continue;
                String id = strictString(pack.opt("id"));
                String name = strictString(pack.opt("name"));
                Integer revision = nonNegativeInteger(pack.opt("revision"));
                if (id == null || name == null || revision == null) continue;
                packs.add(new Pack(id, name, revision));
            }
        }
        return new Document(Collections.unmodifiableList(phrases), Collections.unmodifiableList(packs),
            nonNegativeInteger(value.opt("skipped"), 0));
    }

    private static Result perform(Context context, JSONObject action, boolean writes) {
        if (action == null) return Result.failed(failureMessage(""));
        String directory = preferencesDirectory(context);
        if (directory.isEmpty()) return Result.failed(failureMessage("unavailable"));
        final String response;
        try {
            response = NativeClient.commonPhrases(new JSONObject()
                .put("directory", directory).put("action", action).toString());
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed(failureMessage(""));
        }
        try {
            JSONObject root = new JSONObject(response == null ? "" : response);
            if (!JsonPolicy.strictTrue(root.opt("ok")))
                return Result.failed(failureMessage(root.optString("error", "")));
            JSONObject value = root.optJSONObject("value");
            if (value == null) return Result.failed(failureMessage(""));
            Document document = parse(value);
            if (writes) SyncSignals.markDirty(context, SyncSwitch.PHRASES);
            return new Result(document, "");
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    private static JSONObject action(String operation) {
        try {
            return new JSONObject().put("operation", operation);
        } catch (JSONException error) {
            return null;
        }
    }

    /** Native response status must remain a JSON boolean; reject org.json string coercion. */
    static Boolean strictBoolean(Object value) {
        return JsonPolicy.strictBoolean(value);
    }

    /** Persisted response fields must retain their JSON string type. */
    public static String strictString(Object value) {
        return JsonPolicy.strictString(value);
    }

    /** Read a JSON integer without org.json's string or fractional coercion. */
    public static Integer strictInteger(Object value) {
        return JsonPolicy.strictInteger(value);
    }

    /** 常用语包修订号和跳过条数必须是非负 JSON 整数。 */
    public static Integer nonNegativeInteger(Object value) {
        Integer parsed = strictInteger(value);
        return parsed == null || parsed < 0 ? null : parsed;
    }

    static int strictInteger(Object value, int fallback) {
        Integer parsed = strictInteger(value);
        return parsed == null ? fallback : parsed;
    }

    static int nonNegativeInteger(Object value, int fallback) {
        Integer parsed = nonNegativeInteger(value);
        return parsed == null ? fallback : parsed;
    }

    /** Bootstrap 写进 runtime-options.json 的偏好目录，首次设置之前为空串。两个进程都从这里读，所以不会各自用一份。 */
    static String preferencesDirectory(Context context) {
        File files = context.getFilesDir();
        if (files == null) return "";
        return HostOptionsPolicy.readOption(files, "preferences_directory");
    }
}
