package app.msime.android;

import android.Manifest;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Matrix;
import android.media.ExifInterface;
import android.graphics.SurfaceTexture;
import android.view.TextureView;
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
    private int[] panelPadding;
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
        restorePadding();
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

    private void restorePadding() {
        if (panelPadding == null) return;
        s.moreToolsPanel.setPadding(panelPadding[0], panelPadding[1], panelPadding[2], panelPadding[3]);
        panelPadding = null;
    }

    private void base() {
        restorePadding();
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
        s.moreToolsPanel.removeAllViews();
        panelPadding = new int[] {s.moreToolsPanel.getPaddingLeft(), s.moreToolsPanel.getPaddingTop(),
            s.moreToolsPanel.getPaddingRight(), s.moreToolsPanel.getPaddingBottom()};
        s.moreToolsPanel.setPadding(0, 0, 0, 0);
        FrameLayout viewport = new FrameLayout(s);
        viewport.setBackgroundColor(android.graphics.Color.BLACK);
        TextureView surface = new TextureView(s);
        surface.setContentDescription("拍照识字相机预览");
        viewport.addView(surface, new FrameLayout.LayoutParams(-1, -1));
        LinearLayout header = LayoutPolicy.row(s);
        s.button(header, "返回", () -> { close(); s.imeFunctionPanel.renderMoreTools(); });
        s.button(header, "拍照识字设置", this::setup);
        viewport.addView(header, new FrameLayout.LayoutParams(-1, -2, android.view.Gravity.TOP));
        LinearLayout footer = LayoutPolicy.row(s);
        capture = s.button(footer, "拍照并识别", () -> {
            if (!valid(expected) || camera == null || !capture.isEnabled()) return;
            capture.setEnabled(false);
            // 冻结按下拍照时的取景框，异步结果不随之后的键区尺寸变化。
            int cropWidth = surface.getWidth(), cropHeight = surface.getHeight();
            OcrCamera source = camera;
            source.capture(bytes -> s.main.post(() -> {
                if (!valid(expected)) return;
                source.close();
                camera = null;
                base();
                message("正在本机识别…");
                recognize(expected, bytes, cropWidth, cropHeight);
            }), () -> cameraFailure(expected));
        });
        capture.setEnabled(false);
        viewport.addView(footer, new FrameLayout.LayoutParams(-1, -2, android.view.Gravity.BOTTOM));
        s.moreToolsPanel.addView(viewport, new LinearLayout.LayoutParams(-1, -1));
        OcrCamera source = new OcrCamera();
        camera = source;
        surface.setSurfaceTextureListener(new TextureView.SurfaceTextureListener() {
            private int previewWidth, previewHeight;
            private boolean handedToCamera;
            private void transform() {
                if (previewWidth <= 0 || previewHeight <= 0 || surface.getWidth() <= 0 || surface.getHeight() <= 0) return;
                float scale = Math.max((float) surface.getWidth() / previewWidth,
                    (float) surface.getHeight() / previewHeight);
                Matrix matrix = new Matrix();
                // TextureView 默认把纹理拉伸到控件大小；补偿后等比例中心裁剪，变换只作用于内容。
                matrix.setScale(previewWidth * scale / surface.getWidth(), previewHeight * scale / surface.getHeight(),
                    surface.getWidth() / 2f, surface.getHeight() / 2f);
                surface.setTransform(matrix);
            }
            @Override public void onSurfaceTextureAvailable(SurfaceTexture texture, int width, int height) {
                if (!valid(expected) || source != camera) return;
                int rotation = s.getSystemService(android.view.WindowManager.class).getDefaultDisplay().getRotation();
                handedToCamera = true;
                source.start(texture, rotation, (readyWidth, readyHeight) -> s.main.post(() -> {
                    if (!valid(expected) || source != camera) return;
                    previewWidth = readyWidth;
                    previewHeight = readyHeight;
                    transform();
                    capture.setEnabled(true);
                }), () -> cameraFailure(expected));
            }
            @Override public void onSurfaceTextureSizeChanged(SurfaceTexture texture, int width, int height) { transform(); }
            @Override public boolean onSurfaceTextureDestroyed(SurfaceTexture texture) {
                // 驱动在相机线程停止后再释放纹理，避免 UI 线程提前销毁正在使用的 Surface。
                source.close();
                if (valid(expected) && source == camera) cameraFailure(expected);
                return !handedToCamera;
            }
            @Override public void onSurfaceTextureUpdated(SurfaceTexture texture) { }
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

    private void recognize(long expected, byte[] jpeg, int width, int height) {
        try {
            worker.execute(() -> {
                String[] lines = null;
                try (OcrRuntime runtime = OcrRuntime.open(s)) {
                    Bitmap bitmap = crop(decode(jpeg), width, height);
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

    /** 与中心裁剪预览使用同一取景比例，只把可见范围交给识别器。 */
    static Bitmap crop(Bitmap bitmap, int width, int height) {
        if (width <= 0 || height <= 0) throw new IllegalArgumentException("ocr_viewport");
        int croppedWidth = bitmap.getWidth(), croppedHeight = bitmap.getHeight();
        if ((long) croppedWidth * height > (long) croppedHeight * width)
            croppedWidth = Math.max(1, (int) ((long) croppedHeight * width / height));
        else croppedHeight = Math.max(1, (int) ((long) croppedWidth * height / width));
        Bitmap cropped = Bitmap.createBitmap(bitmap, (bitmap.getWidth() - croppedWidth) / 2,
            (bitmap.getHeight() - croppedHeight) / 2, croppedWidth, croppedHeight);
        if (cropped != bitmap) bitmap.recycle();
        return cropped;
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
