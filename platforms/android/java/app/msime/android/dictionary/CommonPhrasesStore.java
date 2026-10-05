package app.msime.android;

import android.content.Context;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
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

    private CommonPhrasesStore() {}

    public static Result load(Context context) {
        return perform(context, action("load"), false);
    }

    public static Result add(Context context, String text) {
        if (!validText(text)) return Result.failed(failureMessage("common_phrases_invalid"));
        try {
            return perform(context, action("add").put("text", text), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
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
            return perform(context, action("replace").put("id", id).put("text", text), true);
        } catch (JSONException error) {
            return Result.failed(failureMessage(""));
        }
    }

    /** 把一条移到第 `index` 位（从 0 数）。 */
    public static Result move(Context context, String id, int index) {
        try {
            return perform(context, action("move").put("id", id).put("index", Math.max(0, index)), true);
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
        List<Phrase> phrases = new ArrayList<>();
        JSONArray rawPhrases = value.optJSONArray("phrases");
        if (rawPhrases != null) {
            for (int index = 0; index < rawPhrases.length(); index++) {
                JSONObject phrase = rawPhrases.optJSONObject(index);
                if (phrase == null) continue;
                String pack = phrase.isNull("pack") ? "" : phrase.optString("pack", "");
                phrases.add(new Phrase(phrase.optString("id", ""), phrase.optString("text", ""), pack));
            }
        }
        List<Pack> packs = new ArrayList<>();
        JSONArray rawPacks = value.optJSONArray("packs");
        if (rawPacks != null) {
            for (int index = 0; index < rawPacks.length(); index++) {
                JSONObject pack = rawPacks.optJSONObject(index);
                if (pack == null) continue;
                packs.add(new Pack(pack.optString("id", ""), pack.optString("name", ""), pack.optInt("revision", 0)));
            }
        }
        return new Document(Collections.unmodifiableList(phrases), Collections.unmodifiableList(packs),
            value.optInt("skipped", 0));
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
            if (!root.optBoolean("ok", false)) return Result.failed(failureMessage(root.optString("error", "")));
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

    /** Bootstrap 写进 runtime-options.json 的偏好目录，首次设置之前为空串。两个进程都从这里读，所以不会各自用一份。 */
    static String preferencesDirectory(Context context) {
        File files = context.getFilesDir();
        if (files == null) return "";
        File options = new File(files, "runtime-options.json");
        if (!options.isFile()) return "";
        try {
            return new JSONObject(HostOptionsPolicy.read(options)).optString("preferences_directory", "");
        } catch (JSONException | IOException | SecurityException error) {
            return "";
        }
    }
}
