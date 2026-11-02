package app.msime.android;

import app.msime.android.LayoutPolicy;


import app.msime.android.policy.HostOptionsPolicy;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Bundle;
import android.os.ResultReceiver;
import android.speech.RecognitionListener;
import android.speech.RecognizerIntent;
import android.speech.SpeechRecognizer;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Toast;
import app.msime.android.KeyboardGeometry;
import app.msime.android.ViewPolicy;
import java.io.File;
import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Voice capture, by whichever engine the user's settings call for.
 *
 * <p>Android's SpeechRecognizer owns the microphone itself and needs no account, no token and no network of the user's choosing; it is what this activity uses when nothing has been configured, and it stays the default. A user who configured a transcription provider has chosen a different transcriber, and reaching it means holding the audio here and uploading it - that is {@link HttpAsrRecognizer}, or {@link DoubaoRecognizer} for the streaming socket. A user who chose on-device recognition gets {@link LocalAsrRecognizer}: the installed model runs in this process and the audio never leaves the device.
 *
 * <p>Either way this activity persists only the bounded text result for the isolated IME process.
 */
public final class VoiceRecognitionActivity extends Activity {
    static final String EXTRA_RESULT_RECEIVER = "app.msime.android.voice.RESULT_RECEIVER";
    static final String RESULT_ID = "result_id";
    private static final String EXTRA_DIAGNOSTICS = "app.msime.android.voice.DIAGNOSTICS";
    private static final int REQUEST_RECORD_AUDIO = 1;
    private static final String EXTRA_LANGUAGE = "app.msime.android.voice.LANGUAGE";
    private static final String EXTRA_REQUEST_ID = "app.msime.android.voice.REQUEST_ID";
    private static final String EXTRA_PROVIDER = "app.msime.android.voice.PROVIDER";
    private static final String EXTRA_REQUEST_FORMAT = "app.msime.android.voice.REQUEST_FORMAT";
    private static final String EXTRA_ENDPOINT = "app.msime.android.voice.ENDPOINT";
    private static final String EXTRA_MODEL = "app.msime.android.voice.MODEL";
    private static final String EXTRA_TOKEN = "app.msime.android.voice.TOKEN";
    private static final String EXTRA_POLISH_ENDPOINT = "app.msime.android.voice.POLISH_ENDPOINT";
    private static final String EXTRA_POLISH_MODEL = "app.msime.android.voice.POLISH_MODEL";
    private static final String EXTRA_POLISH_TOKEN = "app.msime.android.voice.POLISH_TOKEN";
    private static final String EXTRA_POLISH_PROMPT = "app.msime.android.voice.POLISH_PROMPT";
    private static final String EXTRA_STREAM_ENDPOINT = "app.msime.android.voice.STREAM_ENDPOINT";
    private static final String EXTRA_STREAM_HEADERS = "app.msime.android.voice.STREAM_HEADERS";
    private static final String EXTRA_STREAM_ITN = "app.msime.android.voice.STREAM_ITN";
    private static final String EXTRA_STREAM_PUNC = "app.msime.android.voice.STREAM_PUNC";
    private static final String EXTRA_STREAM_DDC = "app.msime.android.voice.STREAM_DDC";
    private static final String EXTRA_STREAM_BOOSTING = "app.msime.android.voice.STREAM_BOOSTING";
    private static final String EXTRA_LOCAL_MODEL = "app.msime.android.voice.LOCAL_MODEL";
    private static final String EXTRA_LOCAL_HOTWORD_TEXTS = "app.msime.android.voice.LOCAL_HOTWORD_TEXTS";
    private static final String EXTRA_LOCAL_HOTWORD_PINYIN = "app.msime.android.voice.LOCAL_HOTWORD_PINYIN";
    private static volatile WeakReference<VoiceRecognitionActivity> active =
        new WeakReference<>(null);
    private static volatile String activeRequestId;
    private SpeechRecognizer recognizer;
    private HttpAsrRecognizer provider;
    private DoubaoRecognizer streaming;
    private LocalAsrRecognizer local;
    private ExecutorService providerWorker;
    private final VoicePolisher voicePolisher = new VoicePolisher();
    private TextView recordingTitle;
    private TextView recordingHint;
    private boolean stopping;
    private boolean finished;
    private boolean resumed;
    private boolean recognitionPending;

