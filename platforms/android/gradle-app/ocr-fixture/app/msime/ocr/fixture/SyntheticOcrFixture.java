package app.msime.ocr.fixture;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import org.json.JSONArray;

/** 两种加载方式共用的固定合成输入；不接受文件、相机或编辑器内容。 */
public final class SyntheticOcrFixture {
    private SyntheticOcrFixture() {}

    public static Bitmap render() {
        Bitmap bitmap = Bitmap.createBitmap(1400, 360, Bitmap.Config.ARGB_8888);
        Canvas canvas = new Canvas(bitmap);
        canvas.drawColor(Color.WHITE);
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setColor(Color.BLACK);
        paint.setTextSize(88);
        canvas.drawText("SYNTHETIC OCR 2468", 40, 125, paint);
        canvas.drawText("中文测试", 40, 270, paint);
        return bitmap;
    }

    public static void saveImage(Context context, Bitmap bitmap) throws Exception {
        try (FileOutputStream output = new FileOutputStream(new File(context.getFilesDir(), "ocr-synthetic.png"))) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("probe_synthetic_image");
        }
    }

    /** 仅用于对照固定合成图的测试产物；此类不进入模块的 release 构建。 */
    public static void saveResult(Context context, String[] lines) throws Exception {
        JSONArray result = new JSONArray();
        for (String line : lines) result.put(line);
        try (FileOutputStream output = new FileOutputStream(new File(context.getFilesDir(), "ocr-synthetic-result.json"))) {
            output.write(result.toString().getBytes(StandardCharsets.UTF_8));
        }
    }
}
