package app.msime.android.home;

import android.content.Context;
import android.net.Uri;
import androidx.annotation.Nullable;
import app.msime.android.AccountStore;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.NativeClient;
import app.msime.android.NumberPolicy;
import app.msime.android.RustAccount;
import app.msime.android.policy.HostOptionsPolicy;
import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginEditorPolicy;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 插件页读写插件目录、偏好和社区插件的那一半：全部经共享 Rust，`msime_client_plugins`（目录、导入、删除、编辑器的 `authoring_load` / `authoring_save`）和 `msime_client_community` 的 plugin_* 操作（会话由 {@link RustAccount} 注册的 session_store 提供）。规则不在这里：包的校验在 client-services，页面上的说法在 {@link PluginCatalogPolicy} 和 {@link PluginEditorPolicy}。
 *
 * <p>插件目录是键盘读的那一份：runtime option `preferences_directory`（{@link HostStore#directory}）下的 `plugins/`，内置音效包在 `<filesDir>/sound-packs`，与 {@link KeySoundPacks} 和键盘的 `ImeKeyFeedback` 相同。不带 `script_page`，脚本插件永远不会出现。
 *
 * <p>每个调用都读写文件或阻塞网络，只能在工作线程上调用（{@link HostTask}）。
 */
final class PluginStore {
    private PluginStore() {}

    /** 目录里的一个包：页面用的字段和原始 JSON（各类型自己的内容，例如短语表的 `phrases`）。 */
    record Entry(PluginCatalogPolicy.Pack pack, JSONObject raw) {}

    /** 插件目录里一个载入不了的文件夹，以及原因。 */
    record Issue(String kind, String folder, String reason) {}

    /** 一次读目录的结果。 */
    record Catalog(List<Entry> entries, List<Issue> issues) {
        @Nullable Entry find(String kind, String id) {
            for (Entry entry : entries) {
                if (entry.pack().kind().equals(kind) && entry.pack().id().equals(id)) return entry;
            }
            return null;
        }
    }

    /** 一次操作的结果：成功时 `value` 非空（没有值的操作是 {@link Boolean#TRUE}），失败时是失败码和 `detail`。 */
    record Result<T>(@Nullable T value, String code, String detail) {
        static <T> Result<T> ok(T value) { return new Result<>(value, "", ""); }

        static <T> Result<T> failed(String code, String detail) {
            return new Result<>(null, code == null || code.isEmpty() ? "storage" : code, detail == null ? "" : detail);
        }

        boolean succeeded() { return value != null; }
    }

    // ---- 插件目录 ----

    /** 读插件目录；读不出来时为 null。 */
    @Nullable static Catalog catalog(Context context) {
        Result<JSONObject> response = plugins(context, operation(() -> operation("catalog")));
        if (!response.succeeded()) return null;
        JSONArray packages = response.value().optJSONArray("packages");
        if (packages == null) return null;
        List<Entry> entries = new ArrayList<>(packages.length());
        for (int index = 0; index < packages.length(); index++) {
            Entry entry = entry(packages.optJSONObject(index));
            if (entry != null) entries.add(entry);
        }
        List<Issue> issues = new ArrayList<>();
        JSONArray listed = response.value().optJSONArray("issues");
        for (int index = 0; listed != null && index < listed.length(); index++) {
            JSONObject issue = listed.optJSONObject(index);
            if (issue == null) continue;
            issues.add(new Issue(JsonPolicy.strictStringOrEmpty(issue.opt("kind")),
                JsonPolicy.strictStringOrEmpty(issue.opt("folder")), JsonPolicy.strictStringOrEmpty(issue.opt("reason"))));
        }
        return new Catalog(List.copyOf(entries), List.copyOf(issues));
    }

    /** `catalog` 或 `import` 回答里的一个包；缺 id 或类型时为 null。 */
    @Nullable static Entry entry(@Nullable JSONObject value) {
        if (value == null) return null;
        String kind = JsonPolicy.strictStringOrEmpty(value.opt("kind"));
        String id = JsonPolicy.strictStringOrEmpty(value.opt("id"));
        if (kind.isEmpty() || id.isEmpty()) return null;
        String name = JsonPolicy.strictStringOrEmpty(value.opt("name"));
        return new Entry(new PluginCatalogPolicy.Pack(kind, id, name.isEmpty() ? id : name,
            JsonPolicy.strictStringOrEmpty(value.opt("version")), JsonPolicy.strictStringOrEmpty(value.opt("license")),
            JsonPolicy.strictStringOrEmpty(value.opt("author")), JsonPolicy.strictStringOrEmpty(value.opt("description")),
            JsonPolicy.strictTrue(value.opt("builtin")), JsonPolicy.strictStringOrEmpty(value.opt("mode"))), value);
    }

    /**
     * 导入用户经系统文件选择器（SAF）选中的 .zip：选择器交回的是原生库打不开的 content URI，所以先复制进应用缓存下的私有暂存目录 `cache/plugin-import/<随机名>/pack.zip`，再由 `msime_client_plugins` 的 `import` 从那里安装，规则与其他宿主完全相同，拒绝时什么也不装。复制到 client-services 的压缩包上限（{@link PluginCatalogPolicy#MAX_ARCHIVE_BYTES}）就停，选错的大文件不会被整份复制；不论结果如何，暂存目录都在返回前删掉。与鸿蒙 `Settings.ets` 的 `importPlugin` 同一套做法；手机上不提供导入文件夹。
     */
    static Result<Entry> importArchive(Context context, Uri uri) {
        File cache = context.getCacheDir();
        if (cache == null) return Result.failed("storage", "");
        File root = new File(cache, "plugin-import");
        // 上一次导入中途被杀留下的暂存目录：导入只在这个进程的设置工作线程上排队进行，此时不会有别的导入在用它们。
        File[] leftovers = root.listFiles();
        if (leftovers != null) for (File leftover : leftovers) delete(leftover);
        File staging = new File(root, UUID.randomUUID().toString());
        try {
            if (!staging.mkdirs()) return Result.failed("storage", "");
            File archive = new File(staging, "pack.zip");
            try (InputStream input = context.getContentResolver().openInputStream(uri);
                 OutputStream output = new FileOutputStream(archive)) {
                if (input == null) return Result.failed("plugin_archive", "读不到选中的文件");
                byte[] buffer = new byte[64 * 1024];
                long total = 0;
                for (int read; (read = input.read(buffer)) > 0; ) {
                    total += read;
                    if (total > PluginCatalogPolicy.MAX_ARCHIVE_BYTES) return Result.failed("plugin_archive", "压缩包太大");
                    output.write(buffer, 0, read);
                }
            } catch (IOException | SecurityException error) {
                return Result.failed("plugin_archive", "读不到选中的文件");
            }
            Result<JSONObject> response = plugins(context, operation("import").put("source", archive.getAbsolutePath()));
            if (!response.succeeded()) return Result.failed(response.code(), response.detail());
            Entry entry = entry(response.value());
            return entry == null ? Result.failed("storage", "") : Result.ok(entry);
        } catch (JSONException error) {
            return Result.failed("storage", "");
        } finally {
            delete(staging);
        }
    }

    private static void delete(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) delete(child);
        if (file.exists() && !file.delete()) android.util.Log.w("MSIMEPlugins", "Plugin import staging was not removed");
    }

    /** 删除一个包；没装的包视为已删除。 */
    static Result<Boolean> remove(Context context, String kind, String id) {
        try {
            Result<JSONObject> response = plugins(context, operation("remove").put("kind", kind).put("id", id));
            return response.succeeded() ? Result.ok(Boolean.TRUE) : Result.failed(response.code(), response.detail());
        } catch (JSONException error) {
            return Result.failed("storage", "");
        }
    }

    /** 编辑器读出用户插件目录里的一个包。 */
    static Result<PluginEditorPolicy.AuthoredPack> load(Context context, String kind, String id) {
        try {
            Result<JSONObject> response = plugins(context, operation("authoring_load").put("kind", kind).put("id", id));
            if (!response.succeeded()) return Result.failed(response.code(), response.detail());
            PluginEditorPolicy.AuthoredPack pack = authoredPack(response.value());
            return pack == null ? Result.failed("plugin_not_editable", "") : Result.ok(pack);
        } catch (JSONException error) {
            return Result.failed("storage", "");
        }
    }

    /** 保存编辑器里的包：宿主按导入同样的规则校验并安装，回答装好的包。 */
    static Result<Entry> save(Context context, PluginEditorPolicy.AuthoredPack pack, boolean replace) {
        try {
            Result<JSONObject> response = plugins(context,
                operation("authoring_save").put("pack", authoredJson(pack)).put("replace", replace));
            if (!response.succeeded()) return Result.failed(response.code(), response.detail());
            Entry entry = entry(response.value());
            return entry == null ? Result.failed("storage", "") : Result.ok(entry);
        } catch (JSONException error) {
            return Result.failed("storage", "");
        }
    }

    private static JSONObject operation(String name) throws JSONException {
        return new JSONObject().put("operation", name);
    }

    /** 一次 `msime_client_plugins` 调用；回答的 value 不是对象（例如 `remove` 的 null）时换成空对象。 */
    private static Result<JSONObject> plugins(Context context, @Nullable JSONObject action) {
        if (action == null) return Result.failed("storage", "");
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return Result.failed("plugin_storage", "");
        try {
            File packs = HostOptionsPolicy.soundPacksDirectory(context.getFilesDir());
            JSONObject request = new JSONObject()
                .put("state_root", directory)
                .put("sound_packs", packs == null ? JSONObject.NULL : packs.getAbsolutePath())
                .put("action", action);
            JSONObject response = new JSONObject(NativeClient.plugins(request.toString()));
            if (!JsonPolicy.strictTrue(response.opt("ok"))) {
                return Result.failed(JsonPolicy.strictStringOrEmpty(response.opt("error")),
                    JsonPolicy.strictStringOrEmpty(response.opt("detail")));
            }
            JSONObject value = response.optJSONObject("value");
            return Result.ok(value == null ? new JSONObject() : value);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed("storage", "");
        }
    }

    /** `authoring_load` 的回答变成编辑器的包；形状不对时为 null。 */
    @Nullable static PluginEditorPolicy.AuthoredPack authoredPack(JSONObject value) {
        String kind = JsonPolicy.strictStringOrEmpty(value.opt("kind"));
        if (!PluginEditorPolicy.isAuthoredKind(kind)) return null;
        List<PluginEditorPolicy.Phrase> phrases = null;
        List<PluginEditorPolicy.Command> commands = null;
        List<PluginEditorPolicy.AuthoredGroup> groups = null;
        List<PluginEditorPolicy.AuthoredWord> words = null;
        JSONArray rows;
        switch (kind) {
            case PluginEditorPolicy.PHRASE_TABLE -> {
                phrases = new ArrayList<>();
                rows = value.optJSONArray("phrases");
                for (int index = 0; rows != null && index < rows.length(); index++) {
                    JSONObject row = rows.optJSONObject(index);
                    if (row != null) phrases.add(new PluginEditorPolicy.Phrase(text(row, "key"), text(row, "text")));
                }
            }
            case PluginEditorPolicy.COMMAND_TABLE -> {
                commands = new ArrayList<>();
                rows = value.optJSONArray("commands");
                for (int index = 0; rows != null && index < rows.length(); index++) {
                    JSONObject row = rows.optJSONObject(index);
                    if (row != null) {
                        commands.add(new PluginEditorPolicy.Command(text(row, "trigger"), text(row, "title"),
                            text(row, "template")));
                    }
                }
            }
            case PluginEditorPolicy.SYMBOL_SET -> {
                groups = new ArrayList<>();
                rows = value.optJSONArray("groups");
                for (int index = 0; rows != null && index < rows.length(); index++) {
                    JSONObject row = rows.optJSONObject(index);
                    if (row == null) continue;
                    List<String> items = new ArrayList<>();
                    JSONArray listed = row.optJSONArray("items");
                    for (int item = 0; listed != null && item < listed.length(); item++) {
                        String text = JsonPolicy.strictString(listed.opt(item));
                        if (text != null) items.add(text);
                    }
                    groups.add(new PluginEditorPolicy.AuthoredGroup(text(row, "tab"), text(row, "title"),
                        JsonPolicy.strictString(row.opt("keywords")), items));
                }
            }
            default -> {
                words = new ArrayList<>();
                rows = value.optJSONArray("words");
                for (int index = 0; rows != null && index < rows.length(); index++) {
                    JSONObject row = rows.optJSONObject(index);
                    if (row != null) {
                        words.add(new PluginEditorPolicy.AuthoredWord(text(row, "word"),
                            JsonPolicy.strictString(row.opt("phonetic")), text(row, "gloss")));
                    }
                }
            }
        }
        return new PluginEditorPolicy.AuthoredPack(kind, JsonPolicy.strictString(value.opt("id")), text(value, "name"),
            text(value, "version"), text(value, "license"), JsonPolicy.strictString(value.opt("author")),
            JsonPolicy.strictString(value.opt("description")), phrases, commands, groups, words);
    }

    private static String text(JSONObject value, String key) {
        return JsonPolicy.strictStringOrEmpty(value.opt(key));
    }

    /** 编辑器的包写成 `authoring_save` 的 `pack`：通用字段加与 `kind` 对应的恰好一个内容字段。 */
    static JSONObject authoredJson(PluginEditorPolicy.AuthoredPack pack) throws JSONException {
        JSONObject value = new JSONObject()
            .put("kind", pack.kind())
            .put("id", pack.id() == null ? JSONObject.NULL : pack.id())
            .put("name", pack.name())
            .put("version", pack.version())
            .put("license", pack.license())
            .put("author", pack.author() == null ? JSONObject.NULL : pack.author())
            .put("description", pack.description() == null ? JSONObject.NULL : pack.description());
        if (pack.phrases() != null) {
            JSONArray rows = new JSONArray();
            for (PluginEditorPolicy.Phrase row : pack.phrases()) rows.put(new JSONObject().put("key", row.key()).put("text", row.text()));
            value.put("phrases", rows);
        } else if (pack.commands() != null) {
            JSONArray rows = new JSONArray();
            for (PluginEditorPolicy.Command row : pack.commands()) {
                rows.put(new JSONObject().put("trigger", row.trigger()).put("title", row.title()).put("template", row.template()));
            }
            value.put("commands", rows);
        } else if (pack.groups() != null) {
            JSONArray rows = new JSONArray();
            for (PluginEditorPolicy.AuthoredGroup group : pack.groups()) {
                rows.put(new JSONObject().put("tab", group.tab()).put("title", group.title())
                    .put("keywords", group.keywords() == null ? JSONObject.NULL : group.keywords())
                    .put("items", new JSONArray(group.items())));
            }
            value.put("groups", rows);
        } else if (pack.words() != null) {
            JSONArray rows = new JSONArray();
            for (PluginEditorPolicy.AuthoredWord word : pack.words()) {
                rows.put(new JSONObject().put("word", word.word())
                    .put("phonetic", word.phonetic() == null ? JSONObject.NULL : word.phonetic()).put("gloss", word.gloss()));
            }
            value.put("words", rows);
        }
        return value;
    }

    // ---- 偏好 ----

    /** 偏好里与插件有关的选择，加上本机的按键音开关。`preferences` 是偏好文档的 `preferences` 对象。 */
    static PluginCatalogPolicy.Selection selection(Context context, JSONObject preferences) {
        JSONObject plugins = preferences.optJSONObject("plugins");
        JSONObject keySound = plugins == null ? null : plugins.optJSONObject("key_sound");
        String pack = keySound == null ? "" : JsonPolicy.strictStringOrEmpty(keySound.opt("pack"));
        JSONObject modes = preferences.optJSONObject("local_modes");
        return new PluginCatalogPolicy.Selection(pack.isEmpty() ? PluginCatalogPolicy.DEFAULT_SOUND_PACK : pack,
            ids(plugins, "phrase_tables"), ids(plugins, "command_tables"),
            plugins == null ? "" : JsonPolicy.strictStringOrEmpty(plugins.opt("helpcode_pack_quanpin")),
            plugins == null ? "" : JsonPolicy.strictStringOrEmpty(plugins.opt("helpcode_pack_shuangpin")),
            KeyboardFeedbackStore.load(context).soundEnabled(),
            modes == null || JsonPolicy.strictBoolean(modes.opt("quick_phrase"), true),
            modes != null && JsonPolicy.strictBoolean(modes.opt("command"), false));
    }

    private static List<String> ids(@Nullable JSONObject plugins, String key) {
        JSONArray values = plugins == null ? null : plugins.optJSONArray(key);
        List<String> ids = new ArrayList<>();
        for (int index = 0; values != null && index < values.length(); index++) {
            String id = JsonPolicy.strictString(values.opt(index));
            if (id != null && !id.isEmpty()) ids.add(id);
        }
        return List.copyOf(ids);
    }

    /** 写入短语表或指令表的启用顺序；空列表时去掉这个键，与共享页「为空时文档里不写」一致。 */
    static void putTables(JSONObject preferences, String kind, List<String> tables) throws JSONException {
        String key = PluginCatalogPolicy.PHRASE_TABLE.equals(kind) ? "phrase_tables" : "command_tables";
        JSONObject plugins = KeyboardSheets.child(preferences, "plugins");
        if (tables.isEmpty()) plugins.remove(key);
        else plugins.put(key, new JSONArray(tables));
    }

    /** 包从磁盘删除之后的偏好：指着它的选择回到默认，启用它的短语表、指令表把它移除；与共享页 `withoutRemovedPack` 相同。 */
    static void withoutRemovedPack(JSONObject preferences, String kind, String id) throws JSONException {
        JSONObject plugins = preferences.optJSONObject("plugins");
        if (plugins == null) return;
        switch (kind) {
            case PluginCatalogPolicy.PHRASE_TABLE, PluginCatalogPolicy.COMMAND_TABLE ->
                putTables(preferences, kind, PluginCatalogPolicy.withTable(
                    ids(plugins, PluginCatalogPolicy.PHRASE_TABLE.equals(kind) ? "phrase_tables" : "command_tables"),
                    id, false));
            case PluginCatalogPolicy.HELPCODE -> {
                for (String key : HelpcodeChoicePolicy.PACK_KEYS) {
                    if (id.equals(plugins.optString(key, ""))) plugins.put(key, "");
                }
            }
            case PluginCatalogPolicy.SOUND -> {
                JSONObject keySound = plugins.optJSONObject("key_sound");
                if (keySound != null && id.equals(keySound.optString("pack", ""))) {
                    keySound.put("pack", PluginCatalogPolicy.DEFAULT_SOUND_PACK);
                }
                JSONObject melody = plugins.optJSONObject("melody");
                if (melody != null && id.equals(melody.optString("pack", ""))) {
                    melody.put("pack", PluginCatalogPolicy.DEFAULT_MELODY_PACK);
                }
            }
            default -> { }
        }
    }

    /** 选一个按键音效包：写共享偏好的包，并打开本机的按键音开关（「设为当前音效包」就是要它响）。按键音的开关在 Android 是本机设置，不改共享偏好的 `key_sound.enabled`，免得同步到电脑上把那边的按键音也打开。 */
    static boolean selectKeySound(Context context, String id) {
        JSONObject saved = KeyboardSheets.write(context, preferences ->
            KeyboardSheets.child(KeyboardSheets.child(preferences, "plugins"), "key_sound").put("pack", id));
        if (saved == null) return false;
        KeyboardFeedbackStore.Settings settings = KeyboardFeedbackStore.load(context);
        if (settings.soundEnabled()) return true;
        try {
            KeyboardFeedbackStore.save(context, new KeyboardFeedbackStore.Settings(true, settings.hapticsEnabled(),
                settings.hapticStrength()));
            app.msime.android.SyncSignals.markDirty(context, app.msime.android.SyncSwitch.SETTINGS);
            return true;
        } catch (IOException error) {
            return false;
        }
    }

    // ---- 社区插件 ----

    /** 社区里的一个插件：`client-services::plugins::community::CommunityPlugin` 里页面要用的字段。 */
    record CommunityPlugin(String id, String kind, String pluginId, String name, String description, String author,
            String version, String license, long size, long downloads, long ratingCount, double ratingAverage,
            boolean owned, int myRating, String moderation) {}

    /** 一页社区插件：这台设备用不上的类型已经去掉，数在 `skipped` 里，「加载更多」按它往后翻。 */
    record CommunityPage(List<CommunityPlugin> plugins, boolean hasMore, int skipped) {}

    static Result<CommunityPage> communityList(Context context, int offset, String search, @Nullable String kind,
            boolean mine) {
        try {
            JSONObject action = operation("plugin_list").put("offset", offset).put("search", search).put("mine", mine);
            if (kind != null) action.put("kind", kind);
            Object value = RustAccount.community(context, action, null, false);
            if (!(value instanceof JSONObject page)) return Result.failed("community_unavailable", "");
            JSONArray listed = page.optJSONArray("plugins");
            if (listed == null) return Result.failed("community_unavailable", "");
            List<CommunityPlugin> plugins = new ArrayList<>(listed.length());
            int skipped = NumberPolicy.strictInt(page.opt("skipped"), 0);
            for (int index = 0; index < listed.length(); index++) {
                CommunityPlugin plugin = communityPlugin(listed.optJSONObject(index));
                if (plugin != null && PluginCatalogPolicy.communityKindShown(plugin.kind())) plugins.add(plugin);
                else skipped++;
            }
            return Result.ok(new CommunityPage(List.copyOf(plugins), JsonPolicy.strictTrue(page.opt("has_more")), skipped));
        } catch (AccountStore.Failure failure) {
            return Result.failed(failure.code, failure.detail);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed("community_unavailable", "");
        }
    }

    static Result<CommunityPlugin> communityDetail(Context context, String id) {
        return communityItem(context, operation(() -> new JSONObject().put("operation", "plugin_detail").put("id", id)), null);
    }

    /** 下载、校验并安装一个发布，整包替换已装的同类型同 id 的包（页面先问过用户）。要登录。 */
    static Result<Entry> communityInstall(Context context, CommunityPlugin plugin) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return Result.failed("plugin_storage", "");
        try {
            Object value = RustAccount.community(context, operation("plugin_install").put("id", plugin.id())
                .put("kind", plugin.kind()).put("plugin_id", plugin.pluginId()), directory, false);
            Entry entry = value instanceof JSONObject object ? entry(object) : null;
            return entry == null ? Result.failed("storage", "") : Result.ok(entry);
        } catch (AccountStore.Failure failure) {
            return Result.failed(failure.code, failure.detail);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed("community_unavailable", "");
        }
    }

    static Result<Boolean> communityRate(Context context, String id, int stars) {
        return communityAction(context, () -> operation("plugin_rate").put("id", id).put("stars", stars), false);
    }

    /** 下架自己的一个发布，本机已装的包保留。 */
    static Result<Boolean> communityDelete(Context context, String id) {
        return communityAction(context, () -> operation("plugin_delete").put("id", id), false);
    }

    /** 举报别人的一个发布：登录账号优先，否则用本机的匿名账号。 */
    static Result<Boolean> communityReport(Context context, String id, String reason, String detail) {
        return communityAction(context, () -> {
            JSONObject action = operation("report").put("kind", "plugins").put("id", id).put("reason", reason);
            if (!detail.isEmpty()) action.put("detail", detail);
            return action;
        }, true);
    }

    /** 发布前的预检：按分享规则检查一个已装的包并算出归档大小，不上传。 */
    record PackPreview(String suggestedName, String suggestedDescription, String version, String license, int fileCount,
            long size) {}

    static Result<PackPreview> packPreview(Context context, String kind, String pluginId) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return Result.failed("plugin_storage", "");
        try {
            Object value = RustAccount.community(context, operation("plugin_pack_preview").put("kind", kind)
                .put("plugin_id", pluginId), directory, false);
            if (!(value instanceof JSONObject preview)) return Result.failed("storage", "");
            return Result.ok(new PackPreview(text(preview, "suggestedName"), text(preview, "suggestedDescription"),
                text(preview, "version"), text(preview, "license"), NumberPolicy.strictInt(preview.opt("fileCount"), 0),
                NumberPolicy.strictLong(preview.opt("size"), 0)));
        } catch (AccountStore.Failure failure) {
            return Result.failed(failure.code, failure.detail);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed("storage", "");
        }
    }

    /** 把一个已装的包打包并以 `publication` 发布；`publication` 由页面生成，失败重试时沿用，服务端据此去重。要登录。 */
    static Result<CommunityPlugin> publish(Context context, String kind, String pluginId, String publication, String name,
            String description) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return Result.failed("plugin_storage", "");
        return communityItem(context, operation(() -> new JSONObject().put("operation", "plugin_publish").put("kind", kind)
            .put("plugin_id", pluginId).put("id", publication).put("name", name).put("description", description)), directory);
    }

    private interface ActionBuilder {
        JSONObject build() throws JSONException;
    }

    @Nullable private static JSONObject operation(ActionBuilder builder) {
        try {
            return builder.build();
        } catch (JSONException error) {
            return null;
        }
    }

    private static Result<CommunityPlugin> communityItem(Context context, @Nullable JSONObject action,
            @Nullable String stateRoot) {
        if (action == null) return Result.failed("community_invalid", "");
        try {
            Object value = RustAccount.community(context, action, stateRoot, false);
            CommunityPlugin plugin = value instanceof JSONObject object ? communityPlugin(object) : null;
            return plugin == null ? Result.failed("community_unavailable", "") : Result.ok(plugin);
        } catch (AccountStore.Failure failure) {
            return Result.failed(failure.code, failure.detail);
        } catch (RuntimeException | LinkageError error) {
            return Result.failed("community_unavailable", "");
        }
    }

    private static Result<Boolean> communityAction(Context context, ActionBuilder builder, boolean anonymousFallback) {
        try {
            RustAccount.community(context, builder.build(), null, anonymousFallback);
            return Result.ok(Boolean.TRUE);
        } catch (AccountStore.Failure failure) {
            return Result.failed(failure.code, failure.detail);
        } catch (JSONException | RuntimeException | LinkageError error) {
            return Result.failed("community_unavailable", "");
        }
    }

    @Nullable private static CommunityPlugin communityPlugin(@Nullable JSONObject value) {
        if (value == null) return null;
        String id = JsonPolicy.strictStringOrEmpty(value.opt("id"));
        String kind = JsonPolicy.strictStringOrEmpty(value.opt("kind"));
        String pluginId = JsonPolicy.strictStringOrEmpty(value.opt("plugin_id"));
        String name = JsonPolicy.strictStringOrEmpty(value.opt("name"));
        if (!JsonPolicy.nonNilUuid(id) || kind.isEmpty() || pluginId.isEmpty() || name.isEmpty()) return null;
        double average = NumberPolicy.strictDouble(value.opt("rating_average"), 0);
        return new CommunityPlugin(id, kind, pluginId, name, text(value, "description"), text(value, "author"),
            text(value, "version"), text(value, "license"), JsonPolicy.nonNegativeLong(value.opt("size"), 0),
            JsonPolicy.nonNegativeLong(value.opt("downloads"), 0), JsonPolicy.nonNegativeLong(value.opt("rating_count"), 0),
            Double.isFinite(average) ? average : 0, JsonPolicy.strictTrue(value.opt("owned")),
            NumberPolicy.strictInt(value.opt("my_rating"), 0), text(value, "moderation"));
    }
}
