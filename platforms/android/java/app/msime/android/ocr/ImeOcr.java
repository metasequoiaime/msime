package app.msime.android;

import android.Manifest;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Matrix;
import android.media.ExifInterface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.io.ByteArrayInputStream;
import java.util.ArrayList;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;

/** 键盘内拍照、离线识别与选字；面板、输入会话和异步结果共用同一代次。 */
@SuppressWarnings("deprecation")
final class ImeOcr {
    private final MSIMEInputService s;
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private boolean active;
    private long generation;
    private InputConnection editor;
    private OcrCamera camera;
    private Button capture;
    private final ArrayList<CheckBox> selectedLines = new ArrayList<>();
    private TextView resultText;

    ImeOcr(MSIMEInputService service) { s = service; }
    boolean active() { return active; }

    void open() {
        s.closeToolbarPanels();
        active = true;
        editor = s.connection;
        long expected = ++generation;
        base();
        ViewPolicy.show(s.moreToolsScroll);
        s.render();
        if (AppVersionPolicy.playDistribution()) { message("此分发渠道不提供外部可执行模块。"); return; }
        if (s.checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            message("请先在拍照识字设置中授权相机并导入配套模块。");
            setupButton();
            return;
        }
        message("正在检查配套模块…");
        worker.execute(() -> {
            boolean available = false;
            try {
                OcrRuntime runtime = OcrRuntime.open(s);
                runtime.close();
                available = true;
            }
            catch (Exception | LinkageError | OutOfMemoryError unavailable) { }
            boolean ready = available;
            s.main.post(() -> {
                if (!valid(expected)) return;
                base();
                if (!ready) {
                    message("尚未导入可用的配套模块，请检查模块版本或重新导入。");
                    setupButton();
                } else preview(expected);
            });
        });
    }

    void close() {
        if (active && s.moreToolsPanel != null) s.moreToolsPanel.removeAllViews();
        active = false;
        generation++;
        editor = null;
        selectedLines.clear();
        resultText = null;
        if (camera != null) camera.close();
        camera = null;
    }

    void destroy() { close(); worker.shutdownNow(); }

    private boolean valid(long expected) {
        return active && expected == generation && editor != null && editor == s.connection && s.isInputViewShown();
    }

    private void base() {
        s.moreToolsPanel.removeAllViews();
        s.moreToolsPanel.setContentDescription("拍照识字面板");
        LinearLayout header = LayoutPolicy.row(s);
        s.button(header, "返回", () -> { close(); s.imeFunctionPanel.renderMoreTools(); });
        s.button(header, "拍照识字设置", this::setup);
        s.moreToolsPanel.addView(header);
        s.moreToolsScroll.scrollTo(0, 0);
    }

    private Button action(String label, Runnable callback) {
        // 公用按钮使用横排的零宽度权重，不能直接放入纵向面板。
        LinearLayout row = LayoutPolicy.row(s);
        Button button = s.button(row, label, callback);
        s.moreToolsPanel.addView(row);
        return button;
    }

    private void setupButton() { action("导入模块与相机权限", this::setup); }

