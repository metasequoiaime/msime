package app.msime.android;

import app.msime.android.policy.HostOptionsPolicy;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.SystemClock;
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
 * 键盘内的语音识别：进程内能跑的识别器（本机 sherpa 模型、豆包流式）直接在键区里聆听，键区换成 {@link VoiceListeningView}（「正在聆听…」「点任意处取消」），识别完自动上屏并短暂显示「已识别：…」。系统识别器与 OpenAI 兼容的上传仍走原来的 VoiceRecognitionActivity（返回 false）。
 *
 * <p>`voice_input.offline_fallback` 打开、没有网络且 `voice_input.asr_model_path` 指向已安装的本机模型时，改用本机识别。没有麦克风权限时交回 Activity 去申请。识别结束且隐私判断允许时记语音时长（`record_voice`）；`voice_input.contribute_audio` 打开、隐私判断允许且不是密码框时，经 {@link VoiceContributionApi} 上传这次的音频与识别文本。
 */
final class ImeVoiceEntry {
    /** 「已识别：…」停留的时间。 */
    static final long DONE_NOTICE_MILLIS = 1200;
    /** 听到内容之后这么久没有新的识别文字，就当说完了，结束录音并出结果（与系统识别器停顿后自动结束一致）。 */
    static final long SILENCE_STOP_MILLIS = 1500;

    /** 键盘里接手的识别方式。 */
    enum Engine { LOCAL, STREAMING }

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
    private long generation;
    private boolean recognizing;

    ImeVoiceEntry(MSIMEInputService s) {
        this.s = s;
    }

    /**
     * 选键盘里的识别方式：已配置本机模型时用本机；豆包流式在线时用豆包，离线且允许回退、本机模型已装好时用本机；其他（系统识别器、上传式识别）返回 null 交给 Activity，离线回退同样适用于它们。
     */
    static Engine choose(boolean localConfigured, boolean streamingConfigured, boolean online,
                         boolean offlineFallback, boolean fallbackInstalled) {
        if (localConfigured) return Engine.LOCAL;
        boolean fallback = !online && offlineFallback && fallbackInstalled;
        if (streamingConfigured) return fallback ? Engine.LOCAL : Engine.STREAMING;
        return fallback ? Engine.LOCAL : null;
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
        String fallbackModel = voice.optString("asr_model_path", "");
        boolean fallbackInstalled = files != null && LocalAsrPolicy.usable(LocalAsrPolicy.PROVIDER, fallbackModel)
            && LocalAsrPolicy.installed(fallbackModel, files.toPath());
        boolean localConfigured = configured.localModel() != null;
        boolean streamingConfigured = configured.streaming() != null;
        boolean needsNetwork = !localConfigured;
        Engine engine = choose(localConfigured, streamingConfigured, !needsNetwork || online(),
            s.localSettings.bool(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK), fallbackInstalled);
        if (engine == null) return false;
        String model = localConfigured ? configured.localModel() : fallbackModel;
        String provider = engine == Engine.LOCAL ? LocalAsrPolicy.PROVIDER : DoubaoAsrPolicy.PROVIDER;
        String language = voice.optString("language", "zh-CN");
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
                    failure = switch (refused.failure()) {
                        case PERMISSION -> "语音识别需要麦克风权限";
                        case UNAVAILABLE -> "麦克风被其他应用占用";
                        case MODEL -> "本地语音模型未安装或已损坏，请在设置中重新下载";
                        case RUNTIME -> "本地语音识别组件无法加载";
                        case EMPTY -> "没有听到内容";
                        case CANCELLED -> null;
                    };
                } catch (RuntimeException | LinkageError error) {
                    failure = "语音识别服务无法启动";
                }
                String result = text == null ? null : text.trim();
                if (result != null && !result.isEmpty() && polish != null) {
                    String polished = new VoicePolisher().polish(polish.endpoint(), polish.model(), polish.token(),
                        polish.prompt(), result);
                    if (polished != null && !polished.trim().isEmpty()) result = polished.trim();
                }
                String finalText = result;
                String finalFailure = failure;
                byte[] audio = pcm;
                long elapsed = SystemClock.uptimeMillis() - startedAt;
                s.main.post(() -> delivered(session, finalText, finalFailure, audio, elapsed, language, provider));
            });
        } catch (RejectedExecutionException error) {
            dismiss();
            return false;
        }
        return true;
    }

    /** 识别中途的文字：显示在提示行，并在停顿后自动结束录音。 */
    private void heard(long session, String partial) {
        if (session != generation || listening == null || partial == null || partial.isEmpty()) return;
        listening.setHint(partial);
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
        dismiss();
    }

    private void delivered(long session, String text, String failure, byte[] pcm, long elapsedMillis,
                           String language, String provider) {
        if (session != generation) return;
        recognizing = false;
        local = null;
        streaming = null;
        if (text == null || text.isEmpty()) {
            if (failure != null) android.widget.Toast.makeText(s, failure, android.widget.Toast.LENGTH_SHORT).show();
            dismiss();
            return;
        }
        // 聆听可能长达一分钟：这期间用户点到了别处、应用改写了输入框，或者开始了组字，结果就不能盲目插在现在的光标处。与识别窗口那条路一样，先存进语音结果，由用户在结果面板里显式插入。
        if (!s.voiceInsertionReady() || !s.voiceTargetMatches()) {
            boolean kept = s.stashVoiceResult(text);
            android.widget.Toast.makeText(s, kept ? "输入位置已变化，结果已保留，可在语音结果中插入"
                : "输入位置已变化；结果已安全清除", android.widget.Toast.LENGTH_SHORT).show();
            dismiss();
            if (kept) s.showVoiceResult();
            return;
        }
        boolean committed = s.commitText(text, TypingSource.VOICE);
        if (!committed) {
            android.widget.Toast.makeText(s, "编辑器拒绝插入；结果已安全清除", android.widget.Toast.LENGTH_SHORT).show();
            dismiss();
            return;
        }
        if (s.imePrivacyGate.recordsVoice()) recordVoice(elapsedMillis);
        if (pcm != null && pcm.length > 0 && s.imePrivacyGate.contributesVoice()
                && !EditorPolicy.password(s.editorInputType)) {
            contribute(pcm, text, language, provider);
        }
        if (listening != null) {
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
        return files == null ? "" : new File(files, "bootstrap/state").getAbsolutePath();
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
            if (child.getVisibility() == View.VISIBLE) {
                hidden.add(child);
                child.setVisibility(View.INVISIBLE);
            }
        }
        VoiceListeningView view = new VoiceListeningView(s);
        view.setColors(Color.parseColor(s.skin.accent()), Color.parseColor(s.skin.onAccent()),
            Color.parseColor(s.skin.keyForeground()), Color.parseColor(s.skin.toolbarIcon()));
        view.setOnClickListener(ignored -> cancel());
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
                }
            }
        });
        listening = view;
        int height = BoundsPolicy.atLeast(keyArea.getHeight(),
            s.pixels(KeyboardGeometry.NINE_KEY_HEIGHT_DP));
        ViewGroup.LayoutParams params = keyArea instanceof LinearLayout
            ? KeyboardGeometry.matchWidthHeightPx(height)
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
