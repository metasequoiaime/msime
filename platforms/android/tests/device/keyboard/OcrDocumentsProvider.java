package app.msime.android.test;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.DocumentsProvider;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.RandomAccessFile;

/** 只向系统选择器提供专项 APK 内的两个固定合成文件，产品必须取得跨 UID 的读取授权。 */
public final class OcrDocumentsProvider extends DocumentsProvider {
    private static final String[] ROOT_COLUMNS = {DocumentsContract.Root.COLUMN_ROOT_ID,
        DocumentsContract.Root.COLUMN_DOCUMENT_ID, DocumentsContract.Root.COLUMN_TITLE,
        DocumentsContract.Root.COLUMN_FLAGS, DocumentsContract.Root.COLUMN_MIME_TYPES};
    private static final String[] DOCUMENT_COLUMNS = {DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE,
        DocumentsContract.Document.COLUMN_FLAGS, DocumentsContract.Document.COLUMN_SIZE};

    @Override public boolean onCreate() {
        // 测试 APK 更新时缓存可能保留，不能把上一轮的模块字节当作本轮资产。
        File directory = new File(getContext().getCacheDir(), "ocr-picker-fixtures");
        for (String id : new String[]{"valid", "corrupt"}) {
            File previous = new File(directory, id + ".apk");
            if (previous.exists() && !previous.delete()) return false;
        }
        return true;
    }

    @Override public Cursor queryRoots(String[] projection) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection == null ? ROOT_COLUMNS : projection);
        // 普通设备测试包没有模块资产，不向其选择器增加 OCR 根。
        try (var input = getContext().getAssets().open("ocr-product-module.apk")) {
            if (input.read() < 0) return cursor;
        } catch (IOException absent) { return cursor; }
        MatrixCursor.RowBuilder row = cursor.newRow();
        put(row, cursor, DocumentsContract.Root.COLUMN_ROOT_ID, "synthetic-ocr");
        put(row, cursor, DocumentsContract.Root.COLUMN_DOCUMENT_ID, "root");
        put(row, cursor, DocumentsContract.Root.COLUMN_TITLE, "MSIME OCR 测试模块");
        put(row, cursor, DocumentsContract.Root.COLUMN_FLAGS, DocumentsContract.Root.FLAG_LOCAL_ONLY);
        put(row, cursor, DocumentsContract.Root.COLUMN_MIME_TYPES, "application/vnd.android.package-archive");
        return cursor;
    }

    @Override public Cursor queryDocument(String id, String[] projection) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(projection == null ? DOCUMENT_COLUMNS : projection);
        add(cursor, id);
        return cursor;
    }

    @Override public Cursor queryChildDocuments(String parentId, String[] projection, String sortOrder)
            throws FileNotFoundException {
        if (!parentId.equals("root")) throw new FileNotFoundException("unknown synthetic parent");
        MatrixCursor cursor = new MatrixCursor(projection == null ? DOCUMENT_COLUMNS : projection);
        add(cursor, "valid");
        add(cursor, "corrupt");
        return cursor;
    }

    @Override public ParcelFileDescriptor openDocument(String id, String mode, CancellationSignal signal)
            throws FileNotFoundException {
        if (!mode.equals("r")) throw new FileNotFoundException("synthetic module is readonly");
        return ParcelFileDescriptor.open(file(id), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    private synchronized File file(String id) throws FileNotFoundException {
        if (!id.equals("valid") && !id.equals("corrupt"))
            throw new FileNotFoundException("unknown synthetic module");
        File directory = new File(getContext().getCacheDir(), "ocr-picker-fixtures");
        if (!directory.isDirectory() && !directory.mkdir())
            throw new FileNotFoundException("synthetic module directory");
        File target = new File(directory, id + ".apk");
        if (target.isFile()) return target;
        try {
            try (var input = getContext().getAssets().open("ocr-product-module.apk");
                    var output = new FileOutputStream(target)) {
                byte[] buffer = new byte[8192];
                int count;
                while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            }
            if (id.equals("corrupt")) {
                try (var output = new RandomAccessFile(target, "rw")) {
                    int first = output.read();
                    output.seek(0);
                    output.write(first ^ 0xff);
                }
            }
        } catch (IOException failure) {
            if (target.exists() && !target.delete())
                throw new FileNotFoundException("synthetic partial module cleanup");
            throw new FileNotFoundException("synthetic module preparation");
        }
        return target;
    }

    private void add(MatrixCursor cursor, String id) throws FileNotFoundException {
        boolean root = id.equals("root");
        File source = root ? null : file(id);
        MatrixCursor.RowBuilder row = cursor.newRow();
        put(row, cursor, DocumentsContract.Document.COLUMN_DOCUMENT_ID, id);
        put(row, cursor, DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            root ? "MSIME OCR 测试模块" : "msime-synthetic-ocr-" + id + ".apk");
        put(row, cursor, DocumentsContract.Document.COLUMN_MIME_TYPE,
            root ? DocumentsContract.Document.MIME_TYPE_DIR : "application/vnd.android.package-archive");
        put(row, cursor, DocumentsContract.Document.COLUMN_FLAGS, 0);
        put(row, cursor, DocumentsContract.Document.COLUMN_SIZE, root ? 0L : source.length());
    }

    private static void put(MatrixCursor.RowBuilder row, MatrixCursor cursor, String name, Object value) {
        if (cursor.getColumnIndex(name) >= 0) row.add(name, value);
    }
}
