package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.text.BreakIterator;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONObject;

/** The named custom skins shared by the Android settings surface and keyboard host. */
public final class CustomSkinLibrary {
    private static final long MAX_LIBRARY_BYTES = 9_000_000;
    private static final int MAX_DESIGNS = 12;
    private static final int MAX_NAME_LENGTH = 32;
    /** 本进程内的写入排队用；跨进程（设置主进程与 `:ime`）靠库目录里的锁文件。 */
    private static final Object UPDATE_LOCK = new Object();

    private CustomSkinLibrary() {}

    /** 加锁期间要做的一次「读 - 改 - 写」。 */
    private interface Update<T> {
        T apply() throws IOException;
    }

    /**
     * 一个命名设计。
     *
     * @param updatedAt 最近一次写入的时间（Unix 毫秒）；旧库没有这个字段时为 0
     */
    public record Item(String id, String name, JSONObject design, long updatedAt) {
        public Item(String id, String name, JSONObject design) {
            this(id, name, design, 0);
        }
    }

    public static List<Item> read(Path preferencesDirectory) throws IOException {
        Path root = checkedRoot(preferencesDirectory);
        if (!Files.exists(root, LinkOption.NOFOLLOW_LINKS)) return List.of();
        if (!Files.isDirectory(root, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("Invalid preferences directory");
        Path directory = root.resolve("CustomSkins");
        if (!Files.exists(directory, LinkOption.NOFOLLOW_LINKS)) return List.of();
        if (!Files.isDirectory(directory, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("Invalid custom skin directory");
        Path file = directory.resolve("library.json");
        if (!Files.exists(file, LinkOption.NOFOLLOW_LINKS))
            return List.of();
        if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("Invalid custom skin library");
        if (!SafePaths.isSingleLink(file))
            throw new IOException("Invalid custom skin library");
        String document;
        try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS)) {
            byte[] bytes = HttpBodyPolicy.readBounded(input, (int) MAX_LIBRARY_BYTES);
            if (bytes == null) return List.of();
            document = new String(bytes, StandardCharsets.UTF_8);
        }
        final JSONArray values;
        try {
            values = new JSONArray(document);
        } catch (org.json.JSONException error) {
            return List.of();
        }
        ArrayList<Item> result = new ArrayList<>(MAX_DESIGNS);
        for (int index = 0; index < values.length() && result.size() < MAX_DESIGNS; index++) {
            JSONObject item = values.optJSONObject(index);
            if (item == null) continue;
            String id = JsonPolicy.strictString(item.opt("id"));
            String name = JsonPolicy.strictString(item.opt("name"));
            JSONObject design = item.optJSONObject("design");
            if (id == null || name == null) continue;
            name = TextPolicy.trimmed(name);
            if (id.isEmpty() || !boundedName(name) || design == null) continue;
            long updatedAt = BoundsPolicy.nonNegative(KeyboardGeometry.strictLong(item.opt("updated_at"), 0));
            result.add(new Item(id, name, design, updatedAt));
        }
        return List.copyOf(result);
    }

    /**
     * Add or replace one named design, keeping the bounds {@link #read} enforces.
     *
     * <p>Replacing by id rather than appending: downloading the same skin twice is one entry that
     * got refreshed, not two entries with the same name. Written to a neighbouring file and moved
     * into place, so a library that is being read by the keyboard never sees a partial document.
     *
     * @return false when the library is already full, which is a limit rather than a failure
     */
    public static boolean add(Path preferencesDirectory, String id, String name, JSONObject design)
            throws IOException {
        return add(preferencesDirectory, id, name, design, System.currentTimeMillis());
    }

    /** {@link #add(Path, String, String, JSONObject)}，并显式给出写入时间（Unix 毫秒）。 */
    public static boolean add(Path preferencesDirectory, String id, String name, JSONObject design,
            long updatedAt) throws IOException {
        if (id == null || id.isEmpty() || name == null || design == null) return false;
        String bounded = TextPolicy.trimmed(name);
        if (!boundedName(bounded)) return false;
        Path root = checkedRoot(preferencesDirectory);
        ensureSafeDirectory(root);
        return locked(root, () -> {
            List<Item> existing = read(root);
            JSONArray values = new JSONArray();
            boolean replaced = false;
            for (Item item : existing) {
                if (item.id().equals(id)) {
                    values.put(entry(id, bounded, design, updatedAt));
                    replaced = true;
                } else {
                    values.put(entry(item.id(), item.name(), item.design(), item.updatedAt()));
                }
            }
            if (!replaced) {
                if (values.length() >= MAX_DESIGNS) return false;
                values.put(entry(id, bounded, design, updatedAt));
            }
            return write(root, values);
        });
    }

    /**
     * 在锁里做一次「读 - 改 - 写」：`:ime` 装社区皮肤和主进程云同步导入可能同时改库，不加锁时后写的一方会用自己读到的旧库整份覆盖，悄悄丢掉对方刚加的设计。
     *
     * <p>文件锁属于整个 JVM 而不是线程：同一进程里另一个线程已经持有时，`channel.lock()` 不会等待，而是抛 OverlappingFileLockException。所以先用 {@link #UPDATE_LOCK} 把本进程的写入排成一队，文件锁只负责协调两个进程，与 AndroidLocalSettings 的做法一致。读库不取锁：写入是整份原子替换，读者不会看到写了一半的文件。
     */
    private static <T> T locked(Path root, Update<T> update) throws IOException {
        synchronized (UPDATE_LOCK) {
            Path directory = root.resolve("CustomSkins");
            ensureSafeDirectory(directory);
            Path lockFile = directory.resolve("library.json.lock");
            try (FileChannel channel = FileChannel.open(lockFile, StandardOpenOption.CREATE,
                    StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS)) {
                FileLock lock = channel.lock();
                try {
                    return update.apply();
                } finally {
                    lock.release();
                }
            }
        }
    }

    /** 原子写入整个库；超过字节上限时不写并返回 false。 */
    private static boolean write(Path root, JSONArray values) throws IOException {
        byte[] document = TextPolicy.utf8Bytes(values.toString());
        if (document.length > MAX_LIBRARY_BYTES) return false;
        Path directory = root.resolve("CustomSkins");
        ensureSafeDirectory(directory);
        Path target = directory.resolve("library.json");
        if (Files.exists(target, LinkOption.NOFOLLOW_LINKS)
                && !Files.isRegularFile(target, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("Invalid custom skin library");
        // 固定的 pending 路径可以被同 UID 的另一个进程预先换成硬链接；普通文件检查无法
        // 区分这种链接，写入会截断目录外的 inode。每次使用新名字，不复用攻击者预先放置的目标。
        Path pending = Files.createTempFile(directory, "library-", ".pending");
        try {
            Files.write(pending, document, StandardOpenOption.TRUNCATE_EXISTING,
                StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS);
            Files.move(pending, target,
                StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            return true;
        } finally {
            Files.deleteIfExists(pending);
        }
    }

    private static Path checkedRoot(Path preferencesDirectory) throws IOException {
        if (preferencesDirectory == null) throw new IOException("Invalid preferences directory");
        SafePaths.rejectSymlinkComponents(preferencesDirectory);
        return preferencesDirectory.toAbsolutePath().normalize();
    }

    static void ensureSafeDirectory(Path directory) throws IOException {
        if (directory == null) throw new IOException("Invalid custom skin directory");
        SafePaths.ensureDirectory(directory);
    }

    /** org.json's optString coerces numbers and booleans; persisted library fields are strings. */
    static String strictString(Object value) {
        return JsonPolicy.strictString(value);
    }

    // ---- 设计参数的导出与导入（云同步与分享用；不含照片） ----

    /** 导出库里全部设计的设计参数，见 {@link #exportDesigns(List, long)}。 */
    public static String exportDesigns(Path preferencesDirectory) throws IOException {
        return exportDesigns(read(preferencesDirectory), Long.MAX_VALUE);
    }

    /** 导出库里的设计参数，编码后的 UTF-8 字节数不超过 `maxBytes`。 */
    public static String exportDesigns(Path preferencesDirectory, long maxBytes)
            throws IOException {
        return exportDesigns(read(preferencesDirectory), maxBytes);
    }

    /**
     * 把设计导出成 JSON 数组：每项 `{id, name, updated_at, design}`，`design` 经 {@link CustomKeyboardSkin} 规整、只含设计参数，不含照片。
     *
     * <p>按列表顺序依次放入，放下某一项会让整个数组编码后的 UTF-8 字节数超过 `maxBytes` 时就停下，后面的不再导出；一项都放不下时是 `[]`。
     */
    public static String exportDesigns(List<Item> items, long maxBytes) {
        JSONArray values = new JSONArray();
        long used = 2;
        for (Item item : items) {
            JSONObject exported = entry(item.id(), item.name(),
                CustomKeyboardSkin.from(item.design()).toJson(false), item.updatedAt());
            long size = TextPolicy.utf8Bytes(exported.toString()).length
                + (values.length() == 0 ? 0 : 1);
            if (used + size > maxBytes) break;
            values.put(exported);
            used += size;
        }
        return values.toString();
    }

    /**
     * 把导出的设计并入库里并写盘。
     *
     * @return 新增或更新的设计数；JSON 不是数组时为 0、库不变
     */
    public static int importDesigns(Path preferencesDirectory, String json) throws IOException {
        Path root = checkedRoot(preferencesDirectory);
        ensureSafeDirectory(root);
        return locked(root, () -> {
            List<Item> existing = read(root);
            Merge merge = mergeDesigns(existing, json);
            if (merge.changed() == 0) return 0;
            JSONArray values = new JSONArray();
            for (Item item : merge.items())
                values.put(entry(item.id(), item.name(), item.design(), item.updatedAt()));
            return write(root, values) ? merge.changed() : 0;
        });
    }

    /** {@link #mergeDesigns} 的结果：合并后的库与新增或更新的条目数。 */
    public record Merge(List<Item> items, int changed) {}

    /**
     * 按 id 合并：库里没有的 id 追加（库满 12 个后不再追加），已有的 id 只在导入项的 `updated_at` 更新时才替换，相同或更旧时保留库里的。替换时如果库里的设计带照片而导入项没有（导出不含照片），保留原照片。名字和设计不合法的导入项跳过。
     */
    public static Merge mergeDesigns(List<Item> existing, String json) {
        final JSONArray values;
        try {
            values = new JSONArray(json == null ? "" : json);
        } catch (org.json.JSONException error) {
            return new Merge(List.copyOf(existing), 0);
        }
        ArrayList<Item> result = new ArrayList<>(existing);
        int changed = 0;
        for (int index = 0; index < values.length(); index++) {
            JSONObject value = values.optJSONObject(index);
            if (value == null) continue;
            String id = JsonPolicy.strictString(value.opt("id"));
            String name = JsonPolicy.strictString(value.opt("name"));
            JSONObject design = value.optJSONObject("design");
            if (id == null || id.isEmpty() || name == null || design == null) continue;
            name = TextPolicy.trimmed(name);
            if (!boundedName(name)) continue;
            long updatedAt = BoundsPolicy.nonNegative(KeyboardGeometry.strictLong(value.opt("updated_at"), 0));
            int position = indexOf(result, id);
            if (position >= 0) {
                Item current = result.get(position);
                if (updatedAt <= current.updatedAt()) continue;
                result.set(position, new Item(id, name,
                    importedDesign(design, current.design()), updatedAt));
                changed++;
            } else if (result.size() < MAX_DESIGNS) {
                result.add(new Item(id, name, importedDesign(design, null), updatedAt));
                changed++;
            }
        }
        return new Merge(List.copyOf(result), changed);
    }

    private static JSONObject importedDesign(JSONObject incoming, JSONObject current) {
        CustomKeyboardSkin skin = CustomKeyboardSkin.from(incoming);
        JSONObject design = skin.toJson(true);
        if (!skin.hasPhoto() && current != null && current.has("photo")) {
            CustomKeyboardSkin previous = CustomKeyboardSkin.from(current);
            if (previous.hasPhoto()) {
                try {
                    design.put("photo", current.getString("photo"));
                } catch (org.json.JSONException error) {
                    // previous.hasPhoto() 已证明 photo 是可解码的字符串。
                    throw new IllegalStateException(error);
                }
            }
        }
        return design;
    }

    private static int indexOf(List<Item> items, String id) {
        for (int index = 0; index < items.size(); index++)
            if (items.get(index).id().equals(id)) return index;
        return -1;
    }

    private static JSONObject entry(String id, String name, JSONObject design, long updatedAt) {
        JSONObject item = entry(id, name, design);
        try {
            item.put("updated_at", updatedAt);
        } catch (org.json.JSONException error) {
            throw new IllegalStateException(error);
        }
        return item;
    }

    private static JSONObject entry(String id, String name, JSONObject design) {
        JSONObject item = new JSONObject();
        try {
            item.put("id", id).put("name", name).put("design", design);
        } catch (org.json.JSONException error) {
            // Three string-keyed puts of non-null values; org.json only throws for a null key.
            throw new IllegalStateException(error);
        }
        return item;
    }

    /** Match the shared Rust store's 32 extended-grapheme name bound. */
    private static boolean boundedName(String value) {
        if (value == null || value.isEmpty()) return false;
        BreakIterator iterator = BreakIterator.getCharacterInstance(Locale.ROOT);
        iterator.setText(value);
        int count = 0;
        iterator.first();
        int boundary;
        while ((boundary = iterator.next()) != BreakIterator.DONE) {
            if (!joinedByZeroWidthJoiner(value, boundary)
                    && ++count > MAX_NAME_LENGTH) return false;
        }
        return true;
    }

    /** Some JDK Unicode tables split an emoji ZWJ sequence at the joiner; Android ICU does not. */
    private static boolean joinedByZeroWidthJoiner(String value, int boundary) {
        int before = value.codePointBefore(boundary);
        int after = boundary < value.length() ? value.codePointAt(boundary) : -1;
        return before == 0x200D || after == 0x200D;
    }
}
