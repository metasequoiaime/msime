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
            String[][] cases = new String[SyntheticOcrFixture.CASE_COUNT][];
            int completeCases = 0;
            try (TextRecognizer recognizer = TextRecognition.getClient(new ChineseTextRecognizerOptions.Builder().build())) {
                for (int index = 0; index < cases.length; index++) {
                    Bitmap bitmap = SyntheticOcrFixture.render(index);
                    try {
                        SyntheticOcrFixture.saveImage(getTargetContext(), bitmap, index);
                        Text recognized = Tasks.await(recognizer.process(InputImage.fromBitmap(bitmap, 0)), 30, TimeUnit.SECONDS);
                        ArrayList<String> lines = new ArrayList<>();
                        for (Text.TextBlock block : recognized.getTextBlocks())
                            for (Text.Line line : block.getLines()) lines.add(line.getText().trim());
                        cases[index] = lines.toArray(new String[0]);
                        boolean matches = SyntheticOcrFixture.matches(cases[index]);
                        if (matches) completeCases++;
                        String exact = String.join("", cases[index]).replaceAll("\\s", "");
                        android.util.Log.i("MSIME_OCR_DIRECT", "synthetic_case=" + index + ",complete=" + matches
                            + ",exact-simplified=" + (exact.contains("SYNTHETICOCR2468") && exact.contains("中文测试")));
                    } finally {
                        bitmap.recycle();
                    }
                }
                SyntheticOcrFixture.saveResult(getTargetContext(), cases);
            }
            result.putString("stream", (completeCases > 0 ? "MSIME_OCR_DIRECT_PASSED" : "MSIME_OCR_DIRECT_FAILED")
                + ": complete_cases=" + completeCases + "/" + cases.length + "\n");
            finish(completeCases > 0 ? -1 : 1, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_OCR_DIRECT_FAILED: " + error.getClass().getName() + "\n");
            finish(1, result);
        }
    }
}
