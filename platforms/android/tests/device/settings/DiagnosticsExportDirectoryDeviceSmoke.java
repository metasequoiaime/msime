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
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.provider.DocumentsContract;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.BooleanSupplier;
import java.util.function.Predicate;
import java.util.zip.ZipInputStream;

/** 从原生开发者页选择合成文件夹，跨进程重启验证持久授权，并覆盖取消、撤权和提供方失败。 */
public final class DiagnosticsExportDirectoryDeviceSmoke extends Instrumentation {
    private static final String KEY = "platform.android.developer.diagnostics_export_tree";
    private static final String ROOT = "content://" + DiagnosticDocumentsProvider.AUTHORITY + "/tree/root";
    private String phase;
    private String stage = "prepare";
    private UiAutomation automation;
    private Class<?> directories;
    private Class<?> settings;
    private Activity activity;

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        phase = arguments == null ? "verify" : arguments.getString("phase", "verify");
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
            directories = getTargetContext().getClassLoader().loadClass("app.msime.android.home.DiagnosticsExportDirectory");
            settings = getTargetContext().getClassLoader().loadClass("app.msime.android.AndroidLocalSettings");
            if (phase.equals("select")) select();
            else verify();
            code = Activity.RESULT_OK;
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: diagnostic directory " + phase + "\n");
        } catch (Exception | AssertionError | LinkageError failure) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: diagnostic directory " + stage
                + " (" + failure.getClass().getSimpleName() + ")\n");
        } finally {
            if (code != Activity.RESULT_OK && settings != null) {
                try { restoreSettings(); } catch (Exception ignored) { result.putString("cleanup", "settings restore failed"); }
            }
            if (activity != null) runOnMainSync(activity::finish);
        }
        finish(code, result);
    }

    private void select() throws Exception {
        stage = "save previous selection";
        Path local = settingsFile();
        Files.write(state(), Files.exists(local) ? Files.readAllBytes(local) : new byte[0]);
        put(null);
        stage = "first export opens picker";
        openPage();
        tap(text("导出"));
        selectFolder(null);
        waitFor(() -> ROOT.equals(savedUnchecked()), "directory selection persisted");
        waitFor(() -> filesUnchecked().size() == 1, "first export completed");
        check(hasGrant(), "real cross-UID persistent grant");
        checkZip(files().values().iterator().next());
    }

    private void verify() throws Exception {
        try {
            stage = "grant survived product process restart";
            check(ROOT.equals(saved()) && hasGrant(), "persisted selection and permission");
            Map<String, byte[]> first = files();
            check(first.size() == 1, "one initial archive");
            openPage();
            stage = "direct export after restart";
            tap(text("导出"));
            waitFor(() -> filesUnchecked().size() == 2, "second export without picker");
            Map<String, byte[]> second = files();
            for (var entry : first.entrySet()) check(Arrays.equals(entry.getValue(), second.get(entry.getKey())), "old ZIP retained");
            for (byte[] bytes : second.values()) checkZip(bytes);

            stage = "cancel folder change";
            tap(text("默认导出文件夹"));
            await(node -> picker(node) && node.isVisibleToUser());
            shell("input keyevent 4");
            await(text("默认导出文件夹"));
            check(ROOT.equals(saved()) && files().size() == 2, "cancel preserves selection and files");

            stage = "sharing archives retain independent contents";
            sharingArchives();

            stage = "reject incomplete permission";
            expectFailure("remember", new Class<?>[] {Context.class, Uri.class, int.class},
                getTargetContext(), Uri.parse(ROOT), Intent.FLAG_GRANT_READ_URI_PERMISSION);
            check(ROOT.equals(saved()) && hasGrant(), "failed permission change preserves old grant");

            stage = "revoked permission prevents export";
            getTargetContext().getContentResolver().releasePersistableUriPermission(Uri.parse(ROOT),
                Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            expectFailure("inspect", new Class<?>[] {Context.class, String.class}, getTargetContext(), ROOT);
            String message = export();
            check(message.contains("重新选择"), "permission recovery explained");
            stage = "reselect after revocation";
            tap(text("默认导出文件夹"));
            selectFolder(null);
            waitFor(this::hasGrant, "new persistent grant");
            check(files().size() == 2, "reselection alone does not export");

            Path archive = Files.createTempFile(getTargetContext().getCacheDir().toPath(), "synthetic-export-", ".zip");
            try {
                Files.write(archive, new byte[] {1, 2, 3, 4});
                stage = "provider returns existing nonempty document";
                tap(text("默认导出文件夹"));
                selectFolder("existing-target");
                waitFor(() -> savedUnchecked().contains("existing-target"), "fault directory persisted");
                expectFailure("write", new Class<?>[] {Context.class, String.class, File.class},
                    getTargetContext(), saved(), archive.toFile());
                Uri existing = document(saved(), "root/existing-target/existing.zip");
                try (var input = getTargetContext().getContentResolver().openInputStream(existing)) {
                    check(input != null && Arrays.equals(read(input), new byte[] {9, 8, 7}), "existing document unchanged");
                }
                stage = "provider fails opening write stream";
                tap(text("默认导出文件夹"));
                selectFolder("failed-write");
                waitFor(() -> savedUnchecked().contains("failed-write"), "unwritable directory persisted");
                expectFailure("write", new Class<?>[] {Context.class, String.class, File.class},
                    getTargetContext(), saved(), archive.toFile());
                try (Cursor cursor = getTargetContext().getContentResolver().query(
                        document(saved(), "root/failed-write/partial.zip"), null, null, null, null)) {
                    check(cursor != null && cursor.moveToFirst(), "failed document is retained");
                }
            } finally { Files.deleteIfExists(archive); }

            stage = "deleted directory prevents export";
            tap(text("默认导出文件夹"));
            selectFolder(null);
            waitFor(() -> ROOT.equals(savedUnchecked()), "restore root directory");
            check(files().size() == 2, "fault exports did not modify root archives");
            check(DocumentsContract.deleteDocument(getTargetContext().getContentResolver(), document(ROOT, "root")),
                "delete synthetic root");
            expectFailure("inspect", new Class<?>[] {Context.class, String.class}, getTargetContext(), ROOT);
            check(export().contains("重新选择"), "deleted directory recovery explained");
            stage = "reset clears default directory";
            settings.getMethod("restoreDefaults", Context.class).invoke(null, getTargetContext());
            check(saved().isEmpty(), "reset has no destination");
        } finally {
            restoreSettings();
        }
    }

    private void openPage() throws Exception {
        Intent intent = new Intent().setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
            .putExtra("msime_open_page", "DEVELOPER").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        activity = startActivitySync(intent);
        tap(text("数据"));
        await(text("默认导出文件夹"));
        await(node -> text("导出").test(node) && node.isEnabled());
    }

    private void selectFolder(String child) throws Exception {
        String reason = stage;
        await(node -> picker(node) && node.isVisibleToUser());
        stage = "picker navigation drawer: " + reason;
        // 初始页可能是最近文件、其他根或上次的子目录，先从导航抽屉进入合成根。
        tap(node -> picker(node) && node.getContentDescription() != null && (
            node.getContentDescription().toString().equalsIgnoreCase("Show roots")
                || node.getContentDescription().toString().contains("根目录")
                || node.getContentDescription().toString().contains("导航")));
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
            if (findAll(text("默认导出文件夹")) != null) break;
            SystemClock.sleep(100);
        }
        await(text("默认导出文件夹"));
        await(node -> text("导出").test(node) && node.isEnabled());
    }

    private Map<String, byte[]> files() throws Exception {
        Map<String, byte[]> result = new LinkedHashMap<>();
        Uri root = Uri.parse(ROOT);
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(root, "root");
        try (Cursor cursor = getTargetContext().getContentResolver().query(children,
                new String[] {DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_MIME_TYPE},
                null, null, null)) {
            check(cursor != null, "directory list");
            while (cursor.moveToNext()) {
                if (!"application/zip".equals(cursor.getString(1))) continue;
                String id = cursor.getString(0);
                try (var input = getTargetContext().getContentResolver().openInputStream(document(ROOT, id))) {
                    check(input != null, "archive readable");
                    result.put(id, read(input));
                }
            }
        }
        return result;
    }

    private Map<String, byte[]> filesUnchecked() {
        try { return files(); } catch (Exception unavailable) { return Map.of(); }
    }

    private boolean hasGrant() {
        return getTargetContext().getContentResolver().getPersistedUriPermissions().stream().anyMatch(
            permission -> permission.getUri().toString().equals(ROOT) && permission.isReadPermission() && permission.isWritePermission());
    }

    private Path state() { return getTargetContext().getCacheDir().toPath().resolve("diagnostic-export-suite-state.json"); }
    private Path settingsFile() throws Exception { return (Path) settings.getMethod("file", Context.class).invoke(null, getTargetContext()); }
    private void restoreSettings() throws Exception {
        if (!Files.exists(state())) return;
        byte[] previous = Files.readAllBytes(state());
        Path local = settingsFile();
        if (previous.length == 0) Files.deleteIfExists(local);
        else {
            AtomicFile atomic = new AtomicFile(local.toFile());
            java.io.FileOutputStream output = atomic.startWrite();
            try { output.write(previous); atomic.finishWrite(output); }
            catch (IOException failure) { atomic.failWrite(output); throw failure; }
        }
        Files.delete(state());
    }
    private String saved() throws Exception { return (String) invoke("saved", new Class<?>[] {Context.class}, getTargetContext()); }
    private String savedUnchecked() { try { return saved(); } catch (Exception unavailable) { return ""; } }
    private void put(Object value) throws Exception { settings.getMethod("put", Context.class, String.class, Object.class).invoke(null, getTargetContext(), KEY, value); }
    private static Uri document(String tree, String id) { return DocumentsContract.buildDocumentUriUsingTree(Uri.parse(tree), id); }

    private String export() throws Exception {
        Class<?> page = getTargetContext().getClassLoader().loadClass("app.msime.android.home.DeveloperPage");
        Method method = page.getDeclaredMethod("exportToDirectory", Context.class);
        method.setAccessible(true);
        return (String) method.invoke(null, getTargetContext());
    }

    private void sharingArchives() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> includeType = loader.loadClass("app.msime.android.DiagnosticsApi$Include");
        Object include = includeType.getConstructor(boolean.class, boolean.class, boolean.class, boolean.class)
            .newInstance(true, true, false, true);
        Method create = loader.loadClass("app.msime.android.home.DeveloperPage")
            .getDeclaredMethod("createExportBundle", Context.class, includeType);
        create.setAccessible(true);
        File first = null;
        File second = null;
        File expired = null;
        try {
            String firstPath = (String) create.invoke(null, getTargetContext(), include);
            check(firstPath != null, "first sharing archive");
            first = new File(firstPath);
            byte[] previous = Files.readAllBytes(first.toPath());
            expired = new File(first.getParentFile(), "msime-diagnostics-synthetic-expired.zip");
            Files.write(expired.toPath(), new byte[] {1});
            check(expired.setLastModified(1), "expired cache timestamp");
            String secondPath = (String) create.invoke(null, getTargetContext(), include);
            check(secondPath != null, "second sharing archive");
            second = new File(secondPath);
            check(!first.equals(second), "sharing archive identity");
            check(Arrays.equals(previous, Files.readAllBytes(first.toPath())), "old sharing contents unchanged");
            check(!expired.exists(), "only expired sharing cache is removed");
        } finally {
            if (first != null) Files.deleteIfExists(first.toPath());
            if (second != null) Files.deleteIfExists(second.toPath());
            if (expired != null) Files.deleteIfExists(expired.toPath());
        }
    }

    private Object invoke(String name, Class<?>[] types, Object... args) throws Exception {
        Method method = directories.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return method.invoke(null, args);
    }

    private void expectFailure(String name, Class<?>[] types, Object... args) throws Exception {
        try { invoke(name, types, args); throw new AssertionError("expected directory failure"); }
        catch (InvocationTargetException failed) {
            check(failed.getCause() instanceof IOException || failed.getCause() instanceof SecurityException,
                "expected storage failure category");
        }
    }

    private static void checkZip(byte[] bytes) throws IOException {
        try (ZipInputStream zip = new ZipInputStream(new java.io.ByteArrayInputStream(bytes))) {
            check(zip.getNextEntry() != null, "native archive contains entries");
        }
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
        return find(automation.getRootInActiveWindow(), predicate);
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
