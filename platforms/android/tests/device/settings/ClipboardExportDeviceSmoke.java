package app.msime.android.test;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.Activity;
import android.app.Instrumentation;
import android.app.UiAutomation;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.graphics.Rect;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.provider.DocumentsContract;
import android.provider.MediaStore;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.BooleanSupplier;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 合成全文经真实设置、系统认证、存储授权和跨进程重启导出，并覆盖无锁屏凭据、隐私与提供方故障。 */
public final class ClipboardExportDeviceSmoke extends Instrumentation {
    private static final String KEY = "platform.android.clipboard_export_tree";
    private static final String ROOT = "content://" + DiagnosticDocumentsProvider.AUTHORITY + "/tree/root";
    private static final String LONG = "  合成剪贴板😀\r\n" + repeatedText() + "\n合成尾行  ";
    private static String repeatedText() {
        StringBuilder text = new StringBuilder();
        for (int index = 0; index < 5000; index++) text.append("家");
        return text.toString();
    }
    private String phase;
    private String stage = "prepare";
    private UiAutomation automation;
    private Class<?> settings;
    private Class<?> host;
    private Object store;
    private Activity activity;
    private android.content.ClipData previousClip;
    private boolean clipChanged;

    @Override public void onCreate(Bundle args) {
        super.onCreate(args);
        phase = args == null ? "verify" : args.getString("phase", "verify");
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        int code = Activity.RESULT_CANCELED;
        try {
            automation = getUiAutomation();
            AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
            automation.setServiceInfo(info);
            ClassLoader loader = getTargetContext().getClassLoader();
            settings = loader.loadClass("app.msime.android.AndroidLocalSettings");
            host = loader.loadClass("app.msime.android.home.HostStore");
            store = loader.loadClass("app.msime.android.ClipboardHistoryStore").getConstructor(File.class)
                .newInstance(getTargetContext().getFilesDir());
            if (phase.equals("select")) select(); else verify();
            code = Activity.RESULT_OK;
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: clipboard export " + phase + "\n");
        } catch (Exception | AssertionError | LinkageError failed) {
            if (failed instanceof AssertionError) result.putString("assertion", failed.getMessage());
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: clipboard export " + stage
                + " (" + failed.getClass().getSimpleName() + ")\n");
        } finally {
            if (code != Activity.RESULT_OK || !phase.equals("select")) {
                try { restore(); } catch (Exception ignored) { result.putString("cleanup", "restore failed"); }
                try { DeviceCredentialFixture.clear(getTargetContext(), automation); }
                catch (Exception | AssertionError ignored) { result.putString("cleanup", "device credential clear failed"); }
            }
            if (activity != null) runOnMainSync(activity::finish);
        }
        finish(code, result);
    }

    private void select() throws Exception {
        stage = "backup synthetic device state";
        Path local = settingsFile();
        Files.write(state("local"), Files.exists(local) ? Files.readAllBytes(local) : new byte[0]);
        Files.write(state("history"), Files.exists(history()) ? Files.readAllBytes(history()) : new byte[0]);
        Files.write(state("enabled"), new byte[] {(byte) (preferences().getJSONObject("preferences")
            .optBoolean("clipboard_history", false) ? 1 : 0)});
        clearPendingSyntheticExports();
        put(null);
        enabled(true);
        seed();
        openPage();
        stage = "export without device credential is refused";
        // 专用 AVD 默认没有锁屏凭据。导出要求系统认证，此时必须说明原因，不写出文件、不打开目录选择器，按钮仍可再次使用。
        check(!DeviceCredentialFixture.secure(getTargetContext()), "dedicated AVD starts without a device credential");
        tap(text("导出"));
        await(text("这台设备没有可用的系统认证"));
        SystemClock.sleep(1000);
        check(downloadsUnchecked().isEmpty() && filesUnchecked().isEmpty() && findAll(ClipboardExportDeviceSmoke::picker) == null
            && !DeviceCredentialFixture.promptVisible(automation), "no export without device credential");
        await(node -> text("导出").test(node) && node.isEnabled());
        stage = "device credential fixture";
        DeviceCredentialFixture.set(getTargetContext(), automation);
        stage = "native export to default destination";
        tap(text("导出"));
        DeviceCredentialFixture.enter(automation);
        if (Build.VERSION.SDK_INT < 29) {
            selectFolder(null);
            waitFor(() -> filesUnchecked().size() == 1, "first authorized export");
            Map<String, byte[]> exported = files();
            check(exported.size() == 1, "first authorized export");
            for (String id : exported.keySet()) checkName(documentName(id));
            checkText(exported.values().iterator().next());
        } else {
            waitFor(() -> downloadsUnchecked().size() == 1, "default Download export");
            for (var entry : downloads().entrySet()) { checkName(downloadName(entry.getKey())); checkText(entry.getValue()); }
            choose();
            selectFolder(null);
        }
        check(ROOT.equals(saved()) && hasGrant(), "persistent cross-UID grant");
    }

    private void verify() throws Exception {
        stage = "directory survives product restart";
        check(ROOT.equals(saved()) && hasGrant(), "persisted directory");
        Map<String, byte[]> first = files();
        openPage();
        stage = "native direct export after restart";
        tap(text("导出"));
        DeviceCredentialFixture.enter(automation);
        waitFor(() -> filesUnchecked().size() == first.size() + 1, "direct export");
        Map<String, byte[]> second = files();
        for (var entry : second.entrySet()) { checkName(documentName(entry.getKey())); checkText(entry.getValue()); }
        for (var entry : first.entrySet()) check(Arrays.equals(entry.getValue(), second.get(entry.getKey())), "old file retained");
        stage = "real keyboard panel export action";
        int beforeKeyboard = files().size();
        var clipboard = getTargetContext().getSystemService(android.content.ClipboardManager.class);
        previousClip = clipboard.getPrimaryClip();
        clipboard.setPrimaryClip(android.content.ClipData.newPlainText("合成导出记录", LONG));
        clipChanged = true;
        shell("ime enable app.msime.android/.MSIMEInputService");
        shell("ime set app.msime.android/.MSIMEInputService");
        shell("am start -W -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(node -> "msime-test-plain".equals(node.getContentDescription() == null ? null : node.getContentDescription().toString()));
        tap(node -> "剪贴板".equals(node.getContentDescription() == null ? null : node.getContentDescription().toString()));
        tap(node -> "导出全部剪贴板历史".equals(node.getContentDescription() == null ? null : node.getContentDescription().toString()));
        // 输入法进程经认证桥 Activity 拉起系统认证，通过后才导出。
        DeviceCredentialFixture.enter(automation);
        waitFor(() -> filesUnchecked().size() == beforeKeyboard + 1, "keyboard export action");
        for (var entry : files().entrySet()) { checkName(documentName(entry.getKey())); checkText(entry.getValue()); }
        stage = "history page prompt interrupted by leaving";
        // 认证窗口开着时离开再回来：Android 10 起系统关掉后台应用的认证窗口，回来必须重新认证并读出记录，不能停在既无授权也无认证窗口的「正在读取…」；Android 9 的凭据确认是本任务里的 Activity，回来时仍在原处，认证成功后不再弹第二次。
        seed();
        activity = startActivitySync(new Intent().setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
            .putExtra("msime_open_page", "CLIPBOARD_SEARCH").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP));
        stage = "history page prompt interrupted by leaving: prompt before leaving";
        DeviceCredentialFixture.awaitPrompt(automation);
        stage = "history page prompt interrupted by leaving: leave and return";
        leaveAndReturn();
        stage = "history page prompt interrupted by leaving: prompt after return";
        DeviceCredentialFixture.enter(automation);
        stage = "history page prompt interrupted by leaving: records";
        await(text("合成新记录"));
        SystemClock.sleep(1000);
        check(!DeviceCredentialFixture.promptVisible(automation), "history page prompted again after authentication");
        stage = "history page export prompt interrupted by leaving";
        // 已授权时导出认证被离开打断：Android 10 起导出认证被系统取消，回来按重新进入处理，先认证页面、不导出；Android 9 回到仍在进行的凭据确认，认证后照常导出。
        int beforeInterrupted = files().size();
        tap(node -> "导出全部剪贴板历史".equals(node.getContentDescription() == null ? null : node.getContentDescription().toString()));
        stage = "history page export prompt interrupted by leaving: prompt before leaving";
        DeviceCredentialFixture.awaitPrompt(automation);
        stage = "history page export prompt interrupted by leaving: leave and return";
        leaveAndReturn();
        stage = "history page export prompt interrupted by leaving: prompt after return";
        DeviceCredentialFixture.enter(automation);
        stage = "history page export prompt interrupted by leaving: outcome";
        if (Build.VERSION.SDK_INT < 29) {
            waitFor(() -> filesUnchecked().size() == beforeInterrupted + 1, "credential confirmation survives leaving");
        } else {
            await(text("合成新记录"));
            SystemClock.sleep(1000);
            check(filesUnchecked().size() == beforeInterrupted && !DeviceCredentialFixture.promptVisible(automation),
                "interrupted export re-authenticates the page without exporting");
        }
        stage = "history page export action";
        activity = startActivitySync(new Intent().setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
            .putExtra("msime_open_page", "CLIPBOARD_SEARCH").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP));
        // 历史页打开时先认证才读取记录，页内导出再认证一次。
        DeviceCredentialFixture.enter(automation);
        int beforeSearch = files().size();
        tap(node -> "导出全部剪贴板历史".equals(node.getContentDescription() == null ? null : node.getContentDescription().toString()));
        DeviceCredentialFixture.enter(automation);
        waitFor(() -> filesUnchecked().size() == beforeSearch + 1, "history page export action");
        openPage();
        stage = "cancel directory change";
        choose();
        waitFor(() -> {
            AccessibilityNodeInfo root = automation.getRootInActiveWindow();
            return root != null && picker(root);
        }, "directory picker foreground");
        stage = "cancel picker returns to clipboard settings";
        // 关闭后的选择器仍可能留在其他窗口的无障碍树里，不能据此继续返回并退出设置页。
        shell("input keyevent 4");
        waitFor(() -> find(automation.getRootInActiveWindow(), text("导出文件夹")) != null,
            "clipboard settings foreground");
        check(ROOT.equals(saved()), "cancel preserves selection");

        stage = "incomplete permission does not change selection";
        expectFailure("remember", new Class<?>[] {Context.class, String.class, Uri.class, int.class},
            getTargetContext(), KEY, Uri.parse(ROOT), Intent.FLAG_GRANT_READ_URI_PERMISSION);
        check(ROOT.equals(saved()), "incomplete grant preserved selection");

        stage = "shared diagnostic grant remains usable";
        String diagnosticKey = "platform.android.developer.diagnostics_export_tree";
        settings.getMethod("put", Context.class, String.class, Object.class)
            .invoke(null, getTargetContext(), diagnosticKey, ROOT);
        choose();
        selectFolder("existing-target");
        check(hasGrant(), "shared root grant retained");
        stage = "nonempty provider document is not truncated";
        Path temporary = Files.createTempFile(getTargetContext().getCacheDir().toPath(), "synthetic-export-", ".txt");
        try {
            Files.write(temporary, new byte[] {1, 2, 3});
            expectFailure("write", new Class<?>[] {Context.class, String.class, File.class, String.class},
                getTargetContext(), saved(), temporary.toFile(), "text/plain");
            Uri existing = document(saved(), "root/existing-target/existing.zip");
            try (var input = getTargetContext().getContentResolver().openInputStream(existing)) {
                check(input != null && Arrays.equals(read(input), new byte[] {9, 8, 7}), "existing bytes retained");
            }
            stage = "write stream failure retains partial document";
            choose();
            selectFolder("failed-write");
            expectFailure("write", new Class<?>[] {Context.class, String.class, File.class, String.class},
                getTargetContext(), saved(), temporary.toFile(), "text/plain");
            try (Cursor cursor = getTargetContext().getContentResolver().query(
                    document(saved(), "root/failed-write/partial.zip"), null, null, null, null)) {
                check(cursor != null && cursor.moveToFirst(), "partial retained");
            }
        } finally { Files.deleteIfExists(temporary); }
        choose();
        selectFolder(null);
        stage = "revoked directory does not fall back to Downloads";
        int count = files().size();
        int downloaded = downloadsUnchecked().size();
        getTargetContext().getContentResolver().releasePersistableUriPermission(Uri.parse(ROOT),
            Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        check(export().contains("重新选择"), "revocation explained");
        check(downloadsUnchecked().size() == downloaded, "no silent fallback");
        choose();
        selectFolder(null);
        check(files().size() == count, "selection alone does not export");
        stage = "empty history creates no document";
        store.getClass().getMethod("clear").invoke(store);
        check(export().contains("为空") && files().size() == count, "empty rejected");
        seed();
        stage = "live privacy switch rejects residual history";
        enabled(false);
        check(export().contains("未开启") && files().size() == count, "disabled history not exported");
        enabled(true);
        stage = "reset default destination";
        tap(text("导出文件夹"));
        tap(text("Download（默认）"));
        waitFor(() -> savedUnchecked().isEmpty(), "default destination reset");
        // 关闭隐私开关允许输入法清空历史，目录验收必须重新建立非空合成夹具。
        seed();
        if (Build.VERSION.SDK_INT < 29) {
            String result = export();
            check(result.contains("先选择"), "legacy requests a grant; empty=" + result.contains("为空")
                + "; disabled=" + result.contains("未开启") + "; settings=" + result.contains("设置读取失败"));
        }
        else check(export().startsWith("导出完成"), "default Download restored");
        choose();
        selectFolder(null);
        stage = "deleted directory requires reselection";
        seed();
        check(DocumentsContract.deleteDocument(getTargetContext().getContentResolver(), document(ROOT, "root")), "delete root");
        check(export().contains("重新选择"), "deleted directory not redirected");
    }

    private void seed() throws Exception {
        store.getClass().getMethod("clear").invoke(store);
        store.getClass().getMethod("add", String.class).invoke(store, LONG);
        store.getClass().getMethod("add", String.class).invoke(store, "合成新记录");
        store.getClass().getMethod("setPinned", String.class, boolean.class).invoke(store, LONG, true);
    }

    /** 认证窗口开着时切到另一个应用再回到原任务；Android 10 起系统应已关掉后台应用的认证窗口。 */
    private void leaveAndReturn() throws Exception {
        shell("am start -W -n app.msime.android.test/app.msime.android.test.EditorActivity");
        if (Build.VERSION.SDK_INT >= 30) {
            waitFor(() -> !DeviceCredentialFixture.promptVisible(automation), "system kept the prompt of a background app");
        }
        SystemClock.sleep(1000);
        shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        automation.waitForIdle(300, 3000);
    }

    private void openPage() throws Exception {
        Intent intent = new Intent().setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
            .putExtra("msime_open_page", "KEYBOARD_OPTIONS").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        activity = startActivitySync(intent);
        // 剪贴板设置并进了「工具栏」分页，排在键盘工具栏一组之后，可能在屏幕外。
        tap(text("工具栏"));
        reveal(text("导出文件夹"));
        await(node -> text("导出").test(node) && node.isEnabled());
    }

    /** 向下滚动设置页，直到目标可见；查找只认可见节点，屏幕外的行等不到。 */
    private void reveal(Predicate<AccessibilityNodeInfo> predicate) throws Exception {
        for (int attempt = 0; attempt < 12 && findAll(predicate) == null; attempt++) {
            AccessibilityNodeInfo scroller = null;
            for (var window : automation.getWindows()) scroller = tallestScrollable(window.getRoot(), scroller);
            if (scroller == null || !scroller.performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)) break;
            automation.waitForIdle(300, 3000);
        }
        await(predicate);
    }

    /** 页面主体的纵向列表是最高的可滚动节点；分页标签和工具栏预览也可滚动，但是横向的矮条。 */
    private static AccessibilityNodeInfo tallestScrollable(AccessibilityNodeInfo node, AccessibilityNodeInfo best) {
        if (node == null || !node.isVisibleToUser()) return best;
        if (node.isScrollable()) {
            Rect bounds = new Rect();
            node.getBoundsInScreen(bounds);
            Rect current = new Rect();
            if (best != null) best.getBoundsInScreen(current);
            if (best == null || bounds.height() > current.height()) best = node;
        }
        for (int index = 0; index < node.getChildCount(); index++) best = tallestScrollable(node.getChild(index), best);
        return best;
    }

    private void choose() throws Exception {
        String reason = stage;
        stage = reason + ": directory settings row";
        tap(text("导出文件夹"));
        stage = reason + ": directory sheet option";
        tap(text("选择其他文件夹"));
        stage = reason;
    }

    private Map<String, byte[]> files() throws Exception {
        Map<String, byte[]> result = new LinkedHashMap<>();
        try (Cursor cursor = getTargetContext().getContentResolver().query(
                DocumentsContract.buildChildDocumentsUriUsingTree(Uri.parse(ROOT), "root"),
                new String[] {DocumentsContract.Document.COLUMN_DOCUMENT_ID}, null, null, null)) {
            check(cursor != null, "directory listing");
            while (cursor.moveToNext()) {
                String id = cursor.getString(0);
                if (!id.endsWith(".txt")) continue;
                try (var input = getTargetContext().getContentResolver().openInputStream(document(ROOT, id))) {
                    check(input != null, "text readable");
                    result.put(id, read(input));
                }
            }
        }
        return result;
    }

    private void clearPendingSyntheticExports() {
        if (Build.VERSION.SDK_INT < 29) return;
        var resolver = getTargetContext().getContentResolver();
        try (Cursor cursor = resolver.query(MediaStore.Downloads.EXTERNAL_CONTENT_URI,
                new String[] {MediaStore.MediaColumns._ID}, MediaStore.MediaColumns.IS_PENDING
                    + " = 1 AND (" + MediaStore.MediaColumns.DISPLAY_NAME + " LIKE ? OR "
                    + MediaStore.MediaColumns.DISPLAY_NAME + " LIKE ?)",
                new String[] {namePrefix() + "%", "msime-clipboard-%"}, null)) {
            if (cursor == null) return;
            while (cursor.moveToNext()) resolver.delete(android.content.ContentUris.withAppendedId(
                MediaStore.Downloads.EXTERNAL_CONTENT_URI, cursor.getLong(0)), null, null);
        }
    }

    private Map<Uri, byte[]> downloads() throws Exception {
        Map<Uri, byte[]> result = new LinkedHashMap<>();
        if (Build.VERSION.SDK_INT < 29) return result;
        try (Cursor cursor = getTargetContext().getContentResolver().query(MediaStore.Downloads.EXTERNAL_CONTENT_URI,
                new String[] {MediaStore.MediaColumns._ID, MediaStore.MediaColumns.IS_PENDING},
                MediaStore.MediaColumns.DISPLAY_NAME + " LIKE ?", new String[] {namePrefix() + "%.txt"}, null)) {
            check(cursor != null, "download listing");
            while (cursor.moveToNext()) {
                check(cursor.getInt(1) == 0, "download published");
                Uri uri = android.content.ContentUris.withAppendedId(MediaStore.Downloads.EXTERNAL_CONTENT_URI, cursor.getLong(0));
                try (var input = getTargetContext().getContentResolver().openInputStream(uri)) {
                    check(input != null, "download readable");
                    result.put(uri, read(input));
                }
            }
        }
        return result;
    }

    private Map<Uri, byte[]> downloadsUnchecked() { try { return downloads(); } catch (Exception failed) { return Map.of(); } }
    private Map<String, byte[]> filesUnchecked() { try { return files(); } catch (Exception failed) { return Map.of(); } }
    private boolean hasGrant() { return getTargetContext().getContentResolver().getPersistedUriPermissions().stream().anyMatch(
        grant -> grant.getUri().toString().equals(ROOT) && grant.isReadPermission() && grant.isWritePermission()); }
    private static Uri document(String tree, String id) { return DocumentsContract.buildDocumentUriUsingTree(Uri.parse(tree), id); }
    /** files() 以文档 ID（root/<文件名>）为键；文件名规则要对提供方报告的显示名验收。 */
    private String documentName(String id) throws Exception {
        try (Cursor cursor = getTargetContext().getContentResolver().query(document(ROOT, id),
                new String[] {DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            check(cursor != null && cursor.moveToFirst(), "document name");
            return cursor.getString(0);
        }
    }
    private String downloadName(Uri uri) throws Exception {
        try (Cursor cursor = getTargetContext().getContentResolver().query(uri,
                new String[] {MediaStore.MediaColumns.DISPLAY_NAME}, null, null, null)) {
            check(cursor != null && cursor.moveToFirst(), "download name");
            return cursor.getString(0);
        }
    }
    private Path state(String part) { return getTargetContext().getCacheDir().toPath().resolve("clipboard-export-suite-" + part); }
    private Path history() { return getTargetContext().getFilesDir().toPath().resolve("MSIME/clipboard_history.json"); }
    private Path settingsFile() throws Exception { return (Path) settings.getMethod("file", Context.class).invoke(null, getTargetContext()); }
    private String savedUnchecked() { try { return saved(); } catch (Exception failed) { return "unavailable"; } }
    private String saved() throws Exception { return (String) settings.getMethod("load", Context.class).invoke(null, getTargetContext())
        .getClass().getMethod("text", String.class).invoke(settings.getMethod("load", Context.class).invoke(null, getTargetContext()), KEY); }
    private void put(Object value) throws Exception { settings.getMethod("put", Context.class, String.class, Object.class)
        .invoke(null, getTargetContext(), KEY, value); }
    private JSONObject preferences() throws Exception { return (JSONObject) host.getMethod("loadPreferences", Context.class).invoke(null, getTargetContext()); }
    private void enabled(boolean value) throws Exception {
        JSONObject snapshot = preferences();
        snapshot.getJSONObject("preferences").put("clipboard_history", value);
        check(host.getMethod("savePreferences", Context.class, JSONObject.class).invoke(null, getTargetContext(), snapshot) != null, "privacy save");
    }
    private String export() throws Exception { return (String) getTargetContext().getClassLoader()
        .loadClass("app.msime.android.ClipboardExport").getMethod("export", Context.class).invoke(null, getTargetContext()); }
    private void restore() throws Exception {
        if (clipChanged) {
            var clipboard = getTargetContext().getSystemService(android.content.ClipboardManager.class);
            if (previousClip == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(previousClip);
            clipChanged = false;
        }
        if (!Files.exists(state("local"))) return;
        for (Uri uri : downloadsUnchecked().keySet()) getTargetContext().getContentResolver().delete(uri, null, null);
        enabled(Files.readAllBytes(state("enabled"))[0] == 1);
        restoreFile(settingsFile(), Files.readAllBytes(state("local")));
        restoreFile(history(), Files.readAllBytes(state("history")));
        for (String part : new String[] {"local", "history", "enabled"}) Files.deleteIfExists(state(part));
    }
    private static void restoreFile(Path path, byte[] bytes) throws IOException {
        if (bytes.length == 0) { Files.deleteIfExists(path); return; }
        AtomicFile atomic = new AtomicFile(path.toFile());
        var output = atomic.startWrite();
        try { output.write(bytes); atomic.finishWrite(output); }
        catch (IOException failed) { atomic.failWrite(output); throw failed; }
    }
    private void expectFailure(String name, Class<?>[] types, Object... args) throws Exception {
        Method method = getTargetContext().getClassLoader().loadClass("app.msime.android.ExportDirectory").getDeclaredMethod(name, types);
        method.setAccessible(true);
        try { method.invoke(null, args); throw new AssertionError("expected storage failure"); }
        catch (InvocationTargetException failed) { check(failed.getCause() instanceof IOException || failed.getCause() instanceof SecurityException, "storage failure category"); }
    }
    private static void checkText(byte[] bytes) {
        String text = new String(bytes, StandardCharsets.UTF_8);
        check(text.contains("共 2 条") && text.contains("1 · 已置顶 ==========\n" + LONG + "\n")
            && text.contains("2 ==========\n合成新记录\n"), "complete Unicode snapshot and order");
    }

    private String namePrefix() {
        String app = safeNamePart(getTargetContext().getApplicationInfo().loadLabel(
            getTargetContext().getPackageManager()).toString(), "msime");
        String version;
        try { version = getTargetContext().getPackageManager()
            .getPackageInfo(getTargetContext().getPackageName(), 0).versionName; }
        catch (Exception missing) { version = "0"; }
        return safeNamePart(app, "msime") + "-" + safeNamePart(version, "0") + "-";
    }

    private String safeNamePart(String value, String fallback) {
        if (value == null || value.isBlank()) return fallback;
        StringBuilder name = new StringBuilder(value.length());
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            boolean unsafe = character < 0x20 || "/\\:*?\"<>|".indexOf(character) >= 0
                || Character.isWhitespace(character);
            name.append(unsafe ? '_' : character);
        }
        return name.toString();
    }

    /** 文件名到分钟为止；同一分钟内再导出时，存储提供方按系统惯例追加「 (n)」，产品不能覆盖前一份。 */
    private void checkName(String name) {
        check(name.matches(java.util.regex.Pattern.quote(namePrefix()) + "\\d{8}-\\d{4}( \\([1-9]\\d*\\))?\\.txt"),
            "clipboard export uses the shared app-version timestamp name: " + name);
    }

    private void selectFolder(String child) throws Exception {
        String reason = stage;
        await(node -> picker(node) && node.isVisibleToUser());
        stage = "picker navigation drawer: " + reason;
        // 初始页可能是最近文件、其他根或上次的子目录，先从导航抽屉进入合成根。
        openPickerRoots();
        stage = "picker root option: " + reason;
        tap(node -> text("MSIME 测试导出").test(node) && "android:id/title".equals(node.getViewIdResourceName()));
        stage = "picker destination: " + reason;
        if (child != null) tap(text(child));
        tap(node -> picker(node) && node.isEnabled() && (text("USE THIS FOLDER").test(node)
            || text("使用此文件夹").test(node) || text("SELECT").test(node) || text("选择").test(node)));
        stage = "picker permission confirmation: " + reason;
        long deadline = SystemClock.uptimeMillis() + 5000;
        while (SystemClock.uptimeMillis() < deadline) {
            AccessibilityNodeInfo allow = findAll(node -> text("ALLOW").test(node) || text("允许").test(node));
            if (allow != null) { click(allow); break; }
            if (findAll(text("导出文件夹")) != null) break;
            SystemClock.sleep(100);
        }
        await(text("导出文件夹"));
        await(node -> text("导出").test(node) && node.isEnabled());
    }

    private AccessibilityNodeInfo activePicker() {
        if (Build.VERSION.SDK_INT >= 34) automation.clearCache();
        AccessibilityNodeInfo root = automation.getRootInActiveWindow();
        return root != null && picker(root) ? root : null;
    }

    private static boolean pickerRootsOpen(AccessibilityNodeInfo root) {
        return find(root, node -> node.getViewIdResourceName() != null
            && node.getViewIdResourceName().endsWith(":id/roots_list")) != null;
    }

    private void openPickerRoots() throws Exception {
        Predicate<AccessibilityNodeInfo> navigation = node -> node.getContentDescription() != null && (
            node.getContentDescription().toString().equalsIgnoreCase("Show roots")
                || node.getContentDescription().toString().contains("根目录")
                || node.getContentDescription().toString().contains("导航"));
        long deadline = SystemClock.uptimeMillis() + 15000;
        int attempts = 0;
        while (SystemClock.uptimeMillis() < deadline) {
            AccessibilityNodeInfo root = activePicker();
            if (pickerRootsOpen(root)) return;
            // 窗口切换期间读不到当前根时只等待；不能从后台窗口找到旧按钮再点击。
            if (find(root, navigation) != null) {
                automation.waitForIdle(300, 3000);
                root = activePicker();
                if (pickerRootsOpen(root)) return;
                AccessibilityNodeInfo button = find(root, navigation);
                if (button != null) {
                    check(attempts++ < 3, "picker navigation drawer did not open");
                    click(button);
                    // 点击返回不代表抽屉已打开，先等待可见结果，再决定是否需要重试。
                    long observation = Math.min(deadline, SystemClock.uptimeMillis() + 3000);
                    while (SystemClock.uptimeMillis() < observation) {
                        if (pickerRootsOpen(activePicker())) return;
                        SystemClock.sleep(100);
                    }
                }
            }
            SystemClock.sleep(100);
        }
        throw new AssertionError("picker navigation drawer did not open");
    }

    private static byte[] read(java.io.InputStream input) throws IOException {
        java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > 16 * 1024 * 1024) throw new IOException("synthetic archive too large");
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }

    private void waitFor(BooleanSupplier condition, String message) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            if (condition.getAsBoolean()) return;
            SystemClock.sleep(100);
        }
        throw new AssertionError(message);
    }

    private static Predicate<AccessibilityNodeInfo> text(String value) {
        return node -> node.getText() != null && value.equalsIgnoreCase(node.getText().toString());
    }

    private static boolean picker(AccessibilityNodeInfo node) {
        return node.getPackageName() != null && node.getPackageName().toString().endsWith(".documentsui");
    }

    private AccessibilityNodeInfo await(Predicate<AccessibilityNodeInfo> predicate) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            AccessibilityNodeInfo node = findAll(predicate);
            if (node != null) return node;
            SystemClock.sleep(100);
        }
        throw new AssertionError("synthetic control missing");
    }

    private AccessibilityNodeInfo findAll(Predicate<AccessibilityNodeInfo> predicate) {
        AccessibilityNodeInfo active = find(automation.getRootInActiveWindow(), predicate);
        if (active != null) return active;
        for (var window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), predicate);
            if (found != null) return found;
        }
        return null;
    }

    private static AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> predicate) {
        if (node == null || (android.os.Build.VERSION.SDK_INT < 34 && !node.refresh())) return null;
        if (node.isVisibleToUser() && predicate.test(node)) return node;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo found = find(node.getChild(index), predicate);
            if (found != null) return found;
        }
        return null;
    }

    private void tap(Predicate<AccessibilityNodeInfo> predicate) throws Exception {
        await(predicate);
        automation.waitForIdle(300, 3000);
        Rect previous = null;
        int stable = 0;
        long deadline = SystemClock.uptimeMillis() + 5000;
        while (SystemClock.uptimeMillis() < deadline) {
            AccessibilityNodeInfo node = await(predicate);
            Rect current = new Rect();
            node.getBoundsInScreen(current);
            stable = !current.isEmpty() && current.equals(previous) ? stable + 1 : 0;
            if (stable >= 2) {
                click(node);
                SystemClock.sleep(150);
                return;
            }
            previous = current;
            SystemClock.sleep(150);
        }
        throw new AssertionError("synthetic control bounds did not settle");
    }
    private void click(AccessibilityNodeInfo node) throws Exception {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        if (picker(node)) {
            shell("input tap " + bounds.centerX() + " " + bounds.centerY());
            return;
        }
        for (AccessibilityNodeInfo current = node; current != null; current = current.getParent()) {
            if (current.isClickable() && current.isEnabled() && current.performAction(AccessibilityNodeInfo.ACTION_CLICK)) return;
        }
        shell("input tap " + bounds.centerX() + " " + bounds.centerY());
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }

    private static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
