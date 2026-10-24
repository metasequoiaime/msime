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
    public static final int CASE_COUNT = 3;
    private SyntheticOcrFixture() {}

    public static Bitmap render(int index) {
        if (index < 0 || index >= CASE_COUNT) throw new IllegalArgumentException("probe_fixture_index");
        Bitmap bitmap = Bitmap.createBitmap(1400 >> index, 360 >> index, Bitmap.Config.ARGB_8888);
        Canvas canvas = new Canvas(bitmap);
        canvas.scale(1f / (1 << index), 1f / (1 << index));
        canvas.drawColor(Color.WHITE);
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setColor(Color.BLACK);
        paint.setTextSize(88);
        canvas.drawText("SYNTHETIC OCR 2468", 40, 125, paint);
        canvas.drawText("中文测试", 40, 270, paint);
        return bitmap;
    }

    public static void saveImage(Context context, Bitmap bitmap, int index) throws Exception {
        try (FileOutputStream output = new FileOutputStream(new File(context.getFilesDir(), "ocr-synthetic-" + index + ".png"))) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("probe_synthetic_image");
        }
    }

    /** 仅用于对照固定合成图的测试产物；此类不进入模块的 release 构建。 */
    public static void saveResult(Context context, String[][] cases) throws Exception {
        JSONArray result = new JSONArray();
        for (String[] lines : cases) {
            JSONArray entry = new JSONArray();
            for (String line : lines) entry.put(line);
            result.put(entry);
        }
        try (FileOutputStream output = new FileOutputStream(new File(context.getFilesDir(), "ocr-synthetic-result.json"))) {
            output.write(result.toString().getBytes(StandardCharsets.UTF_8));
        }
    }

    /** 每个合格用例都必须同时包含完整英文单词、数字和中文；不拼接不同图片的局部成功。 */
    public static boolean matches(String[] lines) {
        String joined = String.join("", lines).replaceAll("\\s", "");
        // 同一中文词的简繁字形均有效；文/丈、O/0 等误识别仍然拒绝。
        boolean chinese = joined.contains("中文测试") || joined.contains("中文測试")
            || joined.contains("中文测試") || joined.contains("中文測試");
        return joined.contains("SYNTHETICOCR2468") && chinese;
    }
}