    public static boolean available(Context context) {
        if (context == null) return false;
        try {
            return SpeechRecognizer.isRecognitionAvailable(context);
        } catch (RuntimeException | LinkageError unavailable) {
            // 有些厂商系统在识别服务被停用或卸载过程中会抛异常；按不可用处理，让设置页给出其他路径。
            return false;
        }
    }

    public static void markLaunched(String requestId) {
        activeRequestId = requestId;
    }

    public static void clearRequest(String requestId) {
        if (requestId != null && requestId.equals(activeRequestId)) activeRequestId = null;
    }

    public static boolean isRequestActive(String requestId) {
        return requestId != null && requestId.equals(activeRequestId);
    }

    /** The optional rewrite, already resolved: the prompt is text here, not a slot to look up. */
    public record Polish(String endpoint, String model, String token, String prompt) {}

    /** The streaming session, already authenticated: `headers` is flattened name/value pairs. */
    public record Streaming(String endpoint, String[] headers, boolean itn, boolean punctuation,
                            boolean ddc, String boostingTableId) {}

    public static void launch(Context context, String requestId, String language) {
        launch(context, requestId, language, null, null, null, null, null, null, null);
    }

    /**
     * Launch with a configured transcription provider, or without one to use the platform
     * recognizer. The three provider values are resolved and validated by the shared layer; this
     * activity only checks that it can speak that protocol before using them. `requestFormat` 是共享层给出的请求格式，上传按它拼请求体。
     */
    public static void launch(Context context, String requestId, String language,
                              String provider, String requestFormat, String endpoint, String model,
                              String token, Streaming streaming, Polish polish) {
        launch(context, requestId, language, provider, requestFormat, endpoint, model, token,
            streaming, polish, null);
    }

    /**
     * Launch with every engine this activity has. `localModel` is the installed model directory for on-device recognition, already resolved by the shared layer; when present it wins over the network settings, which the shared resolution leaves empty for provider `local` anyway.
     */
    public static void launch(Context context, String requestId, String language,
                              String provider, String requestFormat, String endpoint, String model,
                              String token, Streaming streaming, Polish polish, String localModel) {
        launch(context, requestId, language, provider, requestFormat, endpoint, model, token,
            streaming, polish, localModel, null, null);
    }

    /**
     * {@link #launch(Context, String, String, String, String, String, String, String, Streaming, Polish, String)} with the hotwords the shared layer already resolved for `localModel`: the Tauri request carries them as parallel `text` / `pinyin` arrays. Null reads them from the user's dictionary when the dictation starts.
     */
    public static void launch(Context context, String requestId, String language,
                              String provider, String requestFormat, String endpoint, String model,
                              String token, Streaming streaming, Polish polish, String localModel,
                              String[] localHotwordTexts, String[] localHotwordPinyin) {
        launch(context, requestId, language, provider, requestFormat, endpoint, model, token,
            streaming, polish, localModel, localHotwordTexts, localHotwordPinyin, null);
    }

    /** 键盘窗口的固定事件回传；没有接收器的独立入口不写键盘日志。 */
    public static void launch(Context context, String requestId, String language,
                              String provider, String requestFormat, String endpoint, String model,
                              String token, Streaming streaming, Polish polish, String localModel,
                              String[] localHotwordTexts, String[] localHotwordPinyin, ResultReceiver diagnostics) {
        launch(context, requestId, language, provider, requestFormat, endpoint, model, token,
            streaming, polish, localModel, localHotwordTexts, localHotwordPinyin, diagnostics, null);
    }

