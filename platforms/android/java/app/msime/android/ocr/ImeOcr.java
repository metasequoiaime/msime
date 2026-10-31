package app.msime.android;

import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Matrix;
import android.media.ExifInterface;
import android.os.Bundle;
import android.os.ResultReceiver;
import java.io.ByteArrayInputStream;

/** 独立识字窗口只持有回传通道；原编辑器连接与上下文留在输入法会话中。 */
final class ImeOcr {
    private final MSIMEInputService s;
    private long generation;
    private EditorContextSnapshot target;
    private ResultReceiver window;

    ImeOcr(MSIMEInputService service) { s = service; }

    @SuppressWarnings("deprecation")
    void open() {
        close();
        s.closeToolbarPanels();
        if (s.connection == null) return;
        s.command(MSIMEInputService.FINISH_COMPOSITION_COMMAND);
        String before = s.editorContext(true), after = s.editorContext(false);
        if (before == null || after == null) {
            s.notice("无法确认原输入位置，请回到输入框重试");
            return;
        }
        target = new EditorContextSnapshot(s.connection, s.editorContextRevision,
            before, s.selectedEditorText(), after);
        long expected = ++generation;
        ResultReceiver receiver = new ResultReceiver(s.main) {
            @Override protected void onReceiveResult(int code, Bundle payload) {
                ResultReceiver reply = payload == null ? null
                    : payload.getParcelable(OcrCaptureActivity.REPLY);
                boolean current = expected == generation && target != null;
                if (code == OcrCaptureActivity.REGISTER) {
                    if (current) window = reply;
                    else if (reply != null) reply.send(OcrCaptureActivity.INVALID, Bundle.EMPTY);
                    return;
                }
                if (code == OcrCaptureActivity.CANCEL) {
                    if (current) close();
                    return;
                }
                if (code != OcrCaptureActivity.INSERT || reply == null) return;
                String text = payload.getString(OcrCaptureActivity.TEXT, "");
                boolean accepted = false;
                if (current) {
                    boolean matches = !text.isEmpty() && text.length() <= OcrCaptureActivity.TEXT_LIMIT
                        && target.matches(s.connection, s.editorContextRevision,
                            s.editorContext(true), s.selectedEditorText(), s.editorContext(false));
                    // 先作废请求，编辑器拒绝或重复回调都不能再投递同一份结果。
                    generation++;
                    target = null;
                    window = null;
                    if (matches) accepted = s.commitText(text, TypingSource.LOCAL);
                }
                reply.send(accepted ? OcrCaptureActivity.ACCEPTED : OcrCaptureActivity.INVALID, Bundle.EMPTY);
            }
        };
        try {
            s.startActivity(new Intent(s, OcrCaptureActivity.class)
                .putExtra(OcrCaptureActivity.RECEIVER, receiver)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS));
        } catch (RuntimeException unavailable) {
            close();
            s.notice("拍照识字窗口无法打开");
        }
    }

    void close() {
        generation++;
        target = null;
        ResultReceiver previous = window;
        window = null;
        if (previous != null) previous.send(OcrCaptureActivity.INVALID, Bundle.EMPTY);
    }

    void destroy() { close(); }

    /** 限制解码内存并按 JPEG 的方向标签旋转，不落盘。 */
    static Bitmap decode(byte[] jpeg) throws Exception {
        BitmapFactory.Options options = new BitmapFactory.Options();
        options.inJustDecodeBounds = true;
        BitmapFactory.decodeByteArray(jpeg, 0, jpeg.length, options);
        if (options.outWidth <= 0 || options.outHeight <= 0) throw new IllegalStateException("ocr_photo");
        options.inSampleSize = 1;
        while (Math.max(options.outWidth, options.outHeight) / options.inSampleSize > 2048) options.inSampleSize *= 2;
        options.inJustDecodeBounds = false;
        Bitmap bitmap = BitmapFactory.decodeByteArray(jpeg, 0, jpeg.length, options);
        if (bitmap == null) throw new IllegalStateException("ocr_photo");
        int orientation = new ExifInterface(new ByteArrayInputStream(jpeg))
            .getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL);
        Matrix matrix = new Matrix();
        switch (orientation) {
            case ExifInterface.ORIENTATION_FLIP_HORIZONTAL -> matrix.setScale(-1, 1);
            case ExifInterface.ORIENTATION_ROTATE_180 -> matrix.setRotate(180);
            case ExifInterface.ORIENTATION_FLIP_VERTICAL -> matrix.setScale(1, -1);
            case ExifInterface.ORIENTATION_TRANSPOSE -> { matrix.setRotate(90); matrix.postScale(-1, 1); }
            case ExifInterface.ORIENTATION_ROTATE_90 -> matrix.setRotate(90);
            case ExifInterface.ORIENTATION_TRANSVERSE -> { matrix.setRotate(270); matrix.postScale(-1, 1); }
            case ExifInterface.ORIENTATION_ROTATE_270 -> matrix.setRotate(270);
            default -> { return bitmap; }
        }
        Bitmap rotated = Bitmap.createBitmap(bitmap, 0, 0, bitmap.getWidth(), bitmap.getHeight(), matrix, true);
        if (rotated != bitmap) bitmap.recycle();
        return rotated;
    }

}
