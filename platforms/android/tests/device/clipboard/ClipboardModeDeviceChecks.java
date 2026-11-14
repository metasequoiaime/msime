package app.msime.android.test;

import android.content.Context;
import android.content.SharedPreferences;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.TextView;
import java.io.File;
import java.io.FileOutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在实际产品服务上切换模式并重开面板；只读写备份过的合成历史，不读取系统剪贴板。 */
final class ClipboardModeDeviceChecks {
    private ClipboardModeDeviceChecks() {}

    static void run(Context context, Class<?> type, Object service, ClassLoader loader) throws Exception {
        File main = new File(context.getFilesDir(), "MSIME/clipboard_history.json");
        File visitor = new File(context.getFilesDir(), "clipboard-visitor/MSIME/clipboard_history.json");
        byte[] mainBackup = main.isFile() ? Files.readAllBytes(main.toPath()) : null;
        byte[] visitorBackup = visitor.isFile() ? Files.readAllBytes(visitor.toPath()) : null;
        SharedPreferences saved = context.getSharedPreferences("android-clipboard-mode", Context.MODE_PRIVATE);
        boolean present = saved.contains("visitor"), wasVisitor = saved.getBoolean("visitor", false);
        Object oldMode = field(type, "clipboardMode").get(service);
        Object oldHistory = field(type, "clipboardHistory").get(service);
        Object oldTab = field(type, "clipboardTab").get(service);
        boolean oldEnabled = field(type, "clipboardHistoryEnabled").getBoolean(service);
        try {
            write(main, "Synthetic main record");
            write(visitor, "Synthetic visitor record");
            Class<?> mode = loader.loadClass("app.msime.android.ClipboardModePolicy$Mode");
            Class<?> tab = loader.loadClass("app.msime.android.CloudClipboardPanelPolicy$Tab");
            Class<?> icon = loader.loadClass("app.msime.android.KeyboardIconKey");
            field(type, "clipboardMode").set(service, constant(mode, "MAIN"));
            field(type, "clipboardHistoryEnabled").setBoolean(service, true);
            Button enter = (Button) field(type, "enterButton").get(service);
            Button shortcut = (Button) field(type, "clipboardShortcutButton").get(service);
            View surface = (View) field(type, "clipboardSurface").get(service);
            Object panels = field(type, "imePanels").get(service);
            Method render = method(panels.getClass(), "renderClipboardHistory");
            // 删除撤销也属于原模式；切换后不能把原模式的条目恢复到另一份历史。
            Class<?> store = loader.loadClass("app.msime.android.ClipboardHistoryStore");
            Object mainStore = store.getMethod("forMode", Context.class, mode).invoke(null, context, constant(mode, "MAIN"));
            field(type, "clipboardHistory").set(service, mainStore);
            method(type, "holdClipboardUndo", java.util.List.class).invoke(service, store.getMethod("load").invoke(mainStore));
            method(type, "toggleClipboardMode", icon).invoke(service, enter);
            boolean pendingUndo = (boolean) method(type, "hasClipboardUndo").invoke(service);
            method(type, "undoClipboardItems").invoke(service);
            check(java.util.Arrays.equals(Files.readAllBytes(visitor.toPath()), entry("Synthetic visitor record")),
                "Undo restored a main-mode record into visitor history");
            check(!pendingUndo, "Mode switch retained the other mode undo payload");
            field(type, "clipboardMode").set(service, constant(mode, "MAIN"));
            field(type, "clipboardHistory").set(service, mainStore);
            for (String initialTab : new String[] {"LOCAL", "CLOUD"}) {
                for (String next : new String[] {"VISITOR", "MAIN"}) {
                    field(type, "clipboardTab").set(service, constant(tab, initialTab));
                    surface.setVisibility(View.VISIBLE);
                    shortcut.setSelected(true);
                    method(type, "toggleClipboardMode", icon).invoke(service, enter);
                    check(next.equals(field(type, "clipboardMode").get(service).toString()), "Mode did not switch");
                    check(saved.getBoolean("visitor", false) == next.equals("VISITOR"), "Mode was not persisted");
                    check(surface.getVisibility() == View.GONE, "Old panel remained visible");
                    boolean redrawn = !shortcut.isSelected();
                    check((next.equals("VISITOR") ? "TRASH" : "RETURN").equals(icon.getMethod("kind").invoke(enter).toString()),
                        "Immediate return-key transition icon was lost while rendering");
                    // 只绘制历史，不调用会补读系统剪贴板或联网的打开入口。
                    render.invoke(panels);
                    surface.setVisibility(View.VISIBLE);
                    capture(context, type, service, next.toLowerCase(java.util.Locale.ROOT));
                    check(redrawn, "Gesture did not redraw the closed clipboard toolbar");
                    check("LOCAL".equals(field(type, "clipboardTab").get(service).toString()), "Cloud tab survived a mode switch");
                    String expected = next.equals("VISITOR") ? "Synthetic visitor record" : "Synthetic main record";
                    String forbidden = next.equals("VISITOR") ? "Synthetic main record" : "Synthetic visitor record";
                    check(contains(surface, expected), "Reopened panel did not show the selected mode history");
                    check(!contains(surface, forbidden), "Reopened panel exposed the other mode history");
                    Object history = field(type, "clipboardHistory").get(service);
                    int limit = (int) store.getMethod("limit").invoke(history);
                    check(contains(surface, (next.equals("VISITOR") ? "1/" : "2/") + limit),
                        "Panel count did not follow the selected mode history");
                }
            }
            check(java.util.Arrays.equals(Files.readAllBytes(main.toPath()), entry("Synthetic main record")),
                "Mode switch rewrote main history");
            check(java.util.Arrays.equals(Files.readAllBytes(visitor.toPath()), entry("Synthetic visitor record")),
                "Mode switch rewrote visitor history");
        } finally {
            restore(main, mainBackup);
            restore(visitor, visitorBackup);
            SharedPreferences.Editor edit = saved.edit();
            if (present) edit.putBoolean("visitor", wasVisitor); else edit.remove("visitor");
            check(edit.commit(), "Mode backup restoration failed");
            field(type, "clipboardMode").set(service, oldMode);
            field(type, "clipboardHistory").set(service, oldHistory);
            field(type, "clipboardTab").set(service, oldTab);
            field(type, "clipboardHistoryEnabled").setBoolean(service, oldEnabled);
            method(type, "closeClipboardHistory").invoke(service);
        }
    }

