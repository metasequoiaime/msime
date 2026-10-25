package app.msime.ocr.module;

import android.app.Instrumentation;
import android.graphics.Bitmap;
import android.os.Bundle;
import app.msime.ocr.fixture.SyntheticOcrFixture;

/** 普通安装方式下的对照：同一个 OcrModule 由系统类加载器加载，仅编进 debug 模块。 */
public final class DirectOcrProbe extends Instrumentation {
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            String[][] cases = new String[SyntheticOcrFixture.CASE_COUNT + 1][];
            int completeCases = 0;
            OcrModule module = new OcrModule();
            try {
                module.initialize(getTargetContext());
                for (int index = 0; index < SyntheticOcrFixture.CASE_COUNT; index++) {
                    Bitmap bitmap = SyntheticOcrFixture.render(index);
                    try {
                        SyntheticOcrFixture.saveImage(getTargetContext(), bitmap, index);
                        cases[index] = module.recognize(bitmap);
                        boolean matches = SyntheticOcrFixture.matches(cases[index]);
                        if (matches) completeCases++;
                        String exact = String.join("", cases[index]).replaceAll("\\s", "");
                        android.util.Log.i("MSIME_OCR_DIRECT", "synthetic_case=" + index + ",complete=" + matches
                            + ",exact-simplified=" + (exact.contains("SYNTHETICOCR2468") && exact.contains("中文测试")));
                    } finally {
                        bitmap.recycle();
                    }
                }
                Bitmap columns = SyntheticOcrFixture.renderVertical();
                try {
                    SyntheticOcrFixture.saveImage(getTargetContext(), columns, SyntheticOcrFixture.CASE_COUNT);
                    cases[SyntheticOcrFixture.CASE_COUNT] = module.recognize(columns);
                } finally {
                    columns.recycle();
                }
                SyntheticOcrFixture.saveResult(getTargetContext(), cases);
            } finally {
                module.close();
            }
            boolean vertical = SyntheticOcrFixture.verticalOrder(cases[SyntheticOcrFixture.CASE_COUNT]);
            android.util.Log.i("MSIME_OCR_DIRECT", "synthetic_vertical_right_to_left=" + vertical);
            boolean passed = completeCases > 0 && vertical;
            result.putString("stream", (passed ? "MSIME_OCR_DIRECT_PASSED" : "MSIME_OCR_DIRECT_FAILED")
                + ": complete_cases=" + completeCases + "/" + SyntheticOcrFixture.CASE_COUNT
                + ",vertical_right_to_left=" + vertical + "\n");
            finish(passed ? -1 : 1, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_OCR_DIRECT_FAILED: " + error.getClass().getName() + "\n");
            finish(1, result);
        }
    }
}
