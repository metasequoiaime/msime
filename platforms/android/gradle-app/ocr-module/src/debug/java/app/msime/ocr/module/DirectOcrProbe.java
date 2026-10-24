package app.msime.ocr.module;

import android.app.Instrumentation;
import android.graphics.Bitmap;
import android.os.Bundle;
import app.msime.ocr.fixture.SyntheticOcrFixture;
import com.google.android.gms.tasks.Tasks;
import com.google.mlkit.vision.common.InputImage;
import com.google.mlkit.vision.text.Text;
import com.google.mlkit.vision.text.TextRecognition;
import com.google.mlkit.vision.text.TextRecognizer;
import com.google.mlkit.vision.text.chinese.ChineseTextRecognizerOptions;
import java.util.ArrayList;
import java.util.concurrent.TimeUnit;

/** 普通捆绑 SDK 的对照，仅编进 debug 模块，不重置或操作任何私有容器。 */
public final class DirectOcrProbe extends Instrumentation {
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            Bitmap bitmap = SyntheticOcrFixture.render();
            try (TextRecognizer recognizer = TextRecognition.getClient(new ChineseTextRecognizerOptions.Builder().build())) {
                SyntheticOcrFixture.saveImage(getTargetContext(), bitmap);
                Text recognized = Tasks.await(recognizer.process(InputImage.fromBitmap(bitmap, 0)), 30, TimeUnit.SECONDS);
                ArrayList<String> lines = new ArrayList<>();
                for (Text.TextBlock block : recognized.getTextBlocks())
                    for (Text.Line line : block.getLines()) lines.add(line.getText().trim());
                SyntheticOcrFixture.saveResult(getTargetContext(), lines.toArray(new String[0]));
                String joined = String.join("", lines).replaceAll("\\s", "");
                boolean english = joined.contains("SYNTHETICOCR2468");
                boolean chinese = joined.contains("中文测试");
                result.putString("stream", "MSIME_OCR_DIRECT_RESULT: english=" + english + ",chinese=" + chinese + "\n");
                finish(english && chinese ? -1 : 1, result);
            } finally {
                bitmap.recycle();
            }
        } catch (Throwable error) {
            result.putString("stream", "MSIME_OCR_DIRECT_FAILED: " + error.getClass().getName() + "\n");
            finish(1, result);
        }
    }
}