    private void setup() {
        s.closeMoreTools();
        s.startActivity(new Intent().setClassName(s, "app.msime.android.OcrSetupActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
    }

    private void preview(long expected) {
        message("对准文字后拍照，图片不保存。");
        SurfaceView surface = new SurfaceView(s);
        surface.setContentDescription("拍照识字相机预览");
        FrameLayout viewport = new FrameLayout(s);
        viewport.setClipChildren(true);
        int height = Math.max(s.pixels(72), Math.min(s.pixels(180), s.moreToolsScroll.getHeight() - s.pixels(128)));
        viewport.addView(surface, new FrameLayout.LayoutParams(-1, -1));
        capture = action("拍照并识别", () -> {
            if (!valid(expected) || camera == null || !capture.isEnabled()) return;
            capture.setEnabled(false);
            OcrCamera source = camera;
            source.capture(bytes -> s.main.post(() -> {
                if (!valid(expected)) return;
                source.close();
                camera = null;
                base();
                message("正在本机识别…");
                recognize(expected, bytes);
            }), () -> cameraFailure(expected));
        });
        capture.setEnabled(false);
        s.moreToolsPanel.addView(viewport, new LinearLayout.LayoutParams(-1, height));
        OcrCamera source = new OcrCamera();
        camera = source;
        surface.getHolder().addCallback(new SurfaceHolder.Callback() {
            private boolean started;
            @Override public void surfaceCreated(SurfaceHolder holder) {
                if (!valid(expected) || source != camera || started) return;
                started = true;
                int rotation = s.getSystemService(android.view.WindowManager.class).getDefaultDisplay().getRotation();
                source.start(holder, rotation, (width, previewHeight) -> s.main.post(() -> {
                    if (!valid(expected) || source != camera) return;
                    // SurfaceView 的独立 Surface 不能依赖父容器裁剪；完整放入预览框，避免覆盖编辑器和按钮。
                    float scale = Math.min((float) viewport.getWidth() / width, (float) viewport.getHeight() / previewHeight);
                    surface.setLayoutParams(new FrameLayout.LayoutParams(Math.round(width * scale),
                        Math.round(previewHeight * scale), android.view.Gravity.CENTER));
                    capture.setEnabled(true);
                }), () -> cameraFailure(expected));
            }
            @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) { }
            @Override public void surfaceDestroyed(SurfaceHolder holder) {
                source.close();
                if (valid(expected) && source == camera) cameraFailure(expected);
            }
        });
    }

    private void cameraFailure(long expected) {
        s.main.post(() -> {
            if (!valid(expected)) return;
            if (camera != null) camera.close();
            camera = null;
            base();
            message("相机不可用，可能被其他应用占用或权限已撤回。");
            action("重试", this::open);
        });
    }

    private void recognize(long expected, byte[] jpeg) {
        try {
            worker.execute(() -> {
                String[] lines = null;
                try (OcrRuntime runtime = OcrRuntime.open(s)) {
                    Bitmap bitmap = decode(jpeg);
                    lines = runtime.recognize(bitmap);
                    // SDK 超时后可能仍持有图像，让其引用自然释放，避免提前 recycle。
                } catch (Exception | LinkageError | OutOfMemoryError unavailable) { }
                String[] result = lines;
                s.main.post(() -> { if (valid(expected)) showResult(expected, result); });
            });
        } catch (RejectedExecutionException destroyed) { }
    }

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

    private void showResult(long expected, String[] lines) {
        base();
        selectedLines.clear();
        resultText = null;
        if (lines == null || lines.length == 0) message(lines == null ? "识别失败，请重新拍照。" : "未识别到文字，请调整距离或光线。" );
        else {
            message("勾选需要的行上屏，或在下方长按选择部分文字。识别结果可能有误，请核对。");
            LinearLayout actions = LayoutPolicy.row(s);
            s.button(actions, "上屏勾选行", () -> insert(expected, selectedLines.stream()
                .filter(CheckBox::isChecked).map(view -> view.getText().toString()).collect(java.util.stream.Collectors.joining("\n"))));
            s.button(actions, "上屏选中文字", () -> {
                if (resultText == null) return;
                int start = resultText.getSelectionStart(), end = resultText.getSelectionEnd();
                if (start >= 0 && end >= 0 && start != end)
                    insert(expected, resultText.getText().subSequence(Math.min(start, end), Math.max(start, end)).toString());
            });
            s.moreToolsPanel.addView(actions);
            for (String line : lines) {
                if (line == null || line.isBlank()) continue;
                CheckBox row = new CheckBox(s);
                row.setText(line);
                row.setChecked(true);
                row.setTextColor(android.graphics.Color.parseColor(s.skin.keyForeground()));
                selectedLines.add(row);
                s.moreToolsPanel.addView(row);
            }
            resultText = message(String.join("\n", lines));
            resultText.setTextIsSelectable(true);
        }
        action("重新拍照", this::open);
    }

    private void insert(long expected, String text) {
        if (!valid(expected) || text.isEmpty()) return;
        s.command(2);
        if (s.commitText(text, TypingSource.LOCAL)) { s.closeMoreTools(); s.render(); }
    }

    private TextView message(String value) {
        TextView view = new TextView(s);
        view.setText(value);
        view.setTextSize(14);
        view.setTextColor(android.graphics.Color.parseColor(s.skin.keyForeground()));
        view.setPadding(s.pixels(8), s.pixels(6), s.pixels(8), s.pixels(6));
        s.moreToolsPanel.addView(view);
        return view;
    }
}