    /** 键盘发起的窗口识别只回传已保存结果的编号，文字仍由同一存储一次性领取。 */
    public static void launch(Context context, String requestId, String language,
                              String provider, String requestFormat, String endpoint, String model,
                              String token, Streaming streaming, Polish polish, String localModel,
                              String[] localHotwordTexts, String[] localHotwordPinyin,
                              ResultReceiver diagnostics, ResultReceiver resultReceiver) {
        markLaunched(requestId);
        Intent intent = new Intent(context, VoiceRecognitionActivity.class);
        if (resultReceiver != null) intent.putExtra(EXTRA_RESULT_RECEIVER, resultReceiver);
        if (diagnostics != null) intent.putExtra(EXTRA_DIAGNOSTICS, diagnostics);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        intent.putExtra(EXTRA_REQUEST_ID, requestId);
        intent.putExtra(EXTRA_LANGUAGE, safeLanguage(language));
        if (provider != null) intent.putExtra(EXTRA_PROVIDER, provider);
        if (requestFormat != null) intent.putExtra(EXTRA_REQUEST_FORMAT, requestFormat);
        if (endpoint != null) intent.putExtra(EXTRA_ENDPOINT, endpoint);
        if (model != null) intent.putExtra(EXTRA_MODEL, model);
        if (token != null) intent.putExtra(EXTRA_TOKEN, token);
        if (localModel != null) intent.putExtra(EXTRA_LOCAL_MODEL, localModel);
        if (localModel != null && localHotwordTexts != null && localHotwordPinyin != null
            && localHotwordTexts.length == localHotwordPinyin.length) {
            intent.putExtra(EXTRA_LOCAL_HOTWORD_TEXTS, localHotwordTexts);
            intent.putExtra(EXTRA_LOCAL_HOTWORD_PINYIN, localHotwordPinyin);
        }
        if (streaming != null) {
            intent.putExtra(EXTRA_STREAM_ENDPOINT, streaming.endpoint());
            intent.putExtra(EXTRA_STREAM_HEADERS, streaming.headers());
            intent.putExtra(EXTRA_STREAM_ITN, streaming.itn());
            intent.putExtra(EXTRA_STREAM_PUNC, streaming.punctuation());
            intent.putExtra(EXTRA_STREAM_DDC, streaming.ddc());
            intent.putExtra(EXTRA_STREAM_BOOSTING, streaming.boostingTableId());
        }
        if (polish != null) {
            intent.putExtra(EXTRA_POLISH_ENDPOINT, polish.endpoint());
            intent.putExtra(EXTRA_POLISH_MODEL, polish.model());
            intent.putExtra(EXTRA_POLISH_TOKEN, polish.token());
            intent.putExtra(EXTRA_POLISH_PROMPT, polish.prompt());
        }
        context.startActivity(intent);
    }

