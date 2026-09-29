package app.msime.android;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import androidx.work.Configuration;
import androidx.work.WorkManager;
import com.google.mlkit.common.MlKit;

/** Initializes packaged handwriting dependencies inside the isolated IME process. */
public final class MlKitImeInitProvider extends ContentProvider {
    @Override public boolean onCreate() {
        Context context = getContext();
        if (context == null) return false;
        try {
            WorkManager.getInstance(context);
        } catch (IllegalStateException uninitialized) {
            WorkManager.initialize(context, new Configuration.Builder().build());
        }
        MlKit.initialize(context);
        return true;
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection,
                                  String[] selectionArgs, String sortOrder) {
        throw new UnsupportedOperationException();
    }

    @Override public String getType(Uri uri) { return null; }

    @Override public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException();
    }

    @Override public int delete(Uri uri, String selection, String[] selectionArgs) {
        throw new UnsupportedOperationException();
    }

    @Override public int update(Uri uri, ContentValues values, String selection,
                                String[] selectionArgs) {
        throw new UnsupportedOperationException();
    }
}
