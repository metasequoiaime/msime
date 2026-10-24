package app.msime.android.test;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.Process;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;

/** 仅测试 APK 提供的编辑器探针；同签名的被测宿主只可读取焦点、边界或切换合成输入框。 */
public final class EditorProbeProvider extends ContentProvider {
    static final Uri URI = Uri.parse("content://app.msime.android.test.editor-probe");

    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String field, Bundle extras) {
        if (!"app.msime.android".equals(getCallingPackage())
                || getContext().getPackageManager().checkSignatures(Binder.getCallingUid(), Process.myUid())
                    != PackageManager.SIGNATURE_MATCH)
            throw new SecurityException("Only the signed synthetic test target may probe its editor");
        if (!"snapshot".equals(method) && !"focus".equals(method))
            throw new IllegalArgumentException("Unknown synthetic editor probe method");
        FutureTask<Bundle> task = new FutureTask<>(() -> EditorActivity.probe(field, "focus".equals(method)));
        new Handler(Looper.getMainLooper()).post(task);
        try {
            return task.get(5, TimeUnit.SECONDS);
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("Synthetic editor probe was interrupted", error);
        } catch (java.util.concurrent.ExecutionException | java.util.concurrent.TimeoutException error) {
            task.cancel(false);
            throw new IllegalStateException("Synthetic editor probe did not complete", error);
        }
    }

    @Override public String getType(Uri uri) { return null; }
    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] selectionArgs, String sortOrder) { return null; }
    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri uri, String selection, String[] selectionArgs) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) { throw new UnsupportedOperationException(); }
}
