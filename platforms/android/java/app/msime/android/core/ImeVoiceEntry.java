package app.msime.android;

import app.msime.android.LayoutPolicy;

import app.msime.android.policy.HostOptionsPolicy;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.Bundle;
import android.os.SystemClock;
import android.speech.RecognitionListener;
import android.speech.SpeechRecognizer;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import java.io.File;
import java.time.LocalDate;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 键盘内的语音识别：进程内能跑的识别器（本机 sherpa 模型、豆包流式）和设备自带的系统识别服务（SpeechRecognizer）都直接在键区里聆听，键区换成 {@link VoiceListeningView}（「正在聆听…」「点任意处取消」，系统识别服务报音量时圆盘外的光圈随音量涨落），识别完自动上屏并短暂显示「已识别：…」。OpenAI 兼容的上传、还没有麦克风权限（要由 Activity 申请）、系统识别服务在键盘里没能开始聆听这几种情况仍走 VoiceRecognitionActivity（返回 false，或识别失败后由这里转交）。
 *
 * <p>系统识别服务以前一律走识别窗口：窗口抢走焦点，键盘随之收起，屏幕上只剩对话框主题的灰色蒙层，看不到任何正在录音的提示，结果还要再到语音结果面板里手动插入（#5553）。键盘可见时系统给输入法进程前台的麦克风能力，本机模型与豆包也是在这里录音的，所以系统识别服务同样留在键区里。
 *
 * <p>`voice_input.offline_fallback` 打开、没有网络且 `voice_input.asr_model_path` 指向已安装的本机模型时，改用本机识别。没有麦克风权限时交回 Activity 去申请。识别结束且隐私判断允许时记语音时长（`record_voice`）；`voice_input.contribute_audio` 打开、隐私判断允许且不是密码框时，经 {@link VoiceContributionApi} 上传这次的音频与识别文本。
 */
final class ImeVoiceEntry {
    /** 系统识别服务在语音时长统计与贡献里用的 provider 名。 */
    static final String PLATFORM_PROVIDER = "system";
    /** 「已识别：…」停留的时间。 */
    static final long DONE_NOTICE_MILLIS = 1200;
    /** 听到内容之后这么久没有新的识别文字，就当说完了，结束录音并出结果（与系统识别器停顿后自动结束一致）。 */
    static final long SILENCE_STOP_MILLIS = 1500;

    /** 键盘里接手的识别方式。 */
    enum Engine { LOCAL, STREAMING, PLATFORM }

    private final MSIMEInputService s;
    private final ExecutorService worker = Executors.newSingleThreadExecutor(runnable -> {
        Thread thread = new Thread(runnable, "msime-keyboard-voice");
        thread.setDaemon(true);
        return thread;
    });
    private Runnable silenceStop;
    private VoiceListeningView listening;
    private ViewGroup host;
    private final ArrayList<View> hidden = new ArrayList<>();
    private LocalAsrRecognizer local;
    private DoubaoRecognizer streaming;
    private SpeechRecognizer platform;
    /** 系统识别服务已经报过 onReadyForSpeech：之后的失败不再转交识别窗口。 */
    private boolean platformListening;
    /** 用户已经按语音键结束了这次系统识别：之后报的错（例如还没开始聆听就被叫停时的 ERROR_CLIENT）不再转交识别窗口，否则按两下语音键会弹出一个识别窗口重新录音。 */
    private boolean platformStopped;
    private long generation;
    private boolean recognizing;

    ImeVoiceEntry(MSIMEInputService s) {
        this.s = s;
    }

    /**
     * 选键盘里的识别方式：已配置本机模型时用本机；豆包流式在线时用豆包，离线且允许回退、本机模型已装好时用本机；没有配置服务商且设备有系统识别服务时用系统识别服务；配置了上传式服务商，或者设备没有系统识别服务时返回 null 交给 Activity。离线回退对系统识别服务和上传式服务商同样适用。
     */
    static Engine choose(boolean localConfigured, boolean streamingConfigured, boolean online,
                         boolean offlineFallback, boolean fallbackInstalled,
                         boolean uploadConfigured, boolean platformAvailable) {
        if (localConfigured) return Engine.LOCAL;
        boolean fallback = !online && offlineFallback && fallbackInstalled;
        if (streamingConfigured) return fallback ? Engine.LOCAL : Engine.STREAMING;
        if (fallback) return Engine.LOCAL;
        return !uploadConfigured && platformAvailable ? Engine.PLATFORM : null;
    }