    @SuppressWarnings("deprecation") // API 28 仍需要旧的 Parcelable 读取入口。
    private void diagnostic(DiagnosticEventKind kind) {
        ResultReceiver sink = getIntent().getParcelableExtra(EXTRA_DIAGNOSTICS);
        if (sink != null) sink.send(kind.ordinal(), null);
    }

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        active = new WeakReference<>(this);
        String requestId = getIntent().getStringExtra(EXTRA_REQUEST_ID);
        if (requestId != null) activeRequestId = requestId;
        // Only the platform recognizer needs the system service. A configured provider records
        // here, so a device without that service can still use voice input through one.
        if (!usesLocal() && !usesStreaming() && !usesProvider() && !available(this)) {
            diagnostic(DiagnosticEventKind.VOICE_PLATFORM_UNAVAILABLE);
            fail("设备没有可用的系统语音识别服务");
            finish();
            return;
        }
        if (checkSelfPermission(android.Manifest.permission.RECORD_AUDIO)
            != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] {android.Manifest.permission.RECORD_AUDIO},
                REQUEST_RECORD_AUDIO);
            return;
        }
        recognitionPending = true;
    }

    @Override protected void onResume() {
        super.onResume();
        resumed = true;
        startPendingRecognition();
    }

    @Override protected void onPause() {
        resumed = false;
        super.onPause();
    }

    /** 前台录音能力在恢复后才可用；授权回调早于恢复时只记住请求，避免重试仍在后台状态被拒。 */
    private void startPendingRecognition() {
        if (!resumed || !recognitionPending || finished || stopping || isFinishing()) return;
        recognitionPending = false;
        startRecognition();
    }

    @Override public void onRequestPermissionsResult(int requestCode, String[] permissions,
            int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode != REQUEST_RECORD_AUDIO || finished) return;
        if (grantResults.length == 1 && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
            recognitionPending = true;
            startPendingRecognition();
        } else {
            diagnostic(DiagnosticEventKind.VOICE_FAILURE_PERMISSION);
            fail("语音识别需要麦克风权限");
            finishRequest();
        }
    }

    /** Requests that the recognizer finish the current utterance and deliver final results. */
    public static void stopActive() {
        VoiceRecognitionActivity activity = active.get();
        if (activity != null) {
            activity.runOnUiThread(activity::stopRecognition);
        }
    }

    /** Stops the platform recognizer launched for the shared Tauri voice panel. */
    public static void cancelActive() {
        VoiceRecognitionActivity activity = active.get();
        if (activity != null) activity.runOnUiThread(activity::cancelRecognition);
    }

    @Override protected void onStop() {
        super.onStop();
        // A dialog activity can be stopped by Home, the lock screen or another app without being
        // destroyed. Do not leave a microphone or provider request alive behind that screen. The
        // permission prompt is also a stopped state, so only cancel once an engine was created.
        if (!isChangingConfigurations() && !isFinishing() && !finished
                && (recognizer != null || provider != null || streaming != null || local != null)) {
            cancelRecognition();
        }
    }

    @Override protected void onDestroy() {
        finished = true;
        voicePolisher.cancel();
        if (local != null) {
            // Releases the microphone and stops a decode; the loaded model stays cached for the next dictation and is dropped after it has been idle (LocalAsrPolicy).
            local.cancel();
            local = null;
        }
        if (streaming != null) {
            streaming.cancel();
            streaming = null;
        }
        if (provider != null) {
            // The recorder holds the microphone until it is told to stop, and a worker outliving
            // this window would keep it past the point anything can use the result.
            provider.cancel();
            provider = null;
        }
        if (providerWorker != null) {
            providerWorker.shutdownNow();
            providerWorker = null;
        }
        if (recognizer != null) {
            recognizer.destroy();
            recognizer = null;
        }
        if (active.get() == this) {
            active = new WeakReference<>(null);
            // A configuration change recreates the activity for the same request.
            // Keep the request visible while the replacement activity registers
            // itself; otherwise the Tauri-side poller settles the job as cancelled.
            if (!isChangingConfigurations()) {
                clearRequest(getIntent().getStringExtra(EXTRA_REQUEST_ID));
            }
        }
        super.onDestroy();
    }

    /** Whether this request is on-device recognition with an installed model directory. */
    private boolean usesLocal() {
        return LocalAsrPolicy.usable(LocalAsrPolicy.PROVIDER,
            getIntent().getStringExtra(EXTRA_LOCAL_MODEL));
    }

    /** Whether this request is the streaming protocol rather than an upload. */
    private boolean usesStreaming() {
        return getIntent().getStringExtra(EXTRA_STREAM_ENDPOINT) != null;
    }

    /** Whether this request carries a provider configuration this host can actually speak. */
    private boolean usesProvider() {
        Intent intent = getIntent();
        return HttpAsrPolicy.usable(intent.getStringExtra(EXTRA_REQUEST_FORMAT),
            intent.getStringExtra(EXTRA_ENDPOINT), intent.getStringExtra(EXTRA_MODEL),
            intent.getStringExtra(EXTRA_TOKEN));
    }

    private void startRecognition() {
        if (finished) return;
        if (usesLocal()) {
            startLocalRecognition();
            return;
        }
        if (usesStreaming()) {
            startStreamingRecognition();
            return;
        }
        if (usesProvider()) {
            startProviderRecognition();
            return;
        }
        if (recognizer != null) return;
        showRecordingControls();
        recordingHint.setText("说完后稍停即可自动识别；点「完成」立即结束，「取消」会丢弃这次录音。");
        diagnostic(DiagnosticEventKind.VOICE_ENGINE_PLATFORM);
        try {
            startPlatformRecognition();
        } catch (SecurityException refused) {
            platformStartFailed(SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS);
        } catch (RuntimeException unavailable) {
            platformStartFailed(SpeechRecognizer.ERROR_CLIENT);
        }
    }

    private void startPlatformRecognition() {
        recognizer = SpeechRecognizer.createSpeechRecognizer(this);
        recognizer.setRecognitionListener(new RecognitionListener() {
            @Override public void onReadyForSpeech(Bundle params) { }
            @Override public void onBeginningOfSpeech() { }
            @Override public void onRmsChanged(float rmsdB) { }
            @Override public void onBufferReceived(byte[] buffer) { }
            @Override public void onEndOfSpeech() { }
            @Override public void onError(int error) {
                if (!finished) {
                    diagnostic(VoiceDiagnostics.platformFailure(error));
                    // 带上错误码：同一句「未返回结果」分不出网络、权限、没听到声音还是服务不可用（#5553）。
                    if (!stopping) fail(PlatformSpeechPolicy.message(error));
                    finishRequest();
                }
            }
            @Override public void onResults(Bundle results) {
                if (finished) return;
                ArrayList<String> values = results == null
                    ? null : results.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION);
                if (values != null && !values.isEmpty()) {
                    // Off the main thread: polishing is a network round trip, and onResults is
                    // delivered on the thread drawing this window.
                    deliver(values.get(0));
                    return;
                }
                diagnostic(DiagnosticEventKind.VOICE_EMPTY);
                // 服务报了成功却没有文字：和键盘里一样说清楚，不再悄无声息地关掉窗口（#5553）。
                if (!stopping) fail(PlatformSpeechPolicy.emptyResult());
                finishRequest();
            }
            @Override public void onPartialResults(Bundle partialResults) { }
            @Override public void onEvent(int eventType, Bundle params) { }
        });
        recognizer.startListening(recognitionIntent(getPackageName(),
            getIntent().getStringExtra(EXTRA_LANGUAGE), false));
    }

    /** 绑定服务也可能同步抛异常；先释放失败的识别器，结束请求时不再向它发送停止命令。 */
    private void platformStartFailed(int error) {
        diagnostic(VoiceDiagnostics.platformFailure(error));
        SpeechRecognizer failed = recognizer;
        recognizer = null;
        if (failed != null) {
            try {
                failed.destroy();
            } catch (RuntimeException unavailable) {
                // 服务已经失效时仍要清除请求并关闭窗口。
            }
        }
        if (finished) return;
        fail(PlatformSpeechPolicy.message(error));
        finishRequest();
    }

    /**
     * Record, upload and hand back the text, off the main thread.
     *
     * <p>The recording runs until the user stops it, then the upload waits on a network round
     * trip; neither belongs on the thread drawing this window.
     */
    private void startProviderRecognition() {
        if (provider != null) return;
        diagnostic(DiagnosticEventKind.VOICE_ENGINE_HTTP);
        showRecordingControls();
        Intent intent = getIntent();
        String requestId = intent.getStringExtra(EXTRA_REQUEST_ID);
        String language = intent.getStringExtra(EXTRA_LANGUAGE);
        String requestFormat = intent.getStringExtra(EXTRA_REQUEST_FORMAT);
        String endpoint = intent.getStringExtra(EXTRA_ENDPOINT);
        String model = intent.getStringExtra(EXTRA_MODEL);
        String token = intent.getStringExtra(EXTRA_TOKEN);
        provider = new HttpAsrRecognizer();
        providerWorker = Executors.newSingleThreadExecutor();
        HttpAsrRecognizer running = provider;
        providerWorker.execute(() -> {
            String text = null;
            String message = null;
            String notice = null;
            try {
                text = running.recognize(requestId, language, requestFormat, endpoint, model, token);
                if (running.capped()) notice = VoiceFailurePolicy.capped(HttpAsrRecognizer.MAX_MILLIS);
            } catch (HttpAsrRecognizer.Refused refused) {
                diagnostic(VoiceFailurePolicy.diagnostic(refused.failure()));
                message = VoiceFailurePolicy.message(refused.failure());
            } catch (RuntimeException | LinkageError error) {
                diagnostic(VoiceDiagnostics.unexpected(error));
                // 识别组件的意外失败也必须结束请求，不能让录音页永远停在转写中。
                message = "语音识别服务未响应，请稍后重试";
            }
            String finalText = text;
            String finalMessage = message;
            String finalNotice = notice;
            String polishedText = finalText == null ? null : polished(finalText);
            runOnUiThread(() -> {
                if (finished) return;
                if (polishedText != null) {
                    saveResult(polishedText);
                    if (finalNotice != null) fail(finalNotice);
                } else if (finalMessage != null) {
                    fail(finalMessage);
                }
                finishRequest();
            });
        });
    }

    /**
     * A window that says it is recording, and a way to end the recording and keep the result.
     *
     * <p>这个 Activity 是对话框主题，自己从不设置内容视图，没有这里的话每条路径都只在屏幕上放一个空白框。服务商上传会一直录到六十秒上限、豆包流式录到五分钟上限，没有这两个按钮时唯一的出路是返回键，而返回会取消并丢掉转写结果，没法表达「我说完了，开始转写」。系统识别服务在说话人停顿时会自己结束，但什么都不画的话，麦克风开着时屏幕上只有对话框的灰色蒙层，看不出有任何东西在听（#5553），所以它也用同一个窗口。
     *
     * <p>Built in code rather than as a layout, which is how this host builds its keyboard: the
     * two buttons are the whole surface, and a resource file for them would be one more place for
     * the wording to drift out of step with what the buttons do.
     */
    private void showRecordingControls() {
        LinearLayout root = LayoutPolicy.column(this);
        int pad = DimensionPolicy.pixels(this, 20);
        ViewPolicy.setPadding(root, pad, pad, pad, pad);
        TextView title = ViewPolicy.textLabel(this, "正在录音", 18);
        recordingTitle = title;
        root.addView(title);
        TextView hint = ViewPolicy.textLabel(this,
            "说完后点「完成」开始转写；「取消」会丢弃这次录音。", 14);
        recordingHint = hint;
        ViewPolicy.setPadding(hint, 0, pad / 2, 0, pad);
        root.addView(hint);
        LinearLayout actions = LayoutPolicy.row(this);
        ViewPolicy.setGravity(actions, Gravity.END);
        Button cancel = ViewPolicy.newPressButton(this);
        cancel.setText("取消");
        cancel.setContentDescription("取消录音并丢弃结果");
        ViewPolicy.bindClick(cancel, this::cancelRecognition);
        actions.addView(cancel);
        Button done = ViewPolicy.newPressButton(this);
        done.setText("完成");
        done.setContentDescription("结束录音并开始转写");
        ViewPolicy.bindClick(done, () -> {
            ViewPolicy.setEnabled(done, false);
            title.setText("正在转写");
            // Local recognition has already shown the text as it was spoken; keep it on screen while the last words are decoded rather than replacing it with a status line.
            if (local == null) hint.setText("正在把录音交给识别服务，请稍候。");
            stopRecognition();
        });
        actions.addView(done);
        root.addView(actions, LayoutPolicy.matchWidthWrapParams());
        setContentView(root);
    }

    /**
     * Dictate with the installed on-device model, showing the text as it is recognised.
     *
     * <p>Everything runs on the worker: the first dictation after the model was dropped loads hundreds of megabytes from storage, and every one decodes while the user speaks. Hotwords are the ones the launcher resolved (the Tauri voice panel passes them in), otherwise read from the user's dictionary through the same runtime options the keyboard uses.
     */
    private void startLocalRecognition() {
        if (local != null) return;
        diagnostic(DiagnosticEventKind.VOICE_ENGINE_LOCAL);
        showRecordingControls();
        recordingHint.setText("正在本机识别，音频不会离开设备。说完后点「完成」；「取消」会丢弃这次录音。");
        LocalAsrRecognizer.watchMemory(this);
        Intent intent = getIntent();
        String modelDirectory = intent.getStringExtra(EXTRA_LOCAL_MODEL);
        String language = intent.getStringExtra(EXTRA_LANGUAGE);
        String[] hotwordTexts = intent.getStringArrayExtra(EXTRA_LOCAL_HOTWORD_TEXTS);
        String[] hotwordPinyin = intent.getStringArrayExtra(EXTRA_LOCAL_HOTWORD_PINYIN);
        File files = getFilesDir();
        local = new LocalAsrRecognizer();
        local.diagnostics(this::diagnostic);
        if (providerWorker == null) providerWorker = Executors.newSingleThreadExecutor();
        LocalAsrRecognizer running = local;
        providerWorker.execute(() -> {
            String text = null;
            String message = null;
            try {
                // 识别窗口由「完成」结束、不按停顿判停，判停灵敏度在这里换不来好处，传 0 让阈值保持 0.5。
                running.vadSensitivity(0);
                text = running.recognize(modelDirectory, files == null ? null : files.toPath(), language,
                    HostOptionsPolicy.readRuntimeOptions(files),
                    hotwordTexts, hotwordPinyin,
                    partial -> runOnUiThread(() -> {
                        if (!finished && recordingHint != null) recordingHint.setText(partial);
                    }));
            } catch (LocalAsrRecognizer.Refused refused) {
                diagnostic(refused.diagnosticKind());
                message = LocalAsrPolicy.failureMessage(refused.failure(), refused.runtimeMissing());
            } catch (RuntimeException | LinkageError error) {
                diagnostic(VoiceDiagnostics.unexpected(error));
                // 本地模型或 JNI 的意外失败也要回到可重试的页面状态。
                message = "本地语音识别组件无法加载";
            }
            String finalMessage = message;
            String polishedText = text == null ? null : polished(text);
            runOnUiThread(() -> {
                if (finished) return;
                if (polishedText != null) saveResult(polishedText);
                else if (finalMessage != null) fail(finalMessage);
                finishRequest();
            });
        });
    }

    /** Stream while the user speaks, then deliver the provider's final result. */
    private void startStreamingRecognition() {
        if (streaming != null) return;
        diagnostic(DiagnosticEventKind.VOICE_ENGINE_STREAMING);
        showRecordingControls();
        Intent intent = getIntent();
        String endpoint = intent.getStringExtra(EXTRA_STREAM_ENDPOINT);
        String[] headers = intent.getStringArrayExtra(EXTRA_STREAM_HEADERS);
        boolean itn = intent.getBooleanExtra(EXTRA_STREAM_ITN, false);
        boolean punctuation = intent.getBooleanExtra(EXTRA_STREAM_PUNC, false);
        boolean ddc = intent.getBooleanExtra(EXTRA_STREAM_DDC, false);
        String boosting = intent.getStringExtra(EXTRA_STREAM_BOOSTING);
        streaming = new DoubaoRecognizer();
        if (providerWorker == null) providerWorker = Executors.newSingleThreadExecutor();
        DoubaoRecognizer running = streaming;
        providerWorker.execute(() -> {
            String text = null;
            String message = null;
            String notice = null;
            try {
                text = running.recognize(endpoint, headers, itn, punctuation, ddc, boosting,
                    null);
                HttpAsrRecognizer.Failure interrupted = running.interruption();
                if (interrupted != null) {
                    diagnostic(VoiceFailurePolicy.diagnostic(interrupted));
                    notice = VoiceFailurePolicy.interrupted(interrupted);
                } else if (running.capped()) {
                    notice = VoiceFailurePolicy.capped(DoubaoRecognizer.MAX_MILLIS);
                }
            } catch (HttpAsrRecognizer.Refused refused) {
                diagnostic(VoiceFailurePolicy.diagnostic(refused.failure()));
                message = VoiceFailurePolicy.message(refused.failure());
            } catch (RuntimeException | LinkageError error) {
                diagnostic(VoiceDiagnostics.unexpected(error));
                // 参数、TLS 或共享 JNI 的意外失败不能让请求悬挂在录音窗口。
                message = "语音识别服务未响应，请稍后重试";
            }
            String polishedText = text == null ? null : polished(text);
            String finalMessage = message;
            String finalNotice = notice;
            runOnUiThread(() -> {
                if (finished) return;
                if (polishedText != null && !polishedText.isEmpty()) {
                    saveResult(polishedText);
                    if (finalNotice != null) fail(finalNotice);
                } else if (finalMessage != null) {
                    fail(finalMessage);
                } else if (polishedText != null) {
                    // 识别出了文字、润色却给回空串；取消时两者都是 null，什么也不提示。
                    diagnostic(DiagnosticEventKind.VOICE_FAILURE_RESPONSE);
                    fail("语音识别未返回结果");
                }
                finishRequest();
            });
        });
    }

    private void stopRecognition() {
        if (finished) return;
        stopping = true;
        if (local != null) {
            local.stop();
            return;
        }
        if (streaming != null) {
            streaming.stop();
            return;
        }
        if (provider != null) {
            provider.stop();
            return;
        }
        if (recognizer != null) {
            recognizer.stopListening();
        } else {
            // 等待恢复或授权时还没有识别器；停止必须撤销待启动请求，不能恢复后再开始录音。
            finishRequest();
        }
    }

    private void cancelRecognition() {
        if (finished) return;
        finished = true;
        recognitionPending = false;
        voicePolisher.cancel();
        if (local != null) local.cancel();
        if (streaming != null) streaming.cancel();
        if (provider != null) provider.cancel();
        if (recognizer != null) recognizer.cancel();
        clearRequest(getIntent().getStringExtra(EXTRA_REQUEST_ID));
        finish();
    }

    private void finishRequest() {
        if (finished) return;
        finished = true;
        recognitionPending = false;
        if (recognizer != null) recognizer.stopListening();
        if (provider != null) provider.stop();
        if (streaming != null) streaming.stop();
        if (local != null) local.stop();
        clearRequest(getIntent().getStringExtra(EXTRA_REQUEST_ID));
        finish();
    }

    private void fail(String message) {
        Toast.makeText(this, message, Toast.LENGTH_LONG).show();
    }

    /** Polish on a worker, then save and finish on the thread that owns this window. */
    private void deliver(String text) {
        if (getIntent().getStringExtra(EXTRA_POLISH_ENDPOINT) == null) {
            saveResult(text);
            finishRequest();
            return;
        }
        if (providerWorker == null) providerWorker = Executors.newSingleThreadExecutor();
        providerWorker.execute(() -> {
            String result = polished(text);
            runOnUiThread(() -> {
                if (finished) return;
                saveResult(result);
                finishRequest();
            });
        });
    }

    /** Return the requested rewrite, or the original transcript if polishing fails. */
    private String polished(String text) {
        Intent intent = getIntent();
        String endpoint = intent.getStringExtra(EXTRA_POLISH_ENDPOINT);
        if (endpoint == null) return text;
        String polished = voicePolisher.polish(endpoint,
            intent.getStringExtra(EXTRA_POLISH_MODEL),
            intent.getStringExtra(EXTRA_POLISH_TOKEN),
            intent.getStringExtra(EXTRA_POLISH_PROMPT), text);
        return polished == null ? text : polished;
    }

    /**
     * 存下最终结果，交给键盘插入。所有识别服务和润色都在这之前结束，语音标点（{@link VoicePunctuation}）在这里按偏好处理一次；只剩标点时不存，提示没有识别到文字。
     */
    @SuppressWarnings("deprecation")
    private void saveResult(String recognized) {
        File files = getFilesDir();
        String text = VoicePunctuation.apply(recognized, files == null ? VoicePunctuation.AUTO
            : VoiceConfiguration.read(HostOptionsPolicy.readOption(files, "preferences_directory"),
                getIntent().getStringExtra(EXTRA_REQUEST_ID)).punctuationMode());
        if (text != null && text.isEmpty() && recognized != null && !recognized.isEmpty()) {
            diagnostic(DiagnosticEventKind.VOICE_EMPTY);
            Toast.makeText(this, "没有识别到文字", Toast.LENGTH_SHORT).show();
            return;
        }
        diagnostic(DiagnosticEventKind.VOICE_SUCCESS);
        if (files == null) {
            Toast.makeText(this, "语音结果无法保存", Toast.LENGTH_SHORT).show();
            return;
        }
        try {
            VoiceResultStore.Entry entry = new VoiceResultStore(files.toPath().resolve("voice-handoff"))
                .save(text, System.currentTimeMillis());
            ResultReceiver receiver = getIntent().getParcelableExtra(EXTRA_RESULT_RECEIVER);
            if (receiver == null) {
                Toast.makeText(this, "语音结果已发送到键盘，10 分钟内可插入", Toast.LENGTH_LONG).show();
            } else {
                Bundle payload = new Bundle();
                payload.putString(RESULT_ID, entry.id().toString());
                receiver.send(Activity.RESULT_OK, payload);
            }
        } catch (VoiceResultStore.Failure error) {
            String message = error.reason() == VoiceResultStore.Reason.BUSY
                ? "语音结果正在更新，请稍后重试" : "语音结果无法保存";
            Toast.makeText(this, message, Toast.LENGTH_SHORT).show();
        }
    }

    /**
     * 交给系统识别服务的请求，识别窗口和键盘里的聆听共用。带上调用方包名：有的识别服务按它区分调用方，缺了会直接拒绝。`partialResults` 只在键盘里打开，聆听面板边听边显示识别出的文字。
     */
    static Intent recognitionIntent(String callingPackage, String language, boolean partialResults) {
        Intent intent = new Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH);
        intent.putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL,
            RecognizerIntent.LANGUAGE_MODEL_FREE_FORM);
        intent.putExtra(RecognizerIntent.EXTRA_LANGUAGE, safeLanguage(language));
        intent.putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1);
        intent.putExtra(RecognizerIntent.EXTRA_PROMPT, "水杉语音输入");
        if (callingPackage != null) intent.putExtra(RecognizerIntent.EXTRA_CALLING_PACKAGE, callingPackage);
        if (partialResults) intent.putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true);
        return intent;
    }

    private static String safeLanguage(String language) {
        if (TextPolicy.boundedNonEmpty(language, VoiceContributionApi.MAX_METADATA_FIELD_LENGTH)) {
            Locale locale = Locale.forLanguageTag(language.replace('_', '-'));
            if (!locale.getLanguage().isEmpty()) return locale.toLanguageTag();
        }
        String fallback = Locale.getDefault().toLanguageTag();
        return fallback.isEmpty() ? "zh-CN" : fallback;
    }
}
