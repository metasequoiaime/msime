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
import android.widget.Button;
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
 * 键盘内的语音识别：进程内能跑的识别器（本机 sherpa 模型、豆包流式、HTTP 整句上传）和设备自带的系统识别服务（SpeechRecognizer）都直接在键区里聆听，键区换成聆听面板（「正在聆听…」「说完了／取消」，系统识别服务报音量时圆盘外的光圈随音量涨落），识别完自动上屏并短暂显示「已识别：…」。还没有麦克风权限（要由 Activity 申请）、系统识别服务在键盘里无法启动或录音权限被拒这几种情况仍走 VoiceRecognitionActivity（返回 false，或识别失败后由这里转交）。
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
    /** 听到内容之后这么久没有新的识别文字，就当说完了，结束录音并出结果（与系统识别器停顿后自动结束一致）。经过 VAD 的本机整段模型还判定在说话时推迟，长按空格按住说话时不自动结束，见 {@link VoiceHoldPolicy#silenceElapsed}。 */
    static final long SILENCE_STOP_MILLIS = 1500;

    /** 键盘里接手的识别方式。 */
    enum Engine { LOCAL, STREAMING, UPLOAD, PLATFORM }

    private final MSIMEInputService s;
    private final ExecutorService worker = Executors.newSingleThreadExecutor(runnable -> {
        Thread thread = new Thread(runnable, "msime-keyboard-voice");
        thread.setDaemon(true);
        return thread;
    });
    private Runnable silenceStop;
    private VoiceListeningView listening;
    private View listeningPanel;
    private Button finishButton;
    /** 用户已经结束整句采集后，权限错误不能重新弹窗录音。 */
    private boolean uploadStopped;
    private ViewGroup host;
    private final ArrayList<View> hidden = new ArrayList<>();
    private LocalAsrRecognizer local;
    private DoubaoRecognizer streaming;
    private HttpAsrRecognizer upload;
    private SpeechRecognizer platform;
    private VoicePolisher polisher = new VoicePolisher();
    /** 系统识别服务已经报过 onReadyForSpeech：之后只为权限拒绝转交识别窗口。 */
    private boolean platformListening;
    /** 用户已经按语音键结束了这次系统识别：之后报的错（例如还没开始聆听就被叫停时的 ERROR_CLIENT）不再转交识别窗口，否则按两下语音键会弹出一个识别窗口重新录音。 */
    private boolean platformStopped;
    private long generation;
    private boolean recognizing;
    private boolean diagnosticAllowed;
    /** 这次聆听由长按空格开始、空格还按着：松手才结束，停顿不自动结束。 */
    private boolean held;
    /** 本机整段模型的 VAD 此刻判定在说话（{@link LocalAsrRecognizer.Listener#onSpeech}）。 */
    private boolean speaking;

    ImeVoiceEntry(MSIMEInputService s) {
        this.s = s;
    }

    /**
     * 选键盘里的识别方式：已配置本机模型时用本机；豆包流式在线时用豆包，离线且允许回退、本机模型已装好时用本机；没有配置服务商且设备有系统识别服务时用系统识别服务；配置了上传式服务商时在键区里采集并上传；设备没有可用服务时返回 null 交给 Activity。离线回退对系统识别服务和上传式服务商同样适用。
     */
    static Engine choose(boolean localConfigured, boolean streamingConfigured, boolean online,
                         boolean offlineFallback, boolean fallbackInstalled,
                         boolean uploadConfigured, boolean platformAvailable) {
        if (localConfigured) return Engine.LOCAL;
        boolean fallback = !online && offlineFallback && fallbackInstalled;
        if (streamingConfigured) return fallback ? Engine.LOCAL : Engine.STREAMING;
        if (fallback) return Engine.LOCAL;
        if (uploadConfigured) return Engine.UPLOAD;
        return platformAvailable ? Engine.PLATFORM : null;
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
        diagnosticAllowed = s.imeDebugOverlay.logging();
        recordDiagnostic(switch (engine) {
            case LOCAL -> DiagnosticEventKind.VOICE_ENGINE_LOCAL;
            case STREAMING -> DiagnosticEventKind.VOICE_ENGINE_STREAMING;
            case UPLOAD -> DiagnosticEventKind.VOICE_ENGINE_HTTP;
            case PLATFORM -> DiagnosticEventKind.VOICE_ENGINE_PLATFORM;
        });
        String model = localConfigured ? configured.localModel() : fallbackModel;
        String provider = engine == Engine.LOCAL ? LocalAsrPolicy.PROVIDER : configured.provider();
        String rawLanguage = JsonPolicy.strictString(voice.opt("language"));
        final String language = rawLanguage == null ? "zh-CN" : rawLanguage;
        boolean contribute = s.localSettings.bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO) && s.imePrivacyGate.contributesVoice()
            && !EditorPolicy.password(s.editorInputType);
        // 记下开始聆听时的输入位置；结果出来时位置变了就不直接上屏。
        s.captureVoiceTarget();
        show(keyArea);
        uploadStopped = false;
        long session = ++generation;
        long startedAt = SystemClock.uptimeMillis();
        recognizing = true;
        held = false;
        speaking = false;
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
            local.diagnostics(kind -> diagnostic(session, kind));
            local.retainAudio(contribute);
        } else if (engine == Engine.UPLOAD) {
            upload = new HttpAsrRecognizer();
            listening.setHint("说完点「说完了」，识别后自动上屏");
        } else {
            streaming = new DoubaoRecognizer();
            streaming.retainAudio(contribute);
        }
        LocalAsrRecognizer runningLocal = local;
        DoubaoRecognizer runningStream = streaming;
        HttpAsrRecognizer runningUpload = upload;
        VoicePolisher runningPolisher = polisher;
        String options = HostOptionsPolicy.readRuntimeOptions(files);
        try {
            worker.execute(() -> {
                String text = null;
                String failure = null;
                byte[] pcm = null;
                try {
                    if (runningLocal != null) {
                        // 上面挑引擎时已经把 `files` 为 null 的情况算过了，这里直接解引用会让取不到
                        // files 目录的设备吃一个 NPE，再被下面的兜底改写成那句泛泛的提示；把 null 交给
                        // 识别器当「模型不可用」处理，用户看到的就是对得上的文案。
                        text = runningLocal.recognize(model, files == null ? null : files.toPath(),
                            language, options, null, null, new LocalAsrRecognizer.Listener() {
                                @Override public void onPartial(String partial) {
                                    s.main.post(() -> heard(session, partial));
                                }

                                @Override public void onSpeech(boolean value) {
                                    s.main.post(() -> spoke(session, value));
                                }
                            });
                        pcm = runningLocal.retainedAudio();
                    } else if (runningUpload != null) {
                        text = runningUpload.recognize(requestId, language, configured.requestFormat(),
                            configured.endpoint(), configured.model(), configured.token());
                    } else if (runningStream != null && stream != null) {
                        text = runningStream.recognize(stream.endpoint(), stream.headers(), stream.itn(),
                            stream.punctuation(), stream.ddc(), stream.boostingTableId(),
                            (update, last) -> s.main.post(() -> heard(session, update)));
                        pcm = runningStream.retainedAudio();
                        if (text == null) {
                            diagnostic(session, DiagnosticEventKind.VOICE_FAILURE_RESPONSE);
                            failure = "语音识别服务未响应，请稍后重试";
                        }
                    }
                } catch (HttpAsrRecognizer.Refused refused) {
                    diagnostic(session, switch (refused.failure()) {
                        case PERMISSION -> DiagnosticEventKind.VOICE_FAILURE_PERMISSION;
                        case UNAVAILABLE -> DiagnosticEventKind.VOICE_FAILURE_AUDIO;
                        case NETWORK -> DiagnosticEventKind.VOICE_FAILURE_NETWORK;
                        case EMPTY -> DiagnosticEventKind.VOICE_EMPTY;
                        case CANCELLED -> DiagnosticEventKind.VOICE_CANCELLED;
                    });
                    if (refused.failure() == HttpAsrRecognizer.Failure.PERMISSION) {
                        s.main.post(() -> uploadPermissionFailed(session));
                        return;
                    }
                    failure = switch (refused.failure()) {
                        case PERMISSION -> "语音识别需要麦克风权限";
                        case UNAVAILABLE -> "麦克风被其他应用占用";
                        case NETWORK -> "语音服务未响应，请检查网络与密钥";
                        case EMPTY -> "没有听到内容";
                        case CANCELLED -> null;
                    };
                } catch (LocalAsrRecognizer.Refused refused) {
                    diagnostic(session, refused.diagnosticKind());
                    failure = LocalAsrPolicy.failureMessage(refused.failure(), refused.runtimeMissing());
                } catch (RuntimeException | LinkageError error) {
                    // ZIP 只记固定异常类别，logcat 仅补异常类名，不写消息、路径或凭据。
                    diagnostic(session, VoiceDiagnostics.unexpected(error));
                    ImeLog.w("Voice engine failed: "
                        + (runningLocal != null ? "local" : runningUpload != null ? "http" : "streaming") + " "
                        + error.getClass().getName());
                    failure = "语音识别服务无法启动";
                }
                String result = text == null ? null : TextPolicy.trimmed(text);
                if (result != null && !result.isEmpty() && polish != null) {
                    String polished = runningPolisher.polish(polish.endpoint(), polish.model(), polish.token(),
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
        armSilenceStop(session);
    }

    /** 从现在起计一个停顿时长；到点时按 {@link VoiceHoldPolicy#silenceElapsed} 结束、推迟或等松手。 */
    private void armSilenceStop(long session) {
        if (silenceStop != null) s.main.removeCallbacks(silenceStop);
        Runnable stop = new Runnable() {
            @Override public void run() {
                if (silenceStop != this || session != generation) return;
                switch (VoiceHoldPolicy.silenceElapsed(held, speaking)) {
                    case STOP -> {
                        silenceStop = null;
                        finishListening();
                    }
                    case POSTPONE -> s.main.postDelayed(this, SILENCE_STOP_MILLIS);
                    case WAIT_FOR_RELEASE -> silenceStop = null;
                }
            }
        };
        silenceStop = stop;
        s.main.postDelayed(stop, SILENCE_STOP_MILLIS);
    }

    /** 本机整段模型的 VAD 开始或停止判定在说话。一段说完时，已经在计的停顿改从这一刻重新计，而不是从上一次出字算起。 */
    private void spoke(long session, boolean value) {
        if (session != generation || listening == null) return;
        speaking = value;
        if (!value && silenceStop != null) armSilenceStop(session);
    }

    /** 当前聆听的代次。空格长按开始语音前记下，交给 {@link #hold} 判断这次按压是否真的在键区里开始了一次新的聆听。 */
    long generation() {
        return generation;
    }

    /**
     * 把 {@code before} 之后刚在键区里开始的这次聆听改成按住说话：松手（{@link #releaseHold}）才结束，按住期间停顿不自动结束。按压只是结束了已有的聆听、转交了识别窗口或者没能开始时返回 false。
     */
    boolean hold(long before) {
        if (generation == before || !recognizing || listening == null) return false;
        held = true;
        if (finishButton != null) ViewPolicy.hide(finishButton);
        listening.setHint("松开结束，上滑取消");
        return true;
    }

    /** 按住说话时手指进出取消区：聆听面板切换「松开取消」的样子。 */
    void holdCancelZone(boolean inZone) {
        if (!held || !recognizing || listening == null) return;
        listening.setReleaseToCancel(inZone);
    }

    /** 按住说话松手：{@code cancel} 为 true（在取消区松手，或者触摸被系统取消）时丢掉这次录音，否则结束录音并出结果。这次聆听已经结束或被取消时什么也不做。 */
    void releaseHold(boolean cancel) {
        if (!held) return;
        held = false;
        if (listening != null) listening.setReleaseToCancel(false);
        if (!recognizing) return;
        if (cancel) cancel();
        else finishListening();
    }

    /** 结束录音（保留已录到的部分并出结果）。 */
    private void finishListening() {
        if (!recognizing) {
            dismiss();
            return;
        }
        if (local != null) local.stop();
        if (streaming != null) streaming.stop();
        if (upload != null) {
            uploadStopped = true;
            upload.stop();
        }
        if (finishButton != null) ViewPolicy.setEnabled(finishButton, false);
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
        held = false;
        polisher.cancel();
        // 后台任务保留旧实例，新识别不能复用它永久生效的取消标记。
        polisher = new VoicePolisher();
        if (local != null) local.cancel();
        if (streaming != null) streaming.cancel();
        if (upload != null) upload.cancel();
        local = null;
        streaming = null;
        upload = null;
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
            recordDiagnostic(VoiceDiagnostics.unexpected(error));
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
            recordDiagnostic(VoiceDiagnostics.unexpected(error));
            releasePlatform();
            return false;
        }
        return true;
    }

    /** 启动时不允许记录的请求，稍后切换输入框也不能补记。 */
    private void recordDiagnostic(DiagnosticEventKind kind) {
        if (diagnosticAllowed) s.imeDebugOverlay.recordEvent(kind);
    }

    /** 工作线程与录音线程都回到主线程检查代次和隐私开关。 */
    private void diagnostic(long session, DiagnosticEventKind kind) {
        s.main.post(() -> {
            if (session == generation) recordDiagnostic(kind);
        });
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
        VoicePolisher runningPolisher = polisher;
        try {
            worker.execute(() -> {
                String polished = runningPolisher.polish(polish.endpoint(), polish.model(), polish.token(),
                    polish.prompt(), transcript);
                String finalText = TextPolicy.hasText(polished)
                    ? TextPolicy.trimmed(polished) : transcript;
                s.main.post(() -> delivered(session, finalText, null, null, elapsed, language, PLATFORM_PROVIDER));
            });
        } catch (RejectedExecutionException error) {
            delivered(session, transcript, null, null, elapsed, language, PLATFORM_PROVIDER);
        }
    }

    /** 系统识别服务报错：权限拒绝或聆听前启动失败时转交前台窗口；主动结束后不再重试。 */
    private void platformFailed(long session, int error) {
        if (session != generation) return;
        boolean retry = PlatformSpeechPolicy.retryInActivity(error, platformListening, platformStopped);
        recordDiagnostic(VoiceDiagnostics.platformFailure(error));
        ImeLog.w("Platform speech recognizer failed in keyboard: error " + error);
        cancel();
        if (retry) {
            s.launchVoiceActivity();
            return;
        }
        s.notice(PlatformSpeechPolicy.message(error));
    }

    /** 键区采集被权限拒绝时让前台窗口恢复；旧回调和主动结束不能重新开始录音。 */
    private void uploadPermissionFailed(long session) {
        if (session != generation) return;
        boolean retry = !uploadStopped;
        cancel();
        if (retry) s.launchVoiceActivity();
        else s.notice("语音识别需要麦克风权限");
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
        if (text != null && !text.isEmpty()) recordDiagnostic(DiagnosticEventKind.VOICE_SUCCESS);
        else if (failure == null) recordDiagnostic(DiagnosticEventKind.VOICE_EMPTY);
        recognizing = false;
        if (finishButton != null) ViewPolicy.setEnabled(finishButton, false);
        local = null;
        streaming = null;
        upload = null;
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
        final long lifecycleGeneration = s.inputLifecycleGeneration();
        try {
            worker.execute(() -> {
                if (lifecycleGeneration != s.inputLifecycleGeneration()
                        || !s.imePrivacyGate.contributesVoice()
                        || !AndroidLocalSettings.load(s).bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO)) return;
                try {
                    new VoiceContributionApi(s).upload(contribution);
                } catch (java.io.IOException | RuntimeException error) {
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
        view.setHint("停顿后自动上屏，也可点「说完了」");
        view.setContentDescription("正在聆听，识别结束后自动上屏");
        LinearLayout column = LayoutPolicy.column(s);
        column.addView(view, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        LinearLayout actions = LayoutPolicy.row(s);
        Button done = ViewPolicy.newPressButton(s);
        done.setText("说完了");
        done.setContentDescription("结束聆听并识别，文字自动上屏");
        ViewPolicy.bindClick(done, this::finishListening);
        finishButton = done;
        actions.addView(done, LayoutPolicy.weightedWrapParams(1));
        Button cancel = ViewPolicy.newPressButton(s);
        cancel.setText("取消");
        ViewPolicy.bindClick(cancel, this::cancel);
        actions.addView(cancel, LayoutPolicy.weightedWrapParams(1));
        column.addView(actions, LayoutPolicy.matchWidthWrapParams());
        View panel = column;
        listeningPanel = panel;
        panel.addOnAttachStateChangeListener(new View.OnAttachStateChangeListener() {
            @Override public void onViewAttachedToWindow(View attached) { }

            @Override public void onViewDetachedFromWindow(View detached) {
                if (listeningPanel == detached) {
                    listeningPanel = null;
                    listening = null;
                    host = null;
                    hidden.clear();
                    cancel();
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
        keyArea.addView(panel, params);
    }

    private void dismiss() {
        if (silenceStop != null) s.main.removeCallbacks(silenceStop);
        silenceStop = null;
        View panel = listeningPanel;
        ViewGroup parent = host;
        listeningPanel = null;
        finishButton = null;
        listening = null;
        host = null;
        for (View child : hidden) ViewPolicy.show(child);
        hidden.clear();
        if (panel != null && parent != null && panel.getParent() == parent) parent.removeView(panel);
    }
}
