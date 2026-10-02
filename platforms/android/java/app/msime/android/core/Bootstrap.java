package app.msime.android;

import android.content.Context;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import org.json.JSONArray;
import org.json.JSONObject;

/** First-install preparation only. Existing configurations are never re-prepared from here; the optional offline glosses, helpcode tables and language dictionaries follow the installed package, and an existing configuration is only refreshed so that it names the language dictionaries installed beside its resources. */
public final class Bootstrap {
    private Bootstrap() {}
    public static boolean prepare(Context context) throws Exception {
        File root = context.getFilesDir();
        try (FileChannel channel = openLock(new File(root, "bootstrap.lock").toPath());
             FileLock lock = channel.lock()) {
            if (!lock.isValid()) throw new IllegalStateException("Bootstrap lock unavailable");
            installOfflineGlosses(context, new File(root, "bootstrap/offline-glosses"));
            installHelpcodes(context, new File(root, "bootstrap/resources/helpcodes"));
            // Before the configuration exists, so that prepare_host below finds them beside the resources and records them.
            installLanguageDictionaries(context, new File(root, "bootstrap/language-dictionaries"));
            File configuration = new File(root, "runtime-options.json");
            if (configuration.exists()) {
                refreshLanguageDictionaries(configuration);
                return false;
            }
            File resources = new File(root, "bootstrap/resources");
            ensureSafeDirectory(resources.toPath());
            JSONObject manifest;
            try (InputStream input = context.getAssets().open("desktop-dictionary.lock.json")) {
                // Small immutable APK manifest; large dictionary files are streamed below.
                java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
                byte[] buffer = new byte[8192];
                int count;
                while ((count = input.read(buffer)) != -1) {
                    if (bytes.size() + count > 16384) throw new IllegalArgumentException("Manifest too large");
                    bytes.write(buffer, 0, count);
                }
                manifest = new JSONObject(bytes.toString(StandardCharsets.UTF_8.name()));
            }
            JSONArray artifacts = manifest.getJSONArray("artifacts");
            for (int index = 0; index < artifacts.length(); index++) {
                String name = artifacts.getJSONObject(index).getString("name");
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("dictionary/" + name)) {
                    copyAsset(input, new File(resources, name).toPath());
                }
            }
            JSONObject request = new JSONObject().put("resources", resources.getAbsolutePath())
                .put("state_root", new File(root, "bootstrap/state").getAbsolutePath());
            JSONObject result = new JSONObject(NativeClient.prepareHost(request.toString()));
            if (!result.getBoolean("ok")) throw new IllegalStateException("Shared resource verification/preparation failed: " + result.optString("error"));
            AtomicFile destination = new AtomicFile(configuration);
            FileOutputStream output = null;
            try {
                output = destination.startWrite();
                output.write(result.getJSONObject("value").toString().getBytes(StandardCharsets.UTF_8));
                destination.finishWrite(output);
            } catch (Exception error) {
                if (output != null) destination.failWrite(output);
                throw error;
            }
            return true;
        }
    }

    static FileChannel openLock(java.nio.file.Path path) throws java.io.IOException {
        if (Files.isSymbolicLink(path)
                || (Files.exists(path, LinkOption.NOFOLLOW_LINKS)
                    && !Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)))
            throw new java.io.IOException("Bootstrap lock is not a regular file");
        return FileChannel.open(path, StandardOpenOption.CREATE, StandardOpenOption.WRITE);
    }

    static void ensureSafeDirectory(java.nio.file.Path directory) throws java.io.IOException {
        java.nio.file.Path absolute = directory.toAbsolutePath().normalize();
        java.nio.file.Path current = absolute.getRoot();
        if (current == null) throw new java.io.IOException("bootstrap directory unavailable");
        for (java.nio.file.Path component : absolute) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current))
                throw new java.io.IOException("bootstrap path contains a symbolic link");
        }
        if (Files.exists(absolute, LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new java.io.IOException("bootstrap directory unavailable");
        Files.createDirectories(absolute);
        for (java.nio.file.Path component : absolute) {
            current = current.getRoot().resolve(component);
            if (Files.isSymbolicLink(current))
                throw new java.io.IOException("bootstrap path contains a symbolic link");
        }
        if (!Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new java.io.IOException("bootstrap directory unavailable");
    }

    /**
     * Non-English candidate glosses (scripts/build_offline_glosses.py), extracted beside the resources where the Engine looks for one zh-&lt;lang&gt;.db per target language.
     *
     * <p>Unlike the dictionary they are not part of the verified configuration, so they follow the installed package: an update replaces them, and a package built without them removes any an earlier one left. A failure leaves the keyboard glossing in English only, never without an Engine.
     */
    private static void installOfflineGlosses(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "offline-glosses.staging");
            ensureSafeDirectory(destination.getParentFile().toPath());
            deleteTree(staging);
            ensureSafeDirectory(staging.toPath());
            String[] names = context.getAssets().list("offline-glosses");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("offline-glosses/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Offline gloss extraction failed", error);
        }
    }

    /**
     * The helpcode tables (resources/helpcodes), extracted into helpcodes/ under the resource directory, where the Engine reads them.
     *
     * <p>Like the offline glosses they are not part of the verified dictionary (the shared verification lets a real helpcodes/ directory through), so they follow the installed package: an install prepared before they shipped gets them on the first start after the update, although its configuration is never rewritten. Each packaged file is replaced on its own through a temporary sibling and an atomic rename, so a table being read is never half written and the user's own tables under helpcodes/custom are left alone. A failure leaves helpcode input narrowing nothing, never the keyboard without an Engine.
     */
    private static void installHelpcodes(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            ensureSafeDirectory(destination.toPath());
            String[] names = context.getAssets().list("helpcodes");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("helpcodes/" + name)) {
                    copyAsset(input, new File(destination, name).toPath());
                }
            }
            writeAtomically(marker.toPath(), stamp.getBytes(StandardCharsets.UTF_8));
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Helpcode table extraction failed", error);
        }
    }

    /**
     * The Cantonese and Zhuyin dictionaries (scripts/fetch_language_dictionaries.py), extracted to language-dictionaries/ beside the resources, where host-api looks for `cantonese.db` and `zhuyin.db` and names the directory in the runtime options.
     *
     * <p>Like the offline glosses they are not part of the verified dictionary, so they follow the installed package: an update replaces them, and a package built without them removes any an earlier one left, which takes those schemes off the keyboard once the configuration is refreshed. The directory is swapped whole through a staging sibling and an atomic rename. A failure leaves Cantonese and Zhuyin unavailable, never the keyboard without an Engine.
     */
    private static void installLanguageDictionaries(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "language-dictionaries.staging");
            ensureSafeDirectory(destination.getParentFile().toPath());
            deleteTree(staging);
            ensureSafeDirectory(staging.toPath());
            String[] names = context.getAssets().list("language-dictionaries");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("language-dictionaries/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Language dictionary extraction failed", error);
        }
    }

    /**
     * Brings `language_dictionaries` in an existing configuration in step with the dictionaries installed beside its resources, through the shared `msime_client_refresh_host`. The input method and this preparation run from the same package, so no older host is left reading a key it does not know.
     *
     * <p>The same call also re-prepares a configuration whose working dictionaries belong to an older resource generation. This host never replaces the resources of an existing configuration, so that step reports the resources outdated and leaves the file as it was, and the language dictionaries are then not recorded either; the keyboard keeps offering only the schemes it can run. A failure is logged without the path.
     */
    private static void refreshLanguageDictionaries(File configuration) {
        try {
            JSONObject result = new JSONObject(NativeClient.refreshHost(configuration.getAbsolutePath()));
            if (!result.optBoolean("ok")) {
                android.util.Log.w("MSIMEBootstrap", "Runtime options refresh failed: "
                    + (result.optString("error").startsWith("dictionary_outdated") ? "dictionary_outdated" : "error"));
            }
        } catch (Exception | LinkageError error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Runtime options refresh failed", error);
        }
    }

    static String readMarker(java.nio.file.Path file) {
        if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)) return null;
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(64);
            byte[] buffer = new byte[64];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() > 64 - count) return null;
                bytes.write(buffer, 0, count);
            }
            return bytes.toString(StandardCharsets.UTF_8.name());
        } catch (Exception ignored) {
            return null;
        }
    }

    /** Copy through a newly-created sibling so an existing destination link is replaced, never followed. */
    static void copyAsset(InputStream input, java.nio.file.Path destination) throws IOException {
        java.nio.file.Path parent = destination.getParent();
        if (parent == null) throw new IOException("asset destination unavailable");
        java.nio.file.Path temporary = Files.createTempFile(parent,
            "." + destination.getFileName() + ".", ".staging");
        try {
            Files.copy(input, temporary, StandardCopyOption.REPLACE_EXISTING);
            Files.move(temporary, destination, StandardCopyOption.REPLACE_EXISTING,
                StandardCopyOption.ATOMIC_MOVE);
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    static void writeAtomically(java.nio.file.Path destination, byte[] bytes) throws IOException {
        try (InputStream input = new java.io.ByteArrayInputStream(bytes)) {
            copyAsset(input, destination);
        }
    }

    private static void deleteTree(File file) throws java.io.IOException {
        java.nio.file.Path path = file.toPath();
        if (Files.isSymbolicLink(path)) {
            Files.deleteIfExists(path);
            return;
        }
        if (Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS)) {
            try (java.nio.file.DirectoryStream<java.nio.file.Path> children = Files.newDirectoryStream(path)) {
                for (java.nio.file.Path child : children) deleteTree(child.toFile());
            }
        }
        Files.deleteIfExists(path);
    }
}
