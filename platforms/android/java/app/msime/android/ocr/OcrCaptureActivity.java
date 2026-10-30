package app.msime.android;

import android.Manifest;
import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ResultReceiver;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.TextPaint;
import android.text.method.LinkMovementMethod;
import android.text.style.ClickableSpan;
import android.view.Gravity;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import java.util.ArrayList;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;

/** 不建立新输入连接的内部拍照窗口；图片与识别内容只在当前实例内存中存在。 */
@SuppressWarnings("deprecation")
public final class OcrCaptureActivity extends Activity {
    static final String RECEIVER = "ocr.receiver", REPLY = "ocr.reply", TEXT = "ocr.text";
    static final int REGISTER = 1, INSERT = 2, CANCEL = 3, ACCEPTED = 4, INVALID = 5;
    static final int TEXT_LIMIT = 32768;
    private enum Page { CAMERA, PHOTO, RECOGNIZING, RESULT, ERROR }
    private final Handler main = new Handler(Looper.getMainLooper());
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final ResultReceiver response = new ResultReceiver(main) {
        @Override protected void onReceiveResult(int code, Bundle ignored) {
            if (isFinishing() || isDestroyed()) return;
            if (code == ACCEPTED) { finish(); return; }
            if (code == INVALID) {
                insertAvailable = false;
                if (insert != null) insert.setEnabled(false);
                if (page == Page.RESULT) toast("无法插入原输入框，可复制所选文字后返回");
            }
        }
    };
    private ResultReceiver receiver;
    private Page page = Page.CAMERA;
    private boolean resumed, moduleReady, insertAvailable = true, checking;
    private long generation;
    private LinearLayout root;
    private FrameLayout content;
    private OcrCamera camera;
    private Button shutter, insert, selectAll;
    private Bitmap photo;
    private OcrPhotoSelectionView selection;
    private TextView result;
    private String recognized = "";
    private final ArrayList<Integer> boundaries = new ArrayList<>();
    private boolean[] selected = new boolean[0];

