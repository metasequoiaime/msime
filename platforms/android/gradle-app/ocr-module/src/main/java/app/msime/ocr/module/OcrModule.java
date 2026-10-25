package app.msime.ocr.module;

import ai.onnxruntime.OnnxTensor;
import ai.onnxruntime.OrtEnvironment;
import ai.onnxruntime.OrtException;
import ai.onnxruntime.OrtLoggingLevel;
import ai.onnxruntime.OrtSession;
import ai.onnxruntime.TensorInfo;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapShader;
import android.graphics.Canvas;
import android.graphics.Matrix;
import android.graphics.Paint;
import android.graphics.Shader;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.FloatBuffer;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * 可选 OCR 模块的公开接口（接口版本 1）：ONNX Runtime 上跑 PP-OCRv6 tiny 的检测和识别，只跨越 Android 平台类型，不把 ONNX Runtime 类型带进宿主。
 *
 * <p>模型和字典从模块自己的 assets 读进内存交给 ONNX Runtime，输入法进程里不写任何文件；也不联网，见 {@link #environment()}。前后处理在 {@link OcrPipeline}。
 */
public final class OcrModule {
    private static OrtEnvironment environment;
    private OrtSession detector;
    private OrtSession recognizer;
    private String[] dictionary;

    public int interfaceVersion() { return 1; }

    /** 建两个推理会话，必须在工作线程调用；重复调用不重建。失败时已建的会话一并释放。 */
    public void initialize(Context context) throws Exception {
        if (detector != null) return;
        OrtEnvironment ort = environment();
        String[] entries = OcrPipeline.dictionary(new String(asset(context, "ocr/dict.txt"), StandardCharsets.UTF_8));
        try (OrtSession.SessionOptions options = sessionOptions()) {
            detector = ort.createSession(asset(context, "ocr/det.onnx"), options);
            recognizer = ort.createSession(asset(context, "ocr/rec.onnx"), options);
            long[] shape = ((TensorInfo) recognizer.getOutputInfo().values().iterator().next().getInfo()).getShape();
            // 识别模型的类别数必须是字典条目加空白和空格两个，否则字典与模型不是同一提交。
            if (shape.length != 3 || shape[2] != entries.length + 2) throw new IllegalStateException("ocr_module_dictionary");
        } catch (Exception | LinkageError error) {
            close();
            throw error;
        }
        dictionary = entries;
    }

    /**
     * 进程内（同一个模块类加载器内）只有一个 ONNX Runtime 环境，模块实例之间共用，不随 {@link #close()} 释放。
     *
     * <p>创建时显式关掉遥测。onnxruntime-android 自带向 Microsoft 上报的 1DS 遥测，靠 AAR 清单里的 `TelemetryInitializer` 在应用启动时初始化 Java 端的发送通道；模块不是被安装的应用，那个 Provider 在宿主里从不运行，模块清单也把它删掉了，这里再关掉原生一侧，不依赖 1.31.0 里通道初始化失败这一缺陷。
     */
    private static synchronized OrtEnvironment environment() throws OrtException {
        if (environment == null) {
            OrtEnvironment created = OrtEnvironment.getEnvironment(OrtLoggingLevel.ORT_LOGGING_LEVEL_WARNING, "msime-ocr");
            created.setTelemetry(false);
            environment = created;
        }
        return environment;
    }

    /**
     * 会话参数。
     *
     * <ul>
     *   <li>关掉 CPU 内存池和内存模式：开着时输入法进程的峰值 PSS 是 330–440 MB，关掉后 121 MB，延迟几乎不变。</li>
     *   <li>`mlas.disable_kleidiai`：Apple 芯片上的 arm64 模拟器（包括本项目的 CI）报告 SME 特性却不支持，KleidiAI 内核在建会话时执行 `rdsvl` 触发 SIGILL，整个输入法进程崩溃。</li>
     *   <li>线程数限在 2–4，空闲时不自旋：识别在输入法进程里偶尔跑一次，自旋等待只会在识别结束后继续占 CPU。</li>
     * </ul>
     */
    private static OrtSession.SessionOptions sessionOptions() throws OrtException {
        OrtSession.SessionOptions options = new OrtSession.SessionOptions();
        try {
            options.setOptimizationLevel(OrtSession.SessionOptions.OptLevel.ALL_OPT);
            options.setCPUArenaAllocator(false);
            options.setMemoryPatternOptimization(false);
            options.addConfigEntry("mlas.disable_kleidiai", "1");
            options.setIntraOpNumThreads(Math.max(2, Math.min(4, Runtime.getRuntime().availableProcessors())));
            options.setInterOpNumThreads(1);
            options.addConfigEntry("session.intra_op.allow_spinning", "0");
            return options;
        } catch (OrtException error) {
            options.close();
            throw error;
        }
    }

    /** 调用方必须使用工作线程；返回按阅读顺序排列的文本行，识别文本从不写入日志。 */
    public String[] recognize(Bitmap bitmap) throws Exception {
        if (detector == null) throw new IllegalStateException("ocr_module_not_initialized");
        Bitmap source = bitmap.getConfig() == Bitmap.Config.HARDWARE ? bitmap.copy(Bitmap.Config.ARGB_8888, false) : bitmap;
        try {
            ArrayList<String> lines = new ArrayList<>();
            for (float[] quad : OcrPipeline.readingOrder(detect(source))) {
                Bitmap line = crop(source, quad);
                if (line == null) continue;
                try {
                    String text = recognizeLine(line).trim();
                    if (!text.isEmpty()) lines.add(text);
                } finally {
                    line.recycle();
                }
            }
            return lines.toArray(new String[0]);
        } finally {
            if (source != bitmap) source.recycle();
        }
    }

    public void close() throws OrtException {
        OrtSession detecting = detector, recognizing = recognizer;
        detector = null;
        recognizer = null;
        dictionary = null;
        try {
            if (detecting != null) detecting.close();
        } finally {
            if (recognizing != null) recognizing.close();
        }
    }

    private List<float[]> detect(Bitmap source) throws OrtException {
        int width = source.getWidth(), height = source.getHeight();
        int[] size = OcrPipeline.detectionSize(width, height);
        int[] pixels = pixels(source, size[0], size[1]);
        float[] input = OcrPipeline.detectionInput(pixels, size[0], size[1]);
        float[] probability = new float[size[0] * size[1]];
        try (OnnxTensor tensor = OnnxTensor.createTensor(environment, FloatBuffer.wrap(input), new long[]{1, 3, size[1], size[0]});
             OrtSession.Result result = detector.run(Collections.singletonMap(detector.getInputNames().iterator().next(), tensor))) {
            FloatBuffer output = ((OnnxTensor) result.get(0)).getFloatBuffer();
            if (output.remaining() != probability.length) throw new IllegalStateException("ocr_detection_output");
            output.get(probability);
        }
        return OcrPipeline.boxes(probability, size[0], size[1], width, height);
    }

    /** 透视裁出一行，边界外按最近的边缘像素延伸（上游的 BORDER_REPLICATE）；竖排行逆时针转 90°，让字序从左到右。 */
    private static Bitmap crop(Bitmap source, float[] quad) {
        int[] size = OcrPipeline.cropSize(quad);
        Matrix matrix = new Matrix();
        // 着色器的局部矩阵把原图坐标映射到行图坐标，即四边形到行图矩形的透视变换。
        if (!matrix.setPolyToPoly(quad, 0, new float[]{0, 0, size[0], 0, size[0], size[1], 0, size[1]}, 0, 4)) return null;
        BitmapShader shader = new BitmapShader(source, Shader.TileMode.CLAMP, Shader.TileMode.CLAMP);
        shader.setLocalMatrix(matrix);
        Paint paint = new Paint(Paint.FILTER_BITMAP_FLAG);
        paint.setShader(shader);
        Bitmap line = Bitmap.createBitmap(size[0], size[1], Bitmap.Config.ARGB_8888);
        new Canvas(line).drawRect(0, 0, size[0], size[1], paint);
        if (!OcrPipeline.vertical(quad)) return line;
        Matrix rotation = new Matrix();
        rotation.postRotate(-90);
        Bitmap rotated = Bitmap.createBitmap(line, 0, 0, size[0], size[1], rotation, true);
        if (rotated != line) line.recycle();
        return rotated;
    }

    private String recognizeLine(Bitmap line) throws OrtException {
        int width = OcrPipeline.resizedWidth(line.getWidth(), line.getHeight());
        int tensorWidth = OcrPipeline.recognitionWidth(line.getWidth(), line.getHeight());
        float[] input = OcrPipeline.recognitionInput(pixels(line, width, OcrPipeline.REC_HEIGHT), width, tensorWidth);
        try (OnnxTensor tensor = OnnxTensor.createTensor(environment, FloatBuffer.wrap(input),
                 new long[]{1, 3, OcrPipeline.REC_HEIGHT, tensorWidth});
             OrtSession.Result result = recognizer.run(Collections.singletonMap(recognizer.getInputNames().iterator().next(), tensor))) {
            OnnxTensor output = (OnnxTensor) result.get(0);
            long[] shape = output.getInfo().getShape();
            if (shape.length != 3 || shape[0] != 1) throw new IllegalStateException("ocr_recognition_output");
            int steps = (int) shape[1], classes = (int) shape[2];
            float[] logits = new float[steps * classes];
            output.getFloatBuffer().get(logits);
            return OcrPipeline.decode(logits, steps, classes, dictionary);
        }
    }

    private static int[] pixels(Bitmap source, int width, int height) {
        Bitmap scaled = source.getWidth() == width && source.getHeight() == height ? source
            : Bitmap.createScaledBitmap(source, width, height, true);
        try {
            int[] pixels = new int[width * height];
            scaled.getPixels(pixels, 0, width, 0, 0, width, height);
            return pixels;
        } finally {
            if (scaled != source) scaled.recycle();
        }
    }

    private static byte[] asset(Context context, String name) throws Exception {
        try (InputStream input = context.getAssets().open(name)) {
            ByteArrayOutputStream output = new ByteArrayOutputStream(Math.max(8192, input.available()));
            byte[] buffer = new byte[1 << 16];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            return output.toByteArray();
        }
    }
}