    private JSONObject voicePreferences() {
        JSONObject preferences = s.preferencesSnapshot == null ? null
            : s.preferencesSnapshot.optJSONObject("preferences");
        JSONObject voice = preferences == null ? null : preferences.optJSONObject("voice_input");
        return voice == null ? new JSONObject() : voice;
    }

    /** 有可用的、经过验证的网络；读不到网络状态（缺少权限等）时按在线处理，不贸然回退。 */
    private boolean online() {
        try {
            ConnectivityManager manager = s.getSystemService(ConnectivityManager.class);
            if (manager == null) return true;
            Network network = manager.getActiveNetwork();
            if (network == null) return false;
            NetworkCapabilities capabilities = manager.getNetworkCapabilities(network);
            return capabilities != null && capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET);
        } catch (RuntimeException error) {
            return true;
        }
    }

    /** 在键区里开始识别；返回 false 表示交回原来的识别窗口流程。正在聆听时再按一次语音键即结束录音、开始出结果。 */
    boolean startInKeyboard(ViewGroup keyArea) {
        if (keyArea == null) return false;
        if (listening != null) {
            finishListening();
            return true;
        }
        if (s.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO)
                != PackageManager.PERMISSION_GRANTED) return false;
        // 正在组字时交回原来的流程：识别窗口与结果面板会提示先完成当前输入，结果不会落在组字旁边。
        if (!s.voiceInsertionReady()) return false;
        JSONObject voice = voicePreferences();
        String requestId = "ime-keyboard-" + Long.toUnsignedString(SystemClock.uptimeMillis());
        VoiceConfiguration configured = VoiceConfiguration.read(s.preferencesDirectory, requestId);
        File files = s.getFilesDir();
        String fallbackModel = JsonPolicy.strictStringOrEmpty(voice.opt("asr_model_path"));
        boolean fallbackInstalled = files != null && LocalAsrPolicy.usable(LocalAsrPolicy.PROVIDER, fallbackModel)
            && LocalAsrPolicy.installed(fallbackModel, files.toPath());
        boolean localConfigured = configured.localModel() != null;
        boolean streamingConfigured = configured.streaming() != null;
        boolean needsNetwork = !localConfigured;
        boolean uploadConfigured = !localConfigured && !streamingConfigured && configured.provider() != null;
        Engine engine = choose(localConfigured, streamingConfigured, !needsNetwork || online(),
            s.localSettings.bool(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK), fallbackInstalled,
            uploadConfigured, VoiceRecognitionActivity.available(s));
        if (engine == null) return false;
        String model = localConfigured ? configured.localModel() : fallbackModel;
        String provider = engine == Engine.LOCAL ? LocalAsrPolicy.PROVIDER : DoubaoAsrPolicy.PROVIDER;
        String rawLanguage = JsonPolicy.strictString(voice.opt("language"));
        final String language = rawLanguage == null ? "zh-CN" : rawLanguage;
        boolean contribute = s.localSettings.bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO) && s.imePrivacyGate.contributesVoice()
            && !EditorPolicy.password(s.editorInputType);
        // 记下开始聆听时的输入位置；结果出来时位置变了就不直接上屏。
        s.captureVoiceTarget();
        show(keyArea);
        long session = ++generation;
        long startedAt = SystemClock.uptimeMillis();
        recognizing = true;
        VoiceRecognitionActivity.Polish polish = configured.polish();
        VoiceRecognitionActivity.Streaming stream = configured.streaming();
        if (engine == Engine.PLATFORM) {
            if (startPlatform(session, startedAt, language, polish)) return true;
            // 系统识别服务连创建都失败时退回识别窗口，至少还有原来的流程可用。
            cancel();
            return false;
        }
        if (engine == Engine.LOCAL) {
            LocalAsrRecognizer.watchMemory(s);
            local = new LocalAsrRecognizer();
            local.retainAudio(contribute);
        } else {
            streaming = new DoubaoRecognizer();
            streaming.retainAudio(contribute);
        }
        LocalAsrRecognizer runningLocal = local;
        DoubaoRecognizer runningStream = streaming;
        String options = HostOptionsPolicy.readRuntimeOptions(files);
        try {
            worker.execute(() -> {
                String text = null;
                String failure = null;
                byte[] pcm = null;
                try {
                    if (runningLocal != null) {
                        text = runningLocal.recognize(model, files.toPath(), language, options, null, null,
                            partial -> s.main.post(() -> heard(session, partial)));
                        pcm = runningLocal.retainedAudio();
                    } else if (runningStream != null && stream != null) {
                        text = runningStream.recognize(stream.endpoint(), stream.headers(), stream.itn(),
                            stream.punctuation(), stream.ddc(), stream.boostingTableId(),
                            (update, last) -> s.main.post(() -> heard(session, update)));
                        pcm = runningStream.retainedAudio();
                        if (text == null) failure = "语音识别服务未响应，请稍后重试";
                    }
                } catch (LocalAsrRecognizer.Refused refused) {
                    failure = LocalAsrPolicy.failureMessage(refused.failure(), refused.runtimeMissing());
                } catch (RuntimeException | LinkageError error) {
                    failure = "语音识别服务无法启动";
                }
                String result = text == null ? null : TextPolicy.trimmed(text);
                if (result != null && !result.isEmpty() && polish != null) {
                    String polished = new VoicePolisher().polish(polish.endpoint(), polish.model(), polish.token(),
                        polish.prompt(), result);
                    if (TextPolicy.hasText(polished)) {
                        result = TextPolicy.trimmed(polished);
                    }
                }
                String finalText = result;
                String finalFailure = failure;
                byte[] audio = pcm;
                long elapsed = SystemClock.uptimeMillis() - startedAt;
                s.main.post(() -> delivered(session, finalText, finalFailure, audio, elapsed, language, provider));
            });
        } catch (RejectedExecutionException error) {
            // The recognizer was created before the worker accepted the task. If the service is
            // shutting down, release that recognizer before handing control back to the window
            // fallback; otherwise it can keep the microphone open after this path returns false.
            cancel();
            return false;
        }
        return true;
    }

    /** 识别中途的文字：显示在提示行，并在停顿后自动结束录音。 */
    private void heard(long session, String partial) {
        if (session != generation || listening == null || partial == null || partial.isEmpty()) return;
        listening.setHint(partial);
        // 系统识别服务自己判断说完了没有，这里再按停顿掐断会切掉还没说完的话。
        if (platform != null) return;
        if (silenceStop != null) s.main.removeCallbacks(silenceStop);
        Runnable stop = new Runnable() {
            @Override public void run() {
                if (silenceStop != this || session != generation) return;
                silenceStop = null;
                finishListening();
            }
        };
        silenceStop = stop;
        s.main.postDelayed(stop, SILENCE_STOP_MILLIS);
    }

    /** 结束录音（保留已录到的部分并出结果）。 */
    private void finishListening() {
        if (!recognizing) {
            dismiss();
            return;
        }
        if (local != null) local.stop();
        if (streaming != null) streaming.stop();
        if (platform != null) {
            platformStopped = true;
            platform.stopListening();
        }
        if (listening != null) listening.setHint("正在识别…");
    }

    /** 键盘服务销毁：停掉识别，并让排队中的语音时长与贡献上传跑完后结束线程。 */
    void shutdown() {
        cancel();
        worker.shutdown();
    }

    /** 取消：丢掉这次录音，恢复键区。 */
    void cancel() {
        generation++;
        recognizing = false;
        if (local != null) local.cancel();
        if (streaming != null) streaming.cancel();
        local = null;
        streaming = null;
        releasePlatform();
        dismiss();
    }

    /**
     * 在键区里调起系统识别服务；回调都在主线程。服务连创建或启动都失败时返回 false。
     */
    private boolean startPlatform(long session, long startedAt, String language,
                                  VoiceRecognitionActivity.Polish polish) {
        SpeechRecognizer recognizer;
        try {
            recognizer = SpeechRecognizer.createSpeechRecognizer(s);
        } catch (RuntimeException error) {
            return false;
        }
        platform = recognizer;
        platformListening = false;
        platformStopped = false;
        recognizer.setRecognitionListener(new RecognitionListener() {
            @Override public void onReadyForSpeech(Bundle params) {
                if (session == generation) platformListening = true;
            }
            @Override public void onBeginningOfSpeech() { }
            @Override public void onRmsChanged(float rmsdB) {
                if (session == generation && listening != null) {
                    listening.setLevel(PlatformSpeechPolicy.level(rmsdB));
                }
            }
            @Override public void onBufferReceived(byte[] buffer) { }
            @Override public void onEndOfSpeech() {
                if (session == generation && listening != null) {
                    // 说完后多数识别服务不再报音量，光圈直接收回，不能停在半截。
                    listening.resetLevel();
                    listening.setHint("正在识别…");
                }
            }
            @Override public void onError(int error) {
                platformFailed(session, error);
            }
            @Override public void onResults(Bundle results) {
                platformRecognized(session, startedAt, firstResult(results), language, polish);
            }
            @Override public void onPartialResults(Bundle partialResults) {
                heard(session, firstResult(partialResults));
            }
            @Override public void onEvent(int eventType, Bundle params) { }
        });
        try {
            recognizer.startListening(VoiceRecognitionActivity.recognitionIntent(s.getPackageName(),
                language, true));
        } catch (RuntimeException error) {
            releasePlatform();
            return false;
        }
        return true;
    }

    private static String firstResult(Bundle results) {
        if (results == null) return null;
        ArrayList<String> values = results.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION);
        return values == null || values.isEmpty() ? null : values.get(0);
    }

    /** 系统识别服务给出最终结果：需要润色时到工作线程上做，再回主线程上屏。 */
    private void platformRecognized(long session, long startedAt, String text, String language,
                                    VoiceRecognitionActivity.Polish polish) {
        if (session != generation) return;
        releasePlatform();
        String result = text == null ? null : TextPolicy.trimmed(text);
        if (result == null || result.isEmpty()) {
            delivered(session, null, PlatformSpeechPolicy.emptyResult(), null, 0, language, PLATFORM_PROVIDER);
            return;
        }
        long elapsed = SystemClock.uptimeMillis() - startedAt;
        if (polish == null) {
            delivered(session, result, null, null, elapsed, language, PLATFORM_PROVIDER);
            return;
        }
        if (listening != null) listening.setHint("正在润色…");
        String transcript = result;
        try {
            worker.execute(() -> {
                String polished = new VoicePolisher().polish(polish.endpoint(), polish.model(), polish.token(),
                    polish.prompt(), transcript);
                String finalText = TextPolicy.hasText(polished)
                    ? TextPolicy.trimmed(polished) : transcript;
                s.main.post(() -> delivered(session, finalText, null, null, elapsed, language, PLATFORM_PROVIDER));
            });
        } catch (RejectedExecutionException error) {
            delivered(session, transcript, null, null, elapsed, language, PLATFORM_PROVIDER);
        }
    }

    /** 系统识别服务报错：还没开始聆听就被拒的转交识别窗口再试，其余按错误码提示。 */
    private void platformFailed(long session, int error) {
        if (session != generation) return;
        boolean retry = PlatformSpeechPolicy.retryInActivity(error, platformListening || platformStopped);
        ImeLog.w("Platform speech recognizer failed in keyboard: error " + error);
        cancel();
        if (retry) {
            s.launchVoiceActivity();
            return;
        }
        s.notice(PlatformSpeechPolicy.message(error, VoiceRecognitionActivity.recognizerLabel(s)));
    }

    private void releasePlatform() {
        SpeechRecognizer recognizer = platform;
        platform = null;
        platformListening = false;
        platformStopped = false;
        if (recognizer != null) recognizer.destroy();
    }

    private void delivered(long session, String text, String failure, byte[] pcm, long elapsedMillis,
                           String language, String provider) {
        if (session != generation) return;
        recognizing = false;
        local = null;
        streaming = null;
        releasePlatform();
        if (text == null || text.isEmpty()) {
            if (failure != null) s.notice(failure);
            dismiss();
            return;
        }
        // 聆听可能长达一分钟：这期间用户点到了别处、应用改写了输入框，或者开始了组字，结果就不能盲目插在现在的光标处。与识别窗口那条路一样，先存进语音结果，由用户在结果面板里显式插入。
        if (!s.voiceInsertionReady() || !s.voiceTargetMatches()) {
            boolean kept = s.stashVoiceResult(text);
            s.notice(kept ? "输入位置已变化，结果已保留，可在语音结果中插入"
                : "输入位置已变化；结果已安全清除");
            dismiss();
            if (kept) s.showVoiceResult();
            return;
        }
        boolean committed = s.commitText(text, TypingSource.VOICE);
        if (!committed) {
            s.notice("编辑器拒绝插入；结果已安全清除");
            dismiss();
            return;
        }
        if (s.imePrivacyGate.recordsVoice()) recordVoice(elapsedMillis);
        if (pcm != null && pcm.length > 0 && s.imePrivacyGate.contributesVoice()
                && !EditorPolicy.password(s.editorInputType)) {
            contribute(pcm, text, language, provider);
        }
        if (listening != null) {
            listening.resetLevel();
            listening.setText("识别完成");
            listening.setHint("已识别：" + text);
            listening.setContentDescription("已识别：" + text);
            s.main.postDelayed(() -> {
                if (session == generation) dismiss();
            }, DONE_NOTICE_MILLIS);
        }
    }

    /** 统计目录：与打字统计相同（偏好目录，没有时是 bootstrap 的状态目录）。 */
    private String statisticsDirectory() {
        if (s.preferencesDirectory != null && !s.preferencesDirectory.isEmpty()) return s.preferencesDirectory;
        File files = s.getFilesDir();
        return HostOptionsPolicy.bootstrapStateDirectory(files);
    }

    private void recordVoice(long milliseconds) {
        String directory = statisticsDirectory();
        if (directory.isEmpty() || milliseconds <= 0) return;
        final String request;
        try {
            request = new JSONObject().put("directory", directory).put("action", new JSONObject()
                .put("operation", "record_voice").put("day", LocalDate.now().toString())
                .put("milliseconds", BoundsPolicy.atMost(milliseconds, 600_000L))).toString();
        } catch (JSONException error) {
            return;
        }
        try {
            worker.execute(() -> {
                try {
                    NativeClient.typingStatistics(request);
                } catch (RuntimeException | LinkageError error) {
                    ImeLog.w("Voice duration not recorded");
                }
            });
        } catch (RejectedExecutionException ignored) {
            // 键盘正在退出。
        }
    }

    private void contribute(byte[] pcm, String transcript, String language, String provider) {
        byte[] wav = WavAudio.wrap(pcm, pcm.length, WavAudio.SAMPLE_RATE);
        if (wav == null) return;
        String version;
        try {
            version = s.getPackageManager().getPackageInfo(s.getPackageName(), 0).versionName;
        } catch (PackageManager.NameNotFoundException error) {
            return;
        }
        VoiceContributionApi.Contribution contribution = new VoiceContributionApi.Contribution(language, provider,
            VoiceContributionApi.durationMillis(pcm.length), transcript, version == null ? "" : version, wav);
        if (!VoiceContributionApi.valid(contribution)) return;
        try {
            worker.execute(() -> {
                try {
                    new VoiceContributionApi(new CloudApi(s)).upload(contribution);
                } catch (CloudApi.Failure | RuntimeException error) {
                    ImeLog.w("Voice contribution not uploaded");
                }
            });
        } catch (RejectedExecutionException ignored) {
            // 键盘正在退出。
        }
    }

    /** 把键区的内容换成聆听面板；键区被重建（换布局、收起键盘）时自动取消。 */
    private void show(ViewGroup keyArea) {
        dismiss();
        host = keyArea;
        hidden.clear();
        hidden.ensureCapacity(keyArea.getChildCount());
        for (int index = 0; index < keyArea.getChildCount(); index++) {
            View child = keyArea.getChildAt(index);
            if (ViewPolicy.isVisible(child)) {
                hidden.add(child);
                ViewPolicy.setInvisible(child);
            }
        }
        VoiceListeningView view = new VoiceListeningView(s);
        view.setColors(Color.parseColor(s.skin.accent()), Color.parseColor(s.skin.onAccent()),
            Color.parseColor(s.skin.keyForeground()), Color.parseColor(s.skin.toolbarIcon()));
        ViewPolicy.bindClick(view, this::cancel);
        view.addOnAttachStateChangeListener(new View.OnAttachStateChangeListener() {
            @Override public void onViewAttachedToWindow(View attached) { }

            @Override public void onViewDetachedFromWindow(View detached) {
                if (listening == detached) {
                    listening = null;
                    host = null;
                    hidden.clear();
                    generation++;
                    recognizing = false;
                    if (local != null) local.cancel();
                    if (streaming != null) streaming.cancel();
                    local = null;
                    streaming = null;
                    releasePlatform();
                }
            }
        });
        listening = view;
        int height = BoundsPolicy.atLeast(keyArea.getHeight(),
            s.pixels(KeyboardGeometry.NINE_KEY_HEIGHT_DP));
        ViewGroup.LayoutParams params = keyArea instanceof LinearLayout
            ? LayoutPolicy.matchWidthHeightPx(height)
            : new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, height);
        // 盖在原来的键行上：键行仍占着位置（INVISIBLE），面板用负的上外边距叠上去，键盘高度不跳。
        if (params instanceof LinearLayout.LayoutParams linear && keyArea.getHeight() > 0) {
            linear.topMargin = -keyArea.getHeight();
        }
        keyArea.addView(view, params);
    }

    private void dismiss() {
        if (silenceStop != null) s.main.removeCallbacks(silenceStop);
        silenceStop = null;
        VoiceListeningView view = listening;
        ViewGroup parent = host;
        listening = null;
        host = null;
        for (View child : hidden) ViewPolicy.show(child);
        hidden.clear();
        if (view != null && parent != null && view.getParent() == parent) parent.removeView(view);
    }
}
