package app.msime.android.test;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.DocumentsProvider;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.file.Files;

/** 专用模拟器的合成文档树，经系统选择器跨 UID 授权给产品，不接触外部存储。 */
public final class DiagnosticDocumentsProvider extends DocumentsProvider {
    static final String AUTHORITY = "app.msime.android.test.diagnostic-documents";
    private static final String[] ROOT_COLUMNS = {DocumentsContract.Root.COLUMN_ROOT_ID,
        DocumentsContract.Root.COLUMN_DOCUMENT_ID, DocumentsContract.Root.COLUMN_TITLE,
        DocumentsContract.Root.COLUMN_FLAGS, DocumentsContract.Root.COLUMN_MIME_TYPES};
    private static final String[] DOCUMENT_COLUMNS = {DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE,
        DocumentsContract.Document.COLUMN_FLAGS, DocumentsContract.Document.COLUMN_SIZE};

    @Override public boolean onCreate() { return true; }

    @Override public Cursor queryRoots(String[] projection) throws FileNotFoundException {
        File directory = file("root");
        if (!directory.isDirectory() && !directory.mkdirs()) throw new FileNotFoundException("fixture root");
        try {
            Files.createDirectories(file("root/existing-target").toPath());
            Files.createDirectories(file("root/failed-write").toPath());
            File existing = file("root/existing-target/existing.zip");
            if (!existing.exists()) Files.write(existing.toPath(), new byte[] {9, 8, 7});
        } catch (IOException failure) { throw new FileNotFoundException("fixture fault directories"); }
        MatrixCursor cursor = new MatrixCursor(projection == null ? ROOT_COLUMNS : projection);
        MatrixCursor.RowBuilder row = cursor.newRow();
        put(row, cursor, DocumentsContract.Root.COLUMN_ROOT_ID, "synthetic");
        put(row, cursor, DocumentsContract.Root.COLUMN_DOCUMENT_ID, "root");
        put(row, cursor, DocumentsContract.Root.COLUMN_TITLE, "MSIME 测试导出");
        put(row, cursor, DocumentsContract.Root.COLUMN_FLAGS, DocumentsContract.Root.FLAG_SUPPORTS_CREATE
            | DocumentsContract.Root.FLAG_LOCAL_ONLY | DocumentsContract.Root.FLAG_SUPPORTS_IS_CHILD);
        put(row, cursor, DocumentsContract.Root.COLUMN_MIME_TYPES, "*/*");
        return cursor;
    }

    @Override public Cursor queryDocument(String documentId, String[] projection) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection == null ? DOCUMENT_COLUMNS : projection);
        add(cursor, documentId);
        return cursor;
    }

    @Override public Cursor queryChildDocuments(String parentId, String[] projection, String sortOrder)
            throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection == null ? DOCUMENT_COLUMNS : projection);
        File[] files = file(parentId).listFiles();
        if (files != null) for (File child : files) add(cursor, parentId + "/" + child.getName());
        return cursor;
    }

    @Override public String createDocument(String parentId, String mimeType, String displayName)
            throws FileNotFoundException {
        if (parentId.equals("root/existing-target")) return "root/existing-target/existing.zip";
        String name = parentId.equals("root/failed-write") ? "partial.zip" : displayName;
        File child = file(parentId + "/" + name);
        // 和系统的 FileSystemProvider 一样，同名文件已存在时改用「名称 (n).扩展名」，不覆盖也不拒绝：导出文件名精确到分钟，同一分钟内的第二次导出在真实设备上就是这样落盘的。
        int dot = name.lastIndexOf('.');
        String stem = dot > 0 ? name.substring(0, dot) : name, extension = dot > 0 ? name.substring(dot) : "";
        for (int copy = 1; child.exists() && !parentId.equals("root/failed-write") && copy < 32; copy++) {
            name = stem + " (" + copy + ")" + extension;
            child = file(parentId + "/" + name);
        }
        try {
            if (!child.createNewFile()) throw new FileNotFoundException("fixture refuses replacement");
        } catch (IOException failure) { throw new FileNotFoundException("fixture create failed"); }
        return parentId + "/" + name;
    }

    @Override public ParcelFileDescriptor openDocument(String documentId, String mode, CancellationSignal signal)
            throws FileNotFoundException {
        if (documentId.startsWith("root/failed-write/")) {
            if (!mode.equals("r")) throw new FileNotFoundException("synthetic write failure");
        }
        return ParcelFileDescriptor.open(file(documentId), ParcelFileDescriptor.parseMode(mode));
    }

    @Override public void deleteDocument(String documentId) throws FileNotFoundException {
        try (var paths = Files.walk(file(documentId).toPath())) {
            for (var path : paths.sorted(java.util.Comparator.reverseOrder())
                    .collect(java.util.stream.Collectors.toList())) Files.delete(path);
        } catch (IOException failure) { throw new FileNotFoundException("fixture delete"); }
    }

    @Override public boolean isChildDocument(String parentId, String childId) {
        return childId.startsWith(parentId + "/");
    }

    private File file(String id) throws FileNotFoundException {
        if (id.contains("..") || id.startsWith("/")) throw new FileNotFoundException("invalid fixture id");
        return new File(getContext().getCacheDir(), "diagnostic-documents/" + id);
    }

    private void add(MatrixCursor cursor, String id) throws FileNotFoundException {
        File file = file(id);
        if (!file.exists()) throw new FileNotFoundException("fixture document missing");
        boolean directory = file.isDirectory();
        MatrixCursor.RowBuilder row = cursor.newRow();
        put(row, cursor, DocumentsContract.Document.COLUMN_DOCUMENT_ID, id);
        put(row, cursor, DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            id.equals("root") ? "MSIME 测试导出" : file.getName());
        put(row, cursor, DocumentsContract.Document.COLUMN_MIME_TYPE,
            directory ? DocumentsContract.Document.MIME_TYPE_DIR : "application/zip");
        put(row, cursor, DocumentsContract.Document.COLUMN_FLAGS,
            directory ? DocumentsContract.Document.FLAG_DIR_SUPPORTS_CREATE | DocumentsContract.Document.FLAG_SUPPORTS_DELETE
                : DocumentsContract.Document.FLAG_SUPPORTS_WRITE | DocumentsContract.Document.FLAG_SUPPORTS_DELETE);
        put(row, cursor, DocumentsContract.Document.COLUMN_SIZE, file.length());
    }

    private static void put(MatrixCursor.RowBuilder row, MatrixCursor cursor, String name, Object value) {
        if (cursor.getColumnIndex(name) >= 0) row.add(name, value);
    }
}
