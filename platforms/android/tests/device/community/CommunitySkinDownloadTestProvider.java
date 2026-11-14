package app.msime.android.test;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.net.Uri;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.os.Process;
import android.util.Base64;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.FileNotFoundException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.zip.CRC32;
import org.json.JSONObject;

/** 独立进程中的合成皮肤回复，不联网、不读取产品账号和偏好。 */
public final class CommunitySkinDownloadTestProvider extends ContentProvider {
    static Class<?> transport(Context context) throws Exception {
        return context.createPackageContext("app.msime.android",
            Context.CONTEXT_INCLUDE_CODE | Context.CONTEXT_IGNORE_SECURITY).getClassLoader()
            .loadClass("app.msime.android.CommunitySkinDownloadStream");
    }

    /** 1×1 PNG 加有界的合成文本块，照片仍在共享的 512000 字节上限内。 */
    static String photo() throws Exception {
        Bitmap bitmap = Bitmap.createBitmap(1, 1, Bitmap.Config.ARGB_8888);
        ByteArrayOutputStream png = new ByteArrayOutputStream();
        try { bitmap.compress(Bitmap.CompressFormat.PNG, 100, png); }
        finally { bitmap.recycle(); }
        byte[] original = png.toByteArray();
        ByteArrayOutputStream result = new ByteArrayOutputStream();
        result.write(original, 0, original.length - 12);
        byte[] text = new byte[500000];
        Arrays.fill(text, (byte) 'A');
        byte[] keyword = "Synthetic\0".getBytes(StandardCharsets.US_ASCII);
        System.arraycopy(keyword, 0, text, 0, keyword.length);
        byte[] type = "tEXt".getBytes(StandardCharsets.US_ASCII);
        DataOutputStream output = new DataOutputStream(result);
        output.writeInt(text.length);
        output.write(type);
        output.write(text);
        CRC32 crc = new CRC32();
        crc.update(type);
        crc.update(text);
        output.writeInt((int) crc.getValue());
        output.write(original, original.length - 12, 12);
        if (result.size() > 512000) throw new AssertionError("Synthetic photo exceeds shared limit");
        return Base64.encodeToString(result.toByteArray(), Base64.NO_WRAP);
    }

    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        Bundle reply = new Bundle();
        reply.putInt("pid", Process.myPid());
        return reply;
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        try {
            JSONObject design = "failure".equals(uri.getLastPathSegment())
                ? null : new JSONObject().put("photo", photo());
            String failure = design == null ? "合成下载失败" : "";
            return (ParcelFileDescriptor) transport(getContext()).getMethod("open",
                ContentProvider.class, Uri.class, JSONObject.class, String.class)
                .invoke(null, this, uri, design, failure);
        } catch (Exception error) {
            throw new FileNotFoundException(error.getClass().getSimpleName());
        }
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] args, String sort) { throw new UnsupportedOperationException(); }
    @Override public String getType(Uri uri) { return "application/json"; }
    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri uri, String selection, String[] args) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] args) { throw new UnsupportedOperationException(); }
}
