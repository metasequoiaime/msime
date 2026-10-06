package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 社区皮肤目录在本机的一份缓存，供键盘的皮肤面板列出全部社区皮肤。
 *
 * <p>键盘跑在 :ime 进程里，按宿主约定不为浏览目录联网；App 进程在拉社区皮肤列表时把条目写进偏好目录下的这个文件（两个进程同一 UID，读同一目录），键盘只读它。条目只存展示和安装要用的 id、名字、作者和设计；用户在键盘里选中一款未获取的皮肤时，再把设计存进 {@link CustomSkinLibrary}。
 */
public final class CommunitySkinCache {
    static final String FILE = "community-skins.json";
    private static final int MAX_ENTRIES = 200;
    private static final long MAX_BYTES = 4_000_000;

    /** 一款社区皮肤。 */
    public record Entry(String id, String name, String author, JSONObject design) {}

    private CommunitySkinCache() {}

    /** 原子地整份替换缓存；超过上限的条目和整份过大的内容不写。 */
    public static void write(Path preferencesDirectory, List<Entry> entries) throws IOException {
        JSONArray array = new JSONArray();
        try {
            for (Entry entry : entries) {
                if (array.length() >= MAX_ENTRIES) break;
                JSONObject value = new JSONObject();
                value.put("id", entry.id());
                value.put("name", entry.name());
                value.put("author", entry.author());
                value.put("design", entry.design());
                array.put(value);
            }
        } catch (JSONException error) {
            throw new IOException(error);
        }
        byte[] bytes = array.toString().getBytes(StandardCharsets.UTF_8);
        if (bytes.length > MAX_BYTES) return;
        Files.createDirectories(preferencesDirectory);
        // 每次写各用一个临时文件：社区页可能同时跑两次缓存（重建页面时），共用一个固定的 .pending 会互相截断，:ime 读到半截 JSON 就当作没有缓存。
        Path pending = Files.createTempFile(preferencesDirectory, FILE + ".", ".pending");
        try {
            Files.write(pending, bytes);
            Files.move(pending, preferencesDirectory.resolve(FILE),
                StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        } finally {
            Files.deleteIfExists(pending);
        }
    }

    /** 读缓存；文件缺失、过大或损坏时当作没有缓存。这是跨进程的文件边界，不能让一份坏文件把皮肤面板拖垮。 */
    public static List<Entry> read(Path preferencesDirectory) {
        Path file = preferencesDirectory.resolve(FILE);
        try {
            if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS) || Files.size(file) > MAX_BYTES) {
                return List.of();
            }
            byte[] bytes;
            try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS)) {
                bytes = HttpBodyPolicy.readBounded(input, (int) MAX_BYTES);
                if (bytes == null) return List.of();
            }
            JSONArray array = new JSONArray(new String(bytes, StandardCharsets.UTF_8));
            List<Entry> entries = new ArrayList<>(MAX_ENTRIES);
            for (int index = 0; index < array.length() && entries.size() < MAX_ENTRIES; index++) {
                JSONObject value = array.optJSONObject(index);
                if (value == null) continue;
                String id = text(value, "id");
                String name = text(value, "name");
                JSONObject design = value.isNull("design") ? null : value.optJSONObject("design");
                if (id.isEmpty() || name.isEmpty() || design == null) continue;
                entries.add(new Entry(id, name, text(value, "author"), design));
            }
            return entries;
        } catch (IOException | JSONException | RuntimeException error) {
            return List.of();
        }
    }

    private static String text(JSONObject value, String key) {
        return value.isNull(key) ? "" : value.optString(key, "");
    }
}
