package app.msime.client;

import android.content.Context;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.ByteArrayOutputStream;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import org.json.JSONArray;
import org.json.JSONObject;

/** First-install preparation only. Existing configurations are never upgraded in place; only the optional offline glosses follow the installed package. */
public final class Bootstrap {
    private Bootstrap() {}
    public static boolean prepare(Context context) throws Exception {
        File root = context.getFilesDir();
        try (FileChannel channel = FileChannel.open(new File(root, "bootstrap.lock").toPath(), StandardOpenOption.CREATE, StandardOpenOption.WRITE);
             FileLock lock = channel.lock()) {
            if (!lock.isValid()) throw new IllegalStateException("Bootstrap lock unavailable");
            installOfflineGlosses(context, new File(root, "bootstrap/offline-glosses"));
            File configuration = new File(root, "runtime-options.json");
            if (configuration.exists()) return false;
            File resources = new File(root, "bootstrap/resources");
            Files.createDirectories(resources.toPath());
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
                    Files.copy(input, new File(resources, name).toPath(), StandardCopyOption.REPLACE_EXISTING);
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
            if (marker.isFile() && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "offline-glosses.staging");
            deleteTree(staging);
            Files.createDirectories(staging.toPath());
            String[] names = context.getAssets().list("offline-glosses");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("offline-glosses/" + name)) {
                    Files.copy(input, new File(staging, name).toPath());
                }
            }
            Files.write(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Offline gloss extraction failed", error);
        }
    }

    static String readMarker(java.nio.file.Path file) {
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
