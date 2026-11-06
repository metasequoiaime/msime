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
        runCapacity(getTargetContext(), this);
    }

    static void runCapacity(Context context, DeviceSmoke owner) throws Exception {
        ClassLoader loader = context.getClassLoader();
        Class<?> host = loader.loadClass("app.msime.android.home.HostStore");
        Method loadPreferences = host.getMethod("loadPreferences", Context.class);
        Method savePreferences = host.getMethod("savePreferences", Context.class, JSONObject.class);
        JSONObject saved = (JSONObject) loadPreferences.invoke(null, context);
        if (saved == null) throw new AssertionError("Shared preferences are unavailable");
        int originalLimit = saved.getJSONObject("preferences").getInt("clipboard_history_limit");
        boolean originalHistory = saved.getJSONObject("preferences").optBoolean("clipboard_history", false);
        File files = context.getFilesDir();
        File main = new File(files, "MSIME/clipboard_history.json");
        File visitor = new File(files, "clipboard-visitor/MSIME/clipboard_history.json");
        byte[] mainBackup = Files.exists(main.toPath()) ? Files.readAllBytes(main.toPath()) : null;
        byte[] visitorBackup = Files.exists(visitor.toPath()) ? Files.readAllBytes(visitor.toPath()) : null;
        try {
            // 容量和开关在同一次保存里写：前一个套件留下的键盘若还在每秒读偏好，读到「剪贴板历史关」会按产品规则清空主模式历史（ClipboardHistoryRetentionPolicy），只改容量时这一清会落在写入六十条和重开读取之间，主模式就读不满六十条。
            owner.stage = "save one hundred entry limit with history on";
            if (!savePair(loadPreferences, savePreferences, context, 100, true))
                throw new AssertionError("Clipboard capacity could not be saved");
            verifyMode(context, owner, "MAIN", false);
            verifyMode(context, owner, "VISITOR", false);
            verifyMode(context, owner, "MAIN", true);
        } finally {
            try {
                if (!savePair(loadPreferences, savePreferences, context, originalLimit, originalHistory))
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

    private static void verifyMode(Context context, DeviceSmoke owner, String modeName, boolean delayedDisable) throws Exception {
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
        owner.stage = "capture sixty entries in " + mode;
        Object history = forMode.invoke(null, context, mode);
        clear.invoke(history);
        for (int index = 0; index < 60; index++) {
            String rejected = (String) add.invoke(history, String.format(java.util.Locale.ROOT,
                "Synthetic clipboard %s %03d", mode, index));
            if (rejected != null) throw new AssertionError("Synthetic clipboard entry was rejected");
            if (index == 0 && delayedDisable)
                ClipboardRetentionRaceDeviceChecks.deliverOldDisable(owner, loader, history, mode);
        }
        owner.stage = (delayedDisable ? "reopen after delayed disable in " : "reopen sixty entries in ") + mode;
        Object reopened = forMode.invoke(null, context, mode);
        int retained = ((List<?>) load.invoke(reopened)).size();
        int reopenedLimit = (Integer) limit.invoke(reopened);
        if (retained != 60 || reopenedLimit != 100)
            throw new AssertionError("Saved clipboard capacity did not reach this mode: retained "
                + retained + " of 60, limit " + reopenedLimit + " instead of 100");
    }

    /** 在一次读改写里同时写容量和剪贴板历史开关；保存被拒或失败时返回 false。 */
    private static boolean savePair(Method loadPreferences, Method savePreferences, Context context,
            int historyLimit, boolean history) throws Exception {
        JSONObject snapshot = (JSONObject) loadPreferences.invoke(null, context);
        if (snapshot == null) return false;
        snapshot.getJSONObject("preferences").put("clipboard_history_limit", historyLimit)
            .put("clipboard_history", history);
        return savePreferences.invoke(null, context, snapshot) != null;
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
