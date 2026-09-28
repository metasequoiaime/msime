package app.msime.client;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** The named custom skins shared by the Android settings surface and keyboard host. */
public final class CustomSkinLibrary {
    private static final long MAX_LIBRARY_BYTES = 1_048_576;
    private static final int MAX_DESIGNS = 12;
    private static final int MAX_NAME_LENGTH = 32;

    private CustomSkinLibrary() {}

    public record Item(String id, String name, JSONObject design) {
    }

    public static List<Item> read(Path preferencesDirectory) throws IOException {
        Path file = preferencesDirectory.resolve("CustomSkins").resolve("library.json");
        if (!Files.isRegularFile(file))
            return List.of();
        String document;
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream((int) MAX_LIBRARY_BYTES);
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() + count > MAX_LIBRARY_BYTES) return List.of();
                bytes.write(buffer, 0, count);
            }
            document = new String(bytes.toByteArray(), StandardCharsets.UTF_8);
        }
        final JSONArray values;
        try {
            values = new JSONArray(document);
        } catch (org.json.JSONException error) {
            return List.of();
        }
        ArrayList<Item> result = new ArrayList<>();
        for (int index = 0; index < values.length() && result.size() < MAX_DESIGNS; index++) {
            JSONObject item = values.optJSONObject(index);
            if (item == null) continue;
            String id = item.optString("id", "");
            String name = item.optString("name", "").trim();
            JSONObject design = item.optJSONObject("design");
            if (id.isEmpty() || name.isEmpty() || name.length() > MAX_NAME_LENGTH || design == null) continue;
            result.add(new Item(id, name, design));
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
        if (id == null || id.isEmpty() || name == null || design == null) return false;
        String bounded = name.trim();
        if (bounded.isEmpty() || bounded.length() > MAX_NAME_LENGTH) return false;
        List<Item> existing = read(preferencesDirectory);
        JSONArray values = new JSONArray();
        boolean replaced = false;
        for (Item item : existing) {
            if (item.id().equals(id)) {
                values.put(entry(id, bounded, design));
                replaced = true;
            } else {
                values.put(entry(item.id(), item.name(), item.design()));
            }
        }
        if (!replaced) {
            if (values.length() >= MAX_DESIGNS) return false;
            values.put(entry(id, bounded, design));
        }
        byte[] document = values.toString().getBytes(StandardCharsets.UTF_8);
        if (document.length > MAX_LIBRARY_BYTES) return false;
        Path directory = preferencesDirectory.resolve("CustomSkins");
        Files.createDirectories(directory);
        Path pending = directory.resolve("library.json.pending");
        Files.write(pending, document);
        Files.move(pending, directory.resolve("library.json"),
            StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        return true;
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
}
