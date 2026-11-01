package app.msime.android;

import android.Manifest;
import android.app.Activity;
import android.app.PendingIntent;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.os.Bundle;
import android.os.Parcel;
import android.speech.RecognizerIntent;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Locale;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** 标准外部语音调用：只在本机识别，文字只回到这次请求指定的结果通道。 */
public final class OfflineVoiceRecognitionActivity extends Activity {
    private static final int RECORD_AUDIO_REQUEST = 1;
    private static final int MAX_RESULT_CHARACTERS = 10_000;
    private final OfflineVoiceSession session = new OfflineVoiceSession();
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final LocalAsrRecognizer recognizer = new LocalAsrRecognizer();
    private PendingIntent pendingResult;
    private Bundle pendingExtras;
    private String language;
    private boolean resumed;
    private boolean started;
    private boolean failed;
    private TextView status;
    private Button done;
    private Button settings;
    private OfflineVoiceListeningView listening;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        setResult(RESULT_CANCELED);
        // 重建时取消旧录音，不能把同一个外部请求再识别、再投递一遍。
        if (state != null || !readRequest()) {
            session.finish();
            finish();
            return;
        }
        showControls();
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] {Manifest.permission.RECORD_AUDIO}, RECORD_AUDIO_REQUEST);
        }
    }

    @SuppressWarnings("deprecation") // 标准 Parcelable 读取兼容 API 28，外部类型仍逐项检查。
    private boolean readRequest() {
        try {
            Intent request = getIntent();
            if (!RecognizerIntent.ACTION_RECOGNIZE_SPEECH.equals(request.getAction())) return false;
            Object pending = request.getParcelableExtra(RecognizerIntent.EXTRA_RESULTS_PENDINGINTENT);
            if (pending != null && !(pending instanceof PendingIntent)) return false;
            pendingResult = (PendingIntent) pending;
            if (pendingResult == null && getCallingActivity() == null) return false;
            pendingExtras = request.getBundleExtra(RecognizerIntent.EXTRA_RESULTS_PENDINGINTENT_BUNDLE);
            if (pendingExtras != null) {
                Parcel parcel = Parcel.obtain();
                try {
                    parcel.writeBundle(pendingExtras);
                    if (parcel.dataSize() > 32 * 1024) return false;
                    pendingExtras.size();
                } finally {
                    parcel.recycle();
                }
            }
            String requested = request.getStringExtra(RecognizerIntent.EXTRA_LANGUAGE);
            if (requested != null && (requested.isBlank()
                    || !TextPolicy.boundedNonEmpty(requested, VoiceRequestPolicy.MAX_LANGUAGE_BYTES))) return false;
            language = requested == null ? Locale.getDefault().toLanguageTag()
                : new Locale.Builder().setLanguageTag(requested).build().toLanguageTag();
            return true;
        } catch (RuntimeException error) {
            return false;
        }
    }

    private void showControls() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setGravity(Gravity.CENTER_HORIZONTAL);
        int padding = pixels(20);
        root.setPadding(padding, padding, padding, padding);
        TextView title = new TextView(this);
        title.setText("水杉离线语音输入");
        title.setTextSize(20);
        title.setGravity(Gravity.CENTER);
        root.addView(title, LayoutPolicy.matchWidthWrapParams());
        listening = new OfflineVoiceListeningView(this);
        root.addView(listening, new LinearLayout.LayoutParams(-1, pixels(250)));
        status = new TextView(this);
        status.setText("只在本机识别，结果返回调用此窗口的应用。");
        status.setGravity(Gravity.CENTER);
        status.setMinLines(2);
        status.setMaxLines(5);
        root.addView(status, LayoutPolicy.matchWidthWrapParams());
        LinearLayout actions = new LinearLayout(this);
        actions.setGravity(Gravity.CENTER);
        Button cancel = new Button(this);
        cancel.setText("取消");
        cancel.setOnClickListener(view -> returnResult(null));
        actions.addView(cancel);
        done = new Button(this);
        done.setText("完成");
        done.setEnabled(false);
        done.setOnClickListener(view -> {
            if (!session.stop()) return;
            recognizer.stop();
            done.setEnabled(false);
            listening.setLevel(0);
            status.setText("正在本机完成识别…");
        });
        actions.addView(done);
        settings = new Button(this);
        settings.setText("语音设置");
        settings.setVisibility(View.GONE);
        settings.setOnClickListener(view -> {
            Bundle args = new Bundle();
            args.putString(HostDeepLink.ARG_SECTION, "服务");
            Intent target = HostDeepLink.page(this, "VOICE", args);
            returnResult(null);
            startActivity(target);
        });
        root.addView(actions, LayoutPolicy.matchWidthWrapParams());
        root.addView(settings);
        setContentView(root);
    }

    @Override protected void onResume() {
        super.onResume();
        resumed = true;
        startWhenVisible();
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] grants) {
        super.onRequestPermissionsResult(request, permissions, grants);
        if (request != RECORD_AUDIO_REQUEST || session.finished()) return;
        if (grants.length == 0 || grants[0] != PackageManager.PERMISSION_GRANTED) {
            showFailure("需要麦克风权限才能使用本地语音输入。");
            return;
        }
        startWhenVisible();
    }

    private void startWhenVisible() {
        if (!resumed || failed || session.finished() || status == null
                || checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED
                || !session.start()) return;
        started = true;
        status.setText("正在准备本地模型…");
        LocalAsrRecognizer.watchMemory(this);
        worker.execute(() -> {
            loadMascot();
            File files = getFilesDir();
            String directory = HostOptionsPolicy.readOption(files, "preferences_directory");
            VoiceConfiguration configuration = VoiceConfiguration.read(directory, UUID.randomUUID().toString());
            if (session.finished()) return;
            if (!"local".equals(configuration.provider()) || configuration.localModel() == null
                    || configuration.localModel().isEmpty()) {
                runOnUiThread(() -> showFailure("请在语音设置中选择「本地模型」，安装并选用模型，同时准备语音运行库。"));
                return;
            }
            try {
                runOnUiThread(() -> {
                    if (session.finished() || failed) return;
                    done.setEnabled(session.listening());
                    status.setText("正在本机聆听，说完后点「完成」。");
                });
                if (session.finished()) return;
                // 离线识别窗口由「完成」结束、不按停顿判停，判停灵敏度在这里换不来好处，传 0 让阈值保持 0.5。
                recognizer.vadSensitivity(0);
                String text = recognizer.recognize(configuration.localModel(), files.toPath(), language,
                    HostOptionsPolicy.readRuntimeOptions(files), null, null, new LocalAsrRecognizer.Listener() {
                        @Override public void onPartial(String text) {
                            runOnUiThread(() -> {
                                if (!session.finished() && !failed) status.setText(text);
                            });
                        }
                        @Override public void onVolume(float level) {
                            runOnUiThread(() -> {
                                if (session.listening() && !failed) listening.setLevel(level);
                            });
                        }
                    });
                runOnUiThread(() -> {
                    if (!failed) returnResult(text);
                });
            } catch (LocalAsrRecognizer.Refused error) {
                runOnUiThread(() -> showFailure(LocalAsrPolicy.failureMessage(error.failure(), error.runtimeMissing())));
            } catch (RuntimeException | LinkageError error) {
                runOnUiThread(() -> showFailure("本地识别未能完成，请检查语音设置中的模型和运行库。"));
            }
        });
    }

    private void loadMascot() {
        try (InputStream input = getAssets().open(StickerPackModel.ASSET_DIRECTORY + "/01.webp")) {
            Bitmap bitmap = BitmapFactory.decodeStream(input);
            runOnUiThread(() -> {
                if (!session.finished()) listening.setMascot(bitmap);
            });
        } catch (IOException error) {
            // 素材损坏不触发下载，错误界面仍提供取消和设置入口。
        }
    }

    private void showFailure(String message) {
        if (session.finished() || status == null) return;
        failed = true;
        recognizer.cancel();
        done.setEnabled(false);
        listening.setLevel(0);
        status.setText(message);
        settings.setVisibility(View.VISIBLE);
    }

    private void returnResult(String text) {
        if (!session.finish()) return;
        recognizer.cancel();
        String result = TextPolicy.trimmed(text);
        boolean success = !failed && !result.isEmpty() && result.length() <= MAX_RESULT_CHARACTERS
            && TextPolicy.validUnicode(result);
        Intent data = new Intent();
        if (pendingResult != null && pendingExtras != null) data.putExtras(pendingExtras);
        data.removeExtra(RecognizerIntent.EXTRA_RESULTS);
        if (success) {
            ArrayList<String> results = new ArrayList<>();
            results.add(result);
            data.putStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS, results);
        }
        int code = success ? RESULT_OK : RESULT_CANCELED;
        if (pendingResult == null) {
            setResult(code, data);
        } else {
            try {
                pendingResult.send(this, code, data);
            } catch (PendingIntent.CanceledException | RuntimeException error) {
                // 调用方取消了结果通道，不改投其他存储或应用。
            }
        }
        finish();
    }

    @Override protected void onPause() {
        resumed = false;
        if (started && !session.finished()) returnResult(null);
        super.onPause();
    }

    @Override protected void onStop() {
        // 权限弹窗会暂停窗口；真正不可见时连尚未授权的请求也必须取消。
        if (!session.finished()) returnResult(null);
        super.onStop();
    }

    @Override protected void onDestroy() {
        if (!session.finished()) returnResult(null);
        recognizer.cancel();
        worker.shutdownNow();
        super.onDestroy();
    }

    @Override @SuppressWarnings("deprecation") // API 28 与关闭预测式返回的原生窗口共用取消路径。
    public void onBackPressed() {
        returnResult(null);
    }

    private int pixels(int dp) {
        return Math.round(dp * getResources().getDisplayMetrics().density);
    }
}