    @Override public void onCreate(Bundle state) {
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM);
        super.onCreate(state);
        ScreenCaptureProtection.apply(this);
        getWindow().getDecorView().setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
        receiver = getIntent().getParcelableExtra(RECEIVER);
        if (receiver == null || state != null) { finish(); return; }
        Bundle registration = new Bundle();
        registration.putParcelable(REPLY, response);
        receiver.send(REGISTER, registration);
        cameraPage();
    }

    @Override protected void onResume() {
        super.onResume();
        resumed = true;
        if (page == Page.CAMERA) cameraPage();
    }

    @Override protected void onPause() {
        resumed = false;
        generation++;
        checking = false;
        closeCamera();
        if (page == Page.RECOGNIZING && photo != null) photoPage();
        super.onPause();
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        if (page == Page.CAMERA) cameraPage();
    }

    @Override protected void onDestroy() {
        generation++;
        closeCamera();
        worker.shutdownNow();
        if (receiver != null) receiver.send(CANCEL, Bundle.EMPTY);
        receiver = null;
        photo = null;
        selection = null;
        recognized = "";
        selected = new boolean[0];
        super.onDestroy();
    }

    private void base() {
        root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Color.WHITE);
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            view.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });
        content = new FrameLayout(this);
        content.setBackgroundColor(Color.rgb(25, 28, 31));
        insert = null;
        setContentView(root);
        root.requestApplyInsets();
    }

    private LinearLayout row() {
        LinearLayout row = new LinearLayout(this);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setPadding(dp(6), 0, dp(6), 0);
        return row;
    }

    private Button button(LinearLayout row, String label, Runnable action) {
        Button button = new Button(this);
        button.setText(label);
        button.setTextColor(Color.rgb(21, 74, 58));
        button.setAllCaps(false);
        button.setMinHeight(dp(48));
        button.setOnClickListener(view -> action.run());
        row.addView(button, new LinearLayout.LayoutParams(0, -2, 1));
        return button;
    }

    private void body() { root.addView(content, new LinearLayout.LayoutParams(-1, 0, 1)); }

    private void cameraPage() {
        page = Page.CAMERA;
        generation++;
        checking = false;
        closeCamera();
        photo = null;
        selection = null;
        recognized = "";
        selected = new boolean[0];
        base();
        LinearLayout top = row();
        button(top, "返回键盘", this::finish);
        button(top, "拍照识字设置", () -> {
            startActivity(new Intent().setClassName(this, "app.msime.android.OcrSetupActivity"));
            finish();
        });
        root.addView(top);
        body();
        LinearLayout bottom = row();
        shutter = button(bottom, "拍照", this::capture);
        shutter.setEnabled(false);
        root.addView(bottom);
        if (AppVersionPolicy.playDistribution()) { message("此分发渠道不提供外部可执行模块。"); return; }
        if (checkSelfPermission(Manifest.permission.CAMERA) != PackageManager.PERMISSION_GRANTED) {
            message("请先在拍照识字设置中授权相机并导入配套模块。"); return;
        }
        if (!resumed) return;
        if (moduleReady) { preview(); return; }
        message("正在检查配套模块…");
        long expected = generation;
        checking = true;
        execute(() -> {
            boolean ready = false;
            try (OcrRuntime runtime = OcrRuntime.open(this)) { ready = runtime != null; }
            catch (Exception | LinkageError | OutOfMemoryError unavailable) { }
            boolean available = ready;
            main.post(() -> {
                if (!current(expected)) return;
                checking = false;
                moduleReady = available;
                if (available) preview();
                else message("尚未导入可用的配套模块，请在拍照识字设置中检查或重新导入。");
            });
        });
    }

    private void preview() {
        content.removeAllViews();
        SurfaceView surface = new SurfaceView(this);
        surface.setContentDescription("拍照识字相机预览");
        content.addView(surface, new FrameLayout.LayoutParams(-1, -1));
        OcrCamera source = new OcrCamera();
        camera = source;
        long expected = generation;
        surface.getHolder().addCallback(new SurfaceHolder.Callback() {
            private boolean started;
            @Override public void surfaceCreated(SurfaceHolder holder) {
                if (!current(expected) || source != camera || started) return;
                started = true;
                source.start(holder, getWindowManager().getDefaultDisplay().getRotation(),
                    (width, height) -> main.post(() -> {
                        if (!current(expected) || source != camera) return;
                        float scale = Math.min((float) content.getWidth() / width, (float) content.getHeight() / height);
                        surface.setLayoutParams(new FrameLayout.LayoutParams(Math.round(width * scale),
                            Math.round(height * scale), Gravity.CENTER));
                        shutter.setEnabled(true);
                    }), () -> cameraFailure(expected));
            }
            @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) { }
            @Override public void surfaceDestroyed(SurfaceHolder holder) {
                source.close();
                if (current(expected) && source == camera) cameraFailure(expected);
            }
        });
    }

    private void capture() {
        if (camera == null || !shutter.isEnabled() || checking) return;
        shutter.setEnabled(false);
        long expected = generation;
        OcrCamera source = camera;
        source.capture(bytes -> main.post(() -> {
            if (!current(expected) || camera != source) return;
            closeCamera();
            message("正在处理照片…");
            execute(() -> {
                Bitmap decoded = null;
                try { decoded = ImeOcr.decode(bytes); }
                catch (Exception | LinkageError | OutOfMemoryError unavailable) { }
                Bitmap image = decoded;
                main.post(() -> {
                    if (!current(expected)) return;
                    if (image == null) { cameraFailure(expected); return; }
                    photo = image;
                    photoPage();
                });
            });
        }), () -> cameraFailure(expected));
    }

    private void photoPage() {
        page = Page.PHOTO;
        base();
        LinearLayout top = row();
        button(top, "返回键盘", this::finish);
        button(top, "裁剪", () -> selection.mode(OcrPhotoSelectionView.Mode.CROP));
        root.addView(top);
        if (selection == null) selection = new OcrPhotoSelectionView(this, photo);
        if (selection.getParent() instanceof android.view.ViewGroup parent) parent.removeView(selection);
        content.addView(selection, new FrameLayout.LayoutParams(-1, -1));
        Button retake = new Button(this);
        retake.setText("↶");
        retake.setTextSize(26);
        retake.setTextColor(Color.WHITE);
        android.graphics.drawable.GradientDrawable circle = new android.graphics.drawable.GradientDrawable();
        circle.setShape(android.graphics.drawable.GradientDrawable.OVAL);
        circle.setColor(Color.rgb(21, 74, 58));
        retake.setBackground(circle);
        retake.setContentDescription("重新拍照");
        retake.setOnClickListener(view -> cameraPage());
        FrameLayout.LayoutParams position = new FrameLayout.LayoutParams(dp(48), dp(48), Gravity.BOTTOM | Gravity.RIGHT);
        position.setMargins(dp(12), dp(12), dp(12), dp(12));
        content.addView(retake, position);
        content.addOnLayoutChangeListener((view, left, topEdge, right, bottomEdge, oldLeft, oldTop, oldRight, oldBottom) -> {
            if (photo == null) return;
            float scale = Math.min((float) view.getWidth() / photo.getWidth(), (float) view.getHeight() / photo.getHeight());
            int rightMargin = Math.round((view.getWidth() - photo.getWidth() * scale) / 2) + dp(12);
            int bottomMargin = Math.round((view.getHeight() - photo.getHeight() * scale) / 2) + dp(12);
            if (position.rightMargin == rightMargin && position.bottomMargin == bottomMargin) return;
            position.rightMargin = rightMargin;
            position.bottomMargin = bottomMargin;
            retake.setLayoutParams(position);
        });
        body();
        LinearLayout bottom = row();
        button(bottom, "涂抹", () -> selection.mode(OcrPhotoSelectionView.Mode.BRUSH));
        button(bottom, "下一步", this::recognize);
        root.addView(bottom);
    }

    private void recognize() {
        if (page != Page.PHOTO || selection == null) return;
        OcrPhotoSelectionView.Selection area = selection.selection();
        if (area == null) { toast("请先涂抹要识别的文字区域"); return; }
        page = Page.RECOGNIZING;
        long expected = ++generation;
        base();
        LinearLayout top = row();
        button(top, "返回键盘", this::finish);
        root.addView(top);
        body();
        message("正在本机识别…");
        execute(() -> {
            String[] lines = null;
            try (OcrRuntime runtime = OcrRuntime.open(this)) {
                Bitmap image = area.image();
                lines = runtime.recognize(image);
                // SDK 异常后仍可能持有图像；只放弃引用，不提前 recycle。
            } catch (Exception | LinkageError | OutOfMemoryError unavailable) { }
            String[] output = lines;
            main.post(() -> { if (current(expected)) showResult(output); });
        });
    }

    private void showResult(String[] lines) {
        page = Page.RESULT;
        StringBuilder text = new StringBuilder();
        if (lines != null) for (String line : lines) {
            if (line == null || line.isBlank()) continue;
            if (text.length() != 0) text.append('\n');
            if (text.length() + line.length() > TEXT_LIMIT) { lines = null; break; }
            text.append(line);
        }
        recognized = lines == null ? "" : text.toString();
        boundaries.clear();
        for (int at : characterBoundaries(recognized)) boundaries.add(at);
        selected = new boolean[Math.max(0, boundaries.size() - 1)];
        java.util.Arrays.fill(selected, true);
        base();
        LinearLayout top = row();
        button(top, "返回键盘", this::finish);
        selectAll = button(top, "☑", () -> {
            if (selected.length == 0) return;
            boolean all = true;
            for (boolean value : selected) all &= value;
            java.util.Arrays.fill(selected, !all);
            renderSelection();
        });
        selectAll.setContentDescription("全选文字");
        selectAll.setLayoutParams(new LinearLayout.LayoutParams(dp(48), dp(48)));
        selectAll.setEnabled(selected.length > 0);
        button(top, "重新拍照", this::cameraPage);
        root.addView(top);
        ScrollView scroll = new ScrollView(this);
        result = new TextView(this);
        result.setTextSize(25);
        result.setTextColor(Color.BLACK);
        result.setPadding(dp(16), dp(16), dp(16), dp(16));
        result.setContentDescription("点选识别文字");
        result.setMovementMethod(LinkMovementMethod.getInstance());
        scroll.setBackgroundColor(Color.WHITE);
        scroll.addView(result);
        content.addView(scroll, new FrameLayout.LayoutParams(-1, -1));
        body();
        LinearLayout bottom = row();
        button(bottom, "复制", () -> {
            String value = selectedText();
            if (value.isEmpty()) { toast("请先点选文字"); return; }
            getSystemService(ClipboardManager.class).setPrimaryClip(ClipData.newPlainText("识别文字", value));
            toast("已复制所选文字");
        });
        insert = button(bottom, "插入", this::insert);
        insert.setEnabled(insertAvailable && selected.length > 0);
        root.addView(bottom);
        if (recognized.isEmpty()) result.setText(lines == null ? "识别失败，请重新拍照。" : "未识别到文字，请重新拍照。");
        else renderSelection();
    }

    /** 系统 ICU 按字素划分点击范围，组合附加符与 emoji 序列不可分开选取。 */
    static int[] characterBoundaries(String text) {
        android.icu.text.BreakIterator iterator = android.icu.text.BreakIterator.getCharacterInstance();
        iterator.setText(text);
        ArrayList<Integer> offsets = new ArrayList<>();
        for (int at = iterator.first(); at != android.icu.text.BreakIterator.DONE; at = iterator.next()) offsets.add(at);
        return offsets.stream().mapToInt(Integer::intValue).toArray();
    }

    private void renderSelection() {
        SpannableString display = new SpannableString(recognized);
        for (int i = 0; i < selected.length; i++) {
            final int index = i;
            display.setSpan(new ClickableSpan() {
                @Override public void onClick(View widget) { selected[index] = !selected[index]; renderSelection(); }
                @Override public void updateDrawState(TextPaint paint) {
                    paint.setColor(Color.rgb(18, 46, 37));
                    paint.bgColor = selected[index] ? Color.rgb(189, 230, 212) : Color.TRANSPARENT;
                    paint.setUnderlineText(false);
                }
            }, boundaries.get(i), boundaries.get(i + 1), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        result.setText(display);
        boolean all = selected.length > 0;
        for (boolean value : selected) all &= value;
        selectAll.setText(all ? "☑" : "☐");
        selectAll.setSelected(all);
    }

    private String selectedText() {
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < selected.length; i++)
            if (selected[i]) text.append(recognized, boundaries.get(i), boundaries.get(i + 1));
        return text.toString();
    }

    private void insert() {
        String text = selectedText();
        if (!insertAvailable || text.isEmpty() || receiver == null) { toast("请先点选文字"); return; }
        insert.setEnabled(false);
        Bundle payload = new Bundle();
        payload.putString(TEXT, text);
        payload.putParcelable(REPLY, response);
        receiver.send(INSERT, payload);
    }

    private void cameraFailure(long expected) {
        main.post(() -> {
            if (!current(expected)) return;
            page = Page.ERROR;
            closeCamera();
            message("相机不可用，可能被其他应用占用或权限已撤回。");
            LinearLayout bottom = row();
            button(bottom, "重试", this::cameraPage);
            root.addView(bottom);
        });
    }

    private void closeCamera() { if (camera != null) camera.close(); camera = null; }
    private boolean current(long expected) { return expected == generation && resumed && !isFinishing() && !isDestroyed(); }
    private void execute(Runnable task) { try { worker.execute(task); } catch (RejectedExecutionException destroyed) { } }
    private void message(String message) {
        content.removeAllViews();
        TextView text = new TextView(this);
        text.setText(message);
        text.setTextColor(Color.WHITE);
        text.setTextSize(18);
        text.setGravity(Gravity.CENTER);
        text.setPadding(dp(24), dp(24), dp(24), dp(24));
        content.addView(text, new FrameLayout.LayoutParams(-1, -1));
    }
    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
    private void toast(String message) { Toast.makeText(this, message, Toast.LENGTH_SHORT).show(); }
}