    private static byte[] entry(String text) throws Exception {
        JSONArray items = new JSONArray().put(new JSONObject().put("text", text)
            .put("timestampMs", 1700000000000L).put("pinned", true));
        if (text.equals("Synthetic main record")) items.put(new JSONObject().put("text", "Synthetic main second record")
            .put("timestampMs", 1699999999999L).put("pinned", true));
        return items.toString().getBytes(StandardCharsets.UTF_8);
    }
    private static void write(File file, String text) throws Exception {
        Files.createDirectories(file.toPath().getParent());
        Files.write(file.toPath(), entry(text));
    }
    private static void restore(File file, byte[] bytes) throws Exception {
        if (bytes == null) Files.deleteIfExists(file.toPath()); else Files.write(file.toPath(), bytes);
    }
    private static boolean contains(View view, String text) {
        if (view instanceof TextView label) {
            String value = label.getText().toString();
            // 历史卡片的下一行是时间和固定状态，正文与计数仍按完整一行比较。
            int newline = value.indexOf('\n');
            if (text.equals(newline < 0 ? value : value.substring(0, newline))) return true;
        }
        if (view instanceof ViewGroup group)
            for (int i = 0; i < group.getChildCount(); i++) if (contains(group.getChildAt(i), text)) return true;
        return false;
    }
    private static void capture(Context context, Class<?> type, Object service, String mode) throws Exception {
        View root = (View) field(type, "keyboardRoot").get(service);
        float density = context.getResources().getDisplayMetrics().density;
        int width = Math.round(400 * density);
        root.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        root.layout(0, 0, width, root.getMeasuredHeight());
        Bitmap bitmap = Bitmap.createBitmap(width, root.getHeight(), Bitmap.Config.ARGB_8888);
        try (FileOutputStream out = new FileOutputStream(new File(context.getCacheDir(), "clipboard-mode-" + mode + ".png"))) {
            root.draw(new Canvas(bitmap));
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, out), "Component capture failed");
        } finally { bitmap.recycle(); }
    }
    private static Object constant(Class<?> type, String name) {
        return java.util.Arrays.stream(type.getEnumConstants()).filter(value -> name.equals(value.toString()))
            .findFirst().orElseThrow();
    }
    private static Field field(Class<?> type, String name) throws Exception {
        Field value = type.getDeclaredField(name); value.setAccessible(true); return value;
    }
    private static Method method(Class<?> type, String name, Class<?>... args) throws Exception {
        Method value = type.getDeclaredMethod(name, args); value.setAccessible(true); return value;
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
