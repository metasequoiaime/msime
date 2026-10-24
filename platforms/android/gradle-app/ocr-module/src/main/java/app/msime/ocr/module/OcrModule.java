package app.msime.ocr.module;

import android.content.Context;
import android.graphics.Bitmap;
import com.google.android.gms.tasks.Tasks;
import com.google.mlkit.common.MlKit;
import com.google.mlkit.vision.common.InputImage;
import com.google.mlkit.vision.text.Text;
import com.google.mlkit.vision.text.TextRecognition;
import com.google.mlkit.vision.text.TextRecognizer;
import com.google.mlkit.vision.text.chinese.ChineseTextRecognizerOptions;
import java.util.ArrayList;
import java.util.concurrent.TimeUnit;

/** 可选模块的试验接口，只跨越平台类型，不把 Google 类型带入宿主。 */
public final class OcrModule {
    private static boolean mlKitInitialized;
    private TextRecognizer recognizer;

    public int interfaceVersion() { return 1; }

    public void initialize(Context context) {
        if (recognizer != null) return;
        initializeMlKit(context);
        recognizer = TextRecognition.getClient(new ChineseTextRecognizerOptions.Builder().build());
    }

    /** 同一模块类加载器复用公开容器；关闭识别器后重开不能再次初始化 SDK。 */
    private static synchronized void initializeMlKit(Context context) {
        if (mlKitInitialized) return;
        MlKit.initialize(context);
        mlKitInitialized = true;
    }

    /** 调用方必须使用工作线程；识别文本从不写入日志。 */
    public String[] recognize(Bitmap bitmap) throws Exception {
        if (recognizer == null) throw new IllegalStateException("ocr_module_not_initialized");
        Text result = Tasks.await(recognizer.process(InputImage.fromBitmap(bitmap, 0)),
            30, TimeUnit.SECONDS);
        ArrayList<String> lines = new ArrayList<>();
        for (Text.TextBlock block : result.getTextBlocks()) {
            for (Text.Line line : block.getLines()) {
                String value = line.getText().trim();
                if (!value.isEmpty()) lines.add(value);
            }
        }
        return lines.toArray(new String[0]);
    }

    public void close() {
        if (recognizer == null) return;
        recognizer.close();
        recognizer = null;
    }
}
