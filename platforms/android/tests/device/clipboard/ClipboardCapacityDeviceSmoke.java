package app.msime.android.test;

import android.content.Context;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.util.List;
import org.json.JSONObject;

/** 在原生 APK 上验收设置容量经 JNI 传给主模式与访客模式，重开后仍保留超过默认上限的记录。 */
public final class ClipboardCapacityDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "main and visitor clipboard histories retain sixty synthetic entries at the saved capacity";
    }

    @Override protected void runChecks() throws Exception {
        Context context = getTargetContext();
        ClassLoader loader = context.getClassLoader();
        Class<?> host = loader.loadClass("app.msime.android.home.HostStore");
        Method loadPreferences = host.getMethod("loadPreferences", Context.class);
        Method putPreference = host.getMethod("putPreference", Context.class, String.class, Object.class);
        JSONObject saved = (JSONObject) loadPreferences.invoke(null, context);
        if (saved == null) throw new AssertionError("Shared preferences are unavailable");
        int originalLimit = saved.getJSONObject("preferences").getInt("clipboard_history_limit");
        File files = context.getFilesDir();
        File main = new File(files, "MSIME/clipboard_history.json");
        File visitor = new File(files, "clipboard-visitor/MSIME/clipboard_history.json");
        byte[] mainBackup = Files.exists(main.toPath()) ? Files.readAllBytes(main.toPath()) : null;
        byte[] visitorBackup = Files.exists(visitor.toPath()) ? Files.readAllBytes(visitor.toPath()) : null;
        try {
            stage = "save one hundred entry limit";
            if (putPreference.invoke(null, context, "clipboard_history_limit", 100) == null)
                throw new AssertionError("Clipboard capacity could not be saved");
            verifyMode(context, "MAIN");
            verifyMode(context, "VISITOR");
        } finally {
            try {
                if (putPreference.invoke(null, context, "clipboard_history_limit", originalLimit) == null)
                    throw new AssertionError("Original clipboard capacity could not be restored");
            } finally {
                try {
                    restore(main, mainBackup);
                } finally {
                    restore(visitor, visitorBackup);
                }
            }
        }
    }

    private void verifyMode(Context context, String modeName) throws Exception {
        ClassLoader loader = context.getClassLoader();
        Class<?> modeClass = loader.loadClass("app.msime.android.ClipboardModePolicy$Mode");
        Object mode = null;
        for (Object candidate : modeClass.getEnumConstants()) {
            if (modeName.equals(candidate.toString())) mode = candidate;
        }
        if (mode == null) throw new AssertionError("Unknown clipboard mode");
        Class<?> store = loader.loadClass("app.msime.android.ClipboardHistoryStore");
        Method forMode = store.getMethod("forMode", Context.class, modeClass);
        Method clear = store.getMethod("clear");
        Method add = store.getMethod("add", String.class);
        Method load = store.getMethod("load");
        Method limit = store.getMethod("limit");
        stage = "capture sixty entries in " + mode;
        Object history = forMode.invoke(null, context, mode);
        clear.invoke(history);
        for (int index = 0; index < 60; index++) {
            String rejected = (String) add.invoke(history, String.format(java.util.Locale.ROOT,
                "Synthetic clipboard %s %03d", mode, index));
            if (rejected != null) throw new AssertionError("Synthetic clipboard entry was rejected");
        }
        stage = "reopen sixty entries in " + mode;
        Object reopened = forMode.invoke(null, context, mode);
        int retained = ((List<?>) load.invoke(reopened)).size();
        if (retained != 60 || ((Integer) limit.invoke(reopened)) != 100)
            throw new AssertionError("Saved clipboard capacity did not reach this mode");
    }

    private static void restore(File file, byte[] backup) throws Exception {
        if (backup == null) {
            Files.deleteIfExists(file.toPath());
        } else {
            Files.createDirectories(file.getParentFile().toPath());
            Files.write(file.toPath(), backup);
        }
    }
}
