package app.msime.android;

import app.msime.android.policy.HostOptionsPolicy;
import android.content.ContentResolver;
import android.content.ContentValues;
import android.content.Context;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.MediaStore;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.time.LocalDateTime;
import org.json.JSONObject;

/** 显式操作才导出全文；调用方必须放到存储工作线程，不能记录快照内容。 */
public final class ClipboardExport {
    public static final String NEEDS_DIRECTORY = "请先选择剪贴板导出文件夹";
    private ClipboardExport() {}

    public static String saved(Context context) {
        return ExportDirectory.saved(context, AndroidLocalSettings.CLIPBOARD_EXPORT_TREE);
    }

    public static String directoryName(Context context) {
        String selected = saved(context);
        if (selected.isEmpty()) return Build.VERSION.SDK_INT < 29 ? "Download（首次需授权）" : "Download";
        try { return ExportDirectory.inspect(context, selected).name(); }
        catch (IOException | RuntimeException unavailable) { return "目录不可用，请重新选择"; }
    }

    /** 在生成快照和创建文件前重新读隐私开关；关闭时不导出残留记录。 */
    public static String export(Context context) {
        JSONObject preferences = preferences(context);
        if (preferences == null) return "剪贴板设置读取失败，请重试";
        if (!preferences.optBoolean("clipboard_history", false)) return "剪贴板历史未开启，未导出";
        File temporary = null;
        try {
            ClipboardHistoryStore.Export snapshot = new ClipboardHistoryStore(context.getFilesDir()).exportText();
            if (snapshot.count() == 0) return "剪贴板历史为空，未导出";
            String selected = saved(context);
            if (selected.isEmpty() && Build.VERSION.SDK_INT < 29) return NEEDS_DIRECTORY;
            if (!selected.isEmpty()) {
                try { ExportDirectory.inspect(context, selected); }
                catch (IOException | RuntimeException unavailable) { return "导出文件夹不可用，请重新选择"; }
            }
            String filename = LocalBackupPolicy.baseName(context.getApplicationInfo().loadLabel(context.getPackageManager()).toString(),
                AppVersionPolicy.versionName(context, "0"), LocalDateTime.now()) + ".txt";
            temporary = new File(context.getCacheDir(), filename);
            Files.write(temporary.toPath(), snapshot.text().getBytes(StandardCharsets.UTF_8));
            // 构建快照期间隐私设置可能已改变，公开文件前再检查一次。
            preferences = preferences(context);
            if (preferences == null || !preferences.optBoolean("clipboard_history", false))
                return "剪贴板历史未开启，未导出";
            String directory = selected.isEmpty() ? writeDownloads(context, temporary)
                : ExportDirectory.write(context, selected, temporary, "text/plain");
            return "导出完成：" + snapshot.count() + " 条，已保存到「" + directory + "」/" + filename;
        } catch (IOException | RuntimeException failed) {
            return "剪贴板未导出成功，请检查目录权限和剩余空间；若有未完成文件，请手动删除";
        } finally {
            if (temporary != null) temporary.delete();
        }
    }

    private static JSONObject preferences(Context context) {
        String directory = HostOptionsPolicy.readOption(context.getFilesDir(), "preferences_directory");
        if (directory.isEmpty()) return null;
        try {
            JSONObject response = new JSONObject(NativeClient.loadPreferences(directory));
            if (!JsonPolicy.strictTrue(response.opt("ok"))) return null;
            JSONObject snapshot = JsonPolicy.objectValue(response);
            return snapshot == null ? null : snapshot.optJSONObject("preferences");
        } catch (org.json.JSONException | LinkageError failed) { return null; }
    }

    @android.annotation.TargetApi(29)
    private static String writeDownloads(Context context, File source) throws IOException {
        ContentResolver resolver = context.getContentResolver();
        ContentValues values = new ContentValues();
        values.put(MediaStore.MediaColumns.DISPLAY_NAME, source.getName());
        values.put(MediaStore.MediaColumns.MIME_TYPE, "text/plain");
        values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS);
        values.put(MediaStore.MediaColumns.IS_PENDING, 1);
        Uri destination = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values);
        if (destination == null) throw new IOException("download not created");
        // MediaStore 首次写打开前只创建数据库记录，没有可读文件；rw 让系统落地文件，且不截断已有内容。
        try (android.os.ParcelFileDescriptor descriptor = resolver.openFileDescriptor(destination, "rw")) {
            if (descriptor == null) throw new IOException("download not opened");
        }
        // 复用空目标检查，防止异常提供方返回已有非空文件。失败时不删除提供方文档。
        BackupDestinationWriter.write(source.toPath(), new BackupDestinationWriter.Destination() {
            @Override public InputStream openForRead() throws IOException { return resolver.openInputStream(destination); }
            @Override public OutputStream openForWrite() throws IOException { return resolver.openOutputStream(destination, "wt"); }
        });
        values.clear();
        values.put(MediaStore.MediaColumns.IS_PENDING, 0);
        if (resolver.update(destination, values, null, null) != 1) throw new IOException("download not published");
        return "Download";
    }
}
