package app.msime.android;

import app.msime.android.policy.HostOptionsPolicy;
import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Bundle;
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
    private static final int REQUEST_RECORD_AUDIO = 1;
    private static final String EXTRA_LANGUAGE = "app.msime.android.voice.LANGUAGE";
    private static final String EXTRA_REQUEST_ID = "app.msime.android.voice.REQUEST_ID";
    private static final String EXTRA_PROVIDER = "app.msime.android.voice.PROVIDER";
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

    public static boolean available(Context context) {
        return SpeechRecognizer.isRecognitionAvailable(context);
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
        launch(context, requestId, language, null, null, null, null, null, null);
    }

    /**
     * Launch with a configured transcription provider, or without one to use the platform
     * recognizer. The three provider values are resolved and validated by the shared layer; this
     * activity only checks that it can speak that protocol before using them.
     */
    public static void launch(Context context, String requestId, String language,
                              String provider, String endpoint, String model, String token,
                              Streaming streaming, Polish polish) {
        launch(context, requestId, language, provider, endpoint, model, token, streaming, polish,
            null);
    }

    /**
     * Launch with every engine this activity has. `localModel` is the installed model directory for on-device recognition, already resolved by the shared layer; when present it wins over the network settings, which the shared resolution leaves empty for provider `local` anyway.
     */
    public static void launch(Context context, String requestId, String language,
                              String provider, String endpoint, String model, String token,
                              Streaming streaming, Polish polish, String localModel) {
        launch(context, requestId, language, provider, endpoint, model, token, streaming, polish,
            localModel, null, null);
    }

    /**
     * {@link #launch(Context, String, String, String, String, String, String, Streaming, Polish, String)} with the hotwords the shared layer already resolved for `localModel`: the Tauri request carries them as parallel `text` / `pinyin` arrays. Null reads them from the user's dictionary when the dictation starts.
     */
    public static void launch(Context context, String requestId, String language,
                              String provider, String endpoint, String model, String token,
                              Streaming streaming, Polish polish, String localModel,
                              String[] localHotwordTexts, String[] localHotwordPinyin) {
        markLaunched(requestId);
        Intent intent = new Intent(context, VoiceRecognitionActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        intent.putExtra(EXTRA_REQUEST_ID, requestId);
        intent.putExtra(EXTRA_LANGUAGE, safeLanguage(language));
        if (provider != null) intent.putExtra(EXTRA_PROVIDER, provider);
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

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        active = new WeakReference<>(this);
        String requestId = getIntent().getStringExtra(EXTRA_REQUEST_ID);
        if (requestId != null) activeRequestId = requestId;
        // Only the platform recognizer needs the system service. A configured provider records
        // here, so a device without that service can still use voice input through one.
        if (!usesLocal() && !usesStreaming() && !usesProvider() && !available(this)) {
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
        startRecognition();
    }

    @Override public void onRequestPermissionsResult(int requestCode, String[] permissions,
            int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        if (requestCode != REQUEST_RECORD_AUDIO || finished) return;
        if (grantResults.length == 1 && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
            startRecognition();
        } else {
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
        return HttpAsrPolicy.usable(intent.getStringExtra(EXTRA_PROVIDER),
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
        recognizer = SpeechRecognizer.createSpeechRecognizer(this);
        recognizer.setRecognitionListener(new RecognitionListener() {
            @Override public void onReadyForSpeech(Bundle params) { }
            @Override public void onBeginningOfSpeech() { }
            @Override public void onRmsChanged(float rmsdB) { }
            @Override public void onBufferReceived(byte[] buffer) { }
            @Override public void onEndOfSpeech() { }
            @Override public void onError(int error) {
                if (!finished) {
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

    /**
     * Record, upload and hand back the text, off the main thread.
     *
     * <p>The recording runs until the user stops it, then the upload waits on a network round
     * trip; neither belongs on the thread drawing this window.
     */
    private void startProviderRecognition() {
        if (provider != null) return;
        showRecordingControls();
        Intent intent = getIntent();
        String requestId = intent.getStringExtra(EXTRA_REQUEST_ID);
        String language = intent.getStringExtra(EXTRA_LANGUAGE);
        String endpoint = intent.getStringExtra(EXTRA_ENDPOINT);
        String model = intent.getStringExtra(EXTRA_MODEL);
        String token = intent.getStringExtra(EXTRA_TOKEN);
        provider = new HttpAsrRecognizer();
        providerWorker = Executors.newSingleThreadExecutor();
        HttpAsrRecognizer running = provider;
        providerWorker.execute(() -> {
            String text = null;
            String message = null;
            try {
                text = running.recognize(requestId, language, endpoint, model, token);
            } catch (HttpAsrRecognizer.Refused refused) {
                message = switch (refused.failure()) {
                    case PERMISSION -> "语音识别需要麦克风权限";
                    case UNAVAILABLE -> "麦克风被其他应用占用";
                    case NETWORK -> "语音服务未响应，请检查网络与密钥";
                    case EMPTY -> "没有听到内容";
                    case CANCELLED -> null;
                };
            } catch (RuntimeException | LinkageError error) {
                // 识别组件的意外失败也必须结束请求，不能让录音页永远停在转写中。
                message = "语音识别服务未响应，请稍后重试";
            }
            String finalText = text;
            String finalMessage = message;
            String polishedText = finalText == null ? null : polished(finalText);
            runOnUiThread(() -> {
                if (finished) return;
                if (polishedText != null) saveResult(polishedText);
                else if (finalMessage != null) fail(finalMessage);
                finishRequest();
            });
        });
    }

    /**
     * A window that says it is recording, and a way to end the recording and keep the result.
     *
     * <p>这个 Activity 是对话框主题，自己从不设置内容视图，没有这里的话每条路径都只在屏幕上放一个空白框。服务商上传和豆包流式会一直录到六十秒上限，没有这两个按钮时唯一的出路是返回键，而返回会取消并丢掉转写结果，没法表达「我说完了，开始转写」。系统识别服务在说话人停顿时会自己结束，但什么都不画的话，麦克风开着时屏幕上只有对话框的灰色蒙层，看不出有任何东西在听（#5553），所以它也用同一个窗口。
     *
     * <p>Built in code rather than as a layout, which is how this host builds its keyboard: the
     * two buttons are the whole surface, and a resource file for them would be one more place for
     * the wording to drift out of step with what the buttons do.
     */
    private void showRecordingControls() {
        LinearLayout root = KeyboardGeometry.column(this);
        int pad = KeyboardGeometry.pixels(this, 20);
        ViewPolicy.setPadding(root, pad, pad, pad, pad);
        TextView title = ViewPolicy.textLabel(this, "正在录音", 18);
        recordingTitle = title;
        root.addView(title);
        TextView hint = ViewPolicy.textLabel(this,
            "说完后点「完成」开始转写；「取消」会丢弃这次录音。", 14);
        recordingHint = hint;
        ViewPolicy.setPadding(hint, 0, pad / 2, 0, pad);
        root.addView(hint);
        LinearLayout actions = KeyboardGeometry.row(this);
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
        root.addView(actions, KeyboardGeometry.matchWidthWrapParams());
        setContentView(root);
    }

    /**
     * Dictate with the installed on-device model, showing the text as it is recognised.
     *
     * <p>Everything runs on the worker: the first dictation after the model was dropped loads hundreds of megabytes from storage, and every one decodes while the user speaks. Hotwords are the ones the launcher resolved (the Tauri voice panel passes them in), otherwise read from the user's dictionary through the same runtime options the keyboard uses.
     */
    private void startLocalRecognition() {
        if (local != null) return;
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
        if (providerWorker == null) providerWorker = Executors.newSingleThreadExecutor();
        LocalAsrRecognizer running = local;
        providerWorker.execute(() -> {
            String text = null;
            String message = null;
            try {
                text = running.recognize(modelDirectory, files.toPath(), language,
                    HostOptionsPolicy.readRuntimeOptions(files),
                    hotwordTexts, hotwordPinyin,
                    partial -> runOnUiThread(() -> {
                        if (!finished && recordingHint != null) recordingHint.setText(partial);
                    }));
            } catch (LocalAsrRecognizer.Refused refused) {
                message = switch (refused.failure()) {
                    case PERMISSION -> "语音识别需要麦克风权限";
                    case UNAVAILABLE -> "麦克风被其他应用占用";
                    case MODEL -> "本地语音模型未安装或已损坏，请在设置中重新下载";
                    case RUNTIME -> refused.runtimeMissing()
                        ? "本地语音识别组件尚未下载，请在设置中下载后再试"
                        : "本地语音识别组件无法加载";
                    case EMPTY -> "没有听到内容";
                    case CANCELLED -> null;
                };
            } catch (RuntimeException | LinkageError error) {
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
            try {
                text = running.recognize(endpoint, headers, itn, punctuation, ddc, boosting,
                    null);
            } catch (RuntimeException | LinkageError error) {
                // 参数、TLS 或共享 JNI 的意外失败不能让请求悬挂在录音窗口。
                message = "语音识别服务未响应，请稍后重试";
            }
            String polishedText = text == null ? null : polished(text);
            String finalMessage = message;
            runOnUiThread(() -> {
                if (finished) return;
                if (polishedText != null && !polishedText.isEmpty()) saveResult(polishedText);
                else if (finalMessage != null) fail(finalMessage);
                else fail("语音识别未返回结果");
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
        if (recognizer != null) recognizer.stopListening();
    }

    private void cancelRecognition() {
        if (finished) return;
        finished = true;
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

    private void saveResult(String text) {
        File files = getFilesDir();
        if (files == null) {
            Toast.makeText(this, "语音结果无法保存", Toast.LENGTH_SHORT).show();
            return;
        }
        try {
            new VoiceResultStore(files.toPath().resolve("voice-handoff"))
                .save(text, System.currentTimeMillis());
            Toast.makeText(this, "语音结果已发送到键盘，10 分钟内可插入", Toast.LENGTH_LONG).show();
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
        if (language != null && !language.isEmpty()
                && TextPolicy.utf8Length(language) <= VoiceContributionApi.MAX_METADATA_FIELD_LENGTH
                && !TextPolicy.hasControl(language) && TextPolicy.validUnicode(language)) {
            Locale locale = Locale.forLanguageTag(language.replace('_', '-'));
            if (!locale.getLanguage().isEmpty()) return locale.toLanguageTag();
        }
        String fallback = Locale.getDefault().toLanguageTag();
        return fallback.isEmpty() ? "zh-CN" : fallback;
    }
}
