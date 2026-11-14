package app.msime.android.test;

import android.app.Instrumentation;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.net.Uri;
import android.os.Bundle;
import android.os.Parcel;
import android.os.Process;
import android.util.Base64;
import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.lang.reflect.InvocationTargetException;
import org.json.JSONObject;

/** 在真实 Binder 和管道中验证大照片，使用独立测试包和合成内容。 */
public final class CommunitySkinDownloadDeviceChecks {
    private CommunitySkinDownloadDeviceChecks() {}

    static void run(Instrumentation owner) {
        Bundle result = new Bundle();
        try {
            var resolver = owner.getContext().getContentResolver();
            Uri uri = Uri.parse("content://" + owner.getContext().getPackageName() + ".skin-download/large");
            Bundle identity = resolver.call(uri, "identity", null, null);
            if (identity == null || identity.getInt("pid") == Process.myPid())
                throw new AssertionError("Skin fixture did not cross a process boundary");
            var read = CommunitySkinDownloadTestProvider.transport(owner.getContext())
                .getMethod("read", InputStream.class);
            String expected = CommunitySkinDownloadTestProvider.photo();
            byte[] image = Base64.decode(expected, Base64.DEFAULT);
            Bitmap decoded = BitmapFactory.decodeByteArray(image, 0, image.length);
            if (decoded == null) throw new AssertionError("Synthetic PNG is invalid");
            decoded.recycle();
            Bundle oldReply = new Bundle();
            oldReply.putString("skin_design", new JSONObject().put("photo", expected).toString());
            Parcel parcel = Parcel.obtain();
            try {
                parcel.writeBundle(oldReply);
                if (parcel.dataSize() <= 1024 * 1024)
                    throw new AssertionError("Synthetic photo does not exceed the string reply budget");
            } finally { parcel.recycle(); }
            JSONObject reply;
            try (var input = resolver.openInputStream(uri)) {
                reply = (JSONObject) read.invoke(null, input);
            }
            if (!expected.equals(reply.getJSONObject("design").getString("photo")))
                throw new AssertionError("Pipe changed the complete photo");
            try (var input = resolver.openInputStream(uri.buildUpon().path("/failure").build())) {
                reply = (JSONObject) read.invoke(null, input);
            }
            if (!"合成下载失败".equals(reply.getString("error")) || reply.optJSONObject("design") != null)
                throw new AssertionError("Failure reply became an installable skin");
            for (byte[] invalid : new byte[][] {new byte[0], new byte[1024 * 1024 + 1]}) {
                try {
                    read.invoke(null, new ByteArrayInputStream(invalid));
                    throw new AssertionError("Invalid skin stream accepted");
                } catch (InvocationTargetException expectedFailure) {
                    Throwable cause = expectedFailure.getCause();
                    if (!(cause instanceof java.io.IOException) && !(cause instanceof org.json.JSONException))
                        throw expectedFailure;
                }
            }
            boolean denied = false;
            Uri product = Uri.parse("content://app.msime.android.account-session/skin-download/00000000-0000-4000-8000-000000000001");
            try (var input = resolver.openInputStream(product)) {
                if (input != null) throw new AssertionError("Another UID opened the product download provider");
            } catch (SecurityException expectedFailure) { denied = true; }
            if (!denied) throw new AssertionError("Cross-UID request was not explicitly denied");
            result.putString("stream", "MSIME_SKIN_DOWNLOAD_STREAM_PASSED: complete synthetic photo crosses a remote pipe, failures and oversized replies are refused, product provider rejects another UID\n");
            owner.finish(0, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: skin download stream "
                + error.getClass().getSimpleName() + ": " + error.getMessage() + "\n");
            owner.finish(1, result);
        }
    }
}
