package app.msime.android.home;

import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.UriPermission;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.BackupDestinationWriter;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/** 只向用户持久授权的文档树导出；所有查询与写入均在存储工作线程运行。 */
final class DiagnosticsExportDirectory {
    private static final int ACCESS = Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION;

    record Directory(Uri tree, String name) {}

    private DiagnosticsExportDirectory() {}

    static String saved(Context context) {
        return AndroidLocalSettings.load(context).text(AndroidLocalSettings.DIAGNOSTICS_EXPORT_TREE);
    }

    static Directory inspect(Context context, String value) throws IOException {
        Uri tree = tree(value);
        if (!hasGrant(context.getContentResolver(), tree)) throw new IOException("directory permission unavailable");
        Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree));
        try (Cursor cursor = context.getContentResolver().query(document, new String[] {
                DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE,
                DocumentsContract.Document.COLUMN_FLAGS}, null, null, null)) {
            if (cursor == null || !cursor.moveToFirst()
                    || !DocumentsContract.Document.MIME_TYPE_DIR.equals(cursor.getString(1))
                    || (cursor.getInt(2) & DocumentsContract.Document.FLAG_DIR_SUPPORTS_CREATE) == 0) {
                throw new IOException("directory unavailable");
            }
            String name = cursor.getString(0);
            return new Directory(tree, name == null || name.isBlank() ? "已选文件夹" : name);
        }
    }

    /** 校验新授权并持久保存成功后才释放旧目录；失败不改变原设置。 */
    static Directory remember(Context context, Uri selected, int offeredFlags) throws IOException {
        Uri tree = tree(selected.toString());
        if ((offeredFlags & ACCESS) != ACCESS) throw new IOException("directory access not offered");
        ContentResolver resolver = context.getContentResolver();
        String previous = saved(context);
        boolean retained = hasGrant(resolver, tree);
        boolean stored = false;
        try {
            resolver.takePersistableUriPermission(tree, ACCESS);
            Directory directory = inspect(context, tree.toString());
            AndroidLocalSettings.put(context, AndroidLocalSettings.DIAGNOSTICS_EXPORT_TREE, tree.toString());
            stored = true;
            if (!previous.isEmpty() && !previous.equals(tree.toString())) release(resolver, Uri.parse(previous));
            return directory;
        } finally {
            if (!stored && !retained) release(resolver, tree);
        }
    }

    /** 提供方若返回已有非空文档，空文档写入器拒绝截断；任何失败都不删除用户文档。 */
    static String write(Context context, String selected, File archive) throws IOException {
        Directory directory = inspect(context, selected);
        Uri parent = DocumentsContract.buildDocumentUriUsingTree(directory.tree(),
            DocumentsContract.getTreeDocumentId(directory.tree()));
        ContentResolver resolver = context.getContentResolver();
        Uri destination = DocumentsContract.createDocument(resolver, parent, "application/zip", archive.getName());
        if (destination == null || destination.equals(parent)) throw new IOException("document not created");
        BackupDestinationWriter.write(archive.toPath(), new BackupDestinationWriter.Destination() {
            @Override public InputStream openForRead() throws IOException { return resolver.openInputStream(destination); }
            @Override public OutputStream openForWrite() throws IOException {
                return resolver.openOutputStream(destination, "wt");
            }
        });
        return directory.name();
    }

    private static Uri tree(String value) throws IOException {
        Uri uri = Uri.parse(value);
        if (!"content".equals(uri.getScheme()) || uri.getAuthority() == null
                || !DocumentsContract.isTreeUri(uri)) throw new IOException("invalid export directory");
        return uri;
    }

    private static boolean hasGrant(ContentResolver resolver, Uri tree) {
        for (UriPermission permission : resolver.getPersistedUriPermissions()) {
            if (tree.equals(permission.getUri()) && permission.isReadPermission() && permission.isWritePermission()) return true;
        }
        return false;
    }

    private static void release(ContentResolver resolver, Uri tree) {
        try {
            resolver.releasePersistableUriPermission(tree, ACCESS);
        } catch (SecurityException | IllegalArgumentException ignored) {
            // 旧目录可能已由系统撤销，不能让新目录保存成功被当作失败。
        }
    }
}
