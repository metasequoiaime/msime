package app.msime.android;

import app.msime.android.LayoutPolicy;

import app.msime.android.policy.HostOptionsPolicy;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import java.io.IOException;
import java.io.InputStream;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.Bundle;
import android.os.SystemClock;
import android.speech.RecognitionListener;
import android.speech.SpeechRecognizer;
import android.view.View;
import android.view.MotionEvent;
import android.widget.PopupWindow;
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
 * 键盘内的语音识别：进程内能跑的识别器（本机 sherpa 模型、豆包流式、HTTP 整句上传）和设备自带的系统识别服务（SpeechRecognizer）都直接在键区里聆听，键区换成聆听面板（「正在聆听…」「说完了／取消」，工具栏人物耳边的红线随麦克风音量变化，长按空格使用独立缩略图与音量短线，边说边识别的引擎把中间文字显示在标题位），识别完自动上屏并短暂显示「已识别：…」。还没有麦克风权限（要由 Activity 申请）、系统识别服务在键盘里无法启动或录音权限被拒这几种情况仍走 VoiceRecognitionActivity（返回 false，或识别失败后由这里转交）。
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
    /** 每次开始本地识别时读取的停顿时间；本次会话中保持固定，长按仍等松手。 */
    private int silenceStopMillis = AndroidLocalSettings.VOICE_PAUSE_DEFAULT_MILLIS;

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
    private Bitmap mascot;
    private View listeningPanel;
    private View coveredActionRow;
    private Button finishButton;
    /** 点按入口的结束按钮行，或长按入口的取消、锁定、发送与编辑控件。 */
    private View actionsRow;
    /** 用户已经结束这次采集（整句上传或豆包流式）后，权限错误不能重新弹窗录音。 */
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
    /** 这次聆听开始时读到的语音标点方式（{@link VoicePunctuation}），结果上屏前按它处理；与 {@link #generation} 同时更新。 */
    private String punctuationMode = VoicePunctuation.AUTO;
    private boolean recognizing;
    private boolean diagnosticAllowed;
    /** 这次聆听由长按空格开始、空格还按着：松手才结束，停顿不自动结束。 */
    private boolean held;
    /** 正在为长按空格开始一次聆听：本地识别在创建会话时就要知道，`held` 要等 `hold` 才设置，那时工作线程可能已经读过判停灵敏度。 */
    private boolean startingHeld;
    /** 松手后继续聆听；只有显式发送会重新授权当前编辑位置。 */
    private boolean locked;
    private Button holdCancelButton;
    private Button holdLockButton;
    private Button holdSendButton;
    private Button holdSpaceButton;
    private View holdUtilities;
    private PopupWindow editPopup;
    /** 服务自行结束后的结果等用户松手、发送或取消，不提前上屏。 */
    private Runnable pendingDelivery;
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
        punctuationMode = configured.punctuationMode();
        long startedAt = SystemClock.uptimeMillis();
        recognizing = true;
        held = false;
        loadMascot(session, listening);
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
            AndroidLocalSettings.Snapshot localPreferences = AndroidLocalSettings.load(s);
            silenceStopMillis = localPreferences.integer(AndroidLocalSettings.VOICE_PAUSE_MILLIS);
            // 判停灵敏度只给点按开始、停顿后自动结束的会话；按住空格由松手结束，传 0 让阈值保持 0.5，与鸿蒙和 iOS 一致。
            local.vadSensitivity(startingHeld ? 0 : localPreferences.integer(AndroidLocalSettings.VOICE_VAD_SENSITIVITY));
            local.diagnostics(kind -> diagnostic(session, kind));
            local.retainAudio(contribute);
        } else if (engine == Engine.UPLOAD) {
            upload = new HttpAsrRecognizer();
            upload.levels(level -> s.main.post(() -> leveled(session, level)));
            listening.setHint("说完后点「说完了」，识别后自动上屏");
        } else {
            streaming = new DoubaoRecognizer();
            streaming.retainAudio(contribute);
            streaming.levels(level -> s.main.post(() -> leveled(session, level)));
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
                // 文字照常上屏、另外要告诉用户的一句：录音到了上限，或者豆包识别到一半连接断了。
                String notice = null;
                byte[] pcm = null;
                try {
                    if (runningLocal != null) {
                        // 上面挑引擎时已经把 `files` 为 null 的情况算过了，这里直接解引用会让取不到
                        // files 目录的设备吃一个 NPE，再被下面的兜底改写成那句泛泛的提示；把 null 交给
                        // 识别器当「模型不可用」处理，用户看到的就是对得上的文案。
                        text = runningLocal.recognize(model, files == null ? null : files.toPath(),
                            language, options, null, null, new LocalAsrRecognizer.Listener() {
                                @Override public void onPartial(String partial) {
                                    s.main.post(() -> heard(session, partial, true));
                                }

                                @Override public void onSpeech(boolean value) {
                                    s.main.post(() -> spoke(session, value));
                                }

                                @Override public void onVolume(float level) {
                                    s.main.post(() -> leveled(session, level));
                                }
                            });
                        pcm = runningLocal.retainedAudio();
                    } else if (runningUpload != null) {
                        text = runningUpload.recognize(requestId, language, configured.requestFormat(),
                            configured.endpoint(), configured.model(), configured.token());
                        if (runningUpload.capped()) notice = VoiceFailurePolicy.capped(HttpAsrRecognizer.MAX_MILLIS);
                    } else if (runningStream != null && stream != null) {
                        text = runningStream.recognize(stream.endpoint(), stream.headers(), stream.itn(),
                            stream.punctuation(), stream.ddc(), stream.boostingTableId(),
                            (update, last) -> s.main.post(() -> heard(session, update, false)));
                        pcm = runningStream.retainedAudio();
                        HttpAsrRecognizer.Failure interrupted = runningStream.interruption();
                        if (interrupted != null) {
                            diagnostic(session, VoiceFailurePolicy.diagnostic(interrupted));
                            notice = VoiceFailurePolicy.interrupted(interrupted);
                        } else if (runningStream.capped()) {
                            notice = VoiceFailurePolicy.capped(DoubaoRecognizer.MAX_MILLIS);
                        }
                    }
                } catch (HttpAsrRecognizer.Refused refused) {
                    diagnostic(session, VoiceFailurePolicy.diagnostic(refused.failure()));
                    if (refused.failure() == HttpAsrRecognizer.Failure.PERMISSION) {
                        s.main.post(() -> uploadPermissionFailed(session));
                        return;
                    }
                    failure = VoiceFailurePolicy.message(refused.failure());
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
                String finalNotice = notice;
                byte[] audio = pcm;
                long elapsed = SystemClock.uptimeMillis() - startedAt;
                s.main.post(() -> delivered(session, finalText, finalFailure, finalNotice, audio, elapsed,
                    language, provider));
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

    /**
     * 识别中途的文字：聆听中显示在聆听面板的标题位；{@code pauseEnds} 为 true 时，从这一刻起按本次的 {@link #silenceStopMillis} 计停顿。
     *
     * <p>只有本机模型这样结束：它的 VAD 会报「还在说话」（{@link #spoke}），停顿计时据此推迟。系统识别服务自己判断说完了没有，这里再按停顿掐断会切掉还没说完的话。豆包流式也不按停顿结束：它不报是否在说话，整句流式（{@code bigmodel_nostream}）说话期间要到音频超过 15 秒才回第一段结果，按出字后停顿计时，口述到 16 秒多就会在用户还在说时被掐断（#9886）；豆包由用户点「说完了」或松开空格结束，和以前一样。
     */
    private void heard(long session, String partial, boolean pauseEnds) {
        if (session != generation || listening == null || partial == null || partial.isEmpty()) return;
        listening.setPartial(partial);
        if (!pauseEnds) return;
        armSilenceStop(session);
    }

    /** 采集线程报来的麦克风音量（0–1）：聆听面板的光圈随它涨落。 */
    private void leveled(long session, float level) {
        if (session != generation || listening == null || !recognizing) return;
        listening.setLevel(level);
    }

    /** 从现在起计一个停顿时长；到点时按 {@link VoiceHoldPolicy#silenceElapsed} 结束、推迟或等松手。 */
    private void armSilenceStop(long session) {
        if (silenceStop != null) s.main.removeCallbacks(silenceStop);
        Runnable stop = new Runnable() {
            @Override public void run() {
                if (silenceStop != this || session != generation) return;
                switch (VoiceHoldPolicy.silenceElapsed(held || locked, speaking)) {
                    case STOP -> {
                        silenceStop = null;
                        finishListening();
                    }
                    case POSTPONE -> s.main.postDelayed(this, silenceStopMillis);
                    case WAIT_FOR_RELEASE -> silenceStop = null;
                }
            }
        };
        silenceStop = stop;
        s.main.postDelayed(stop, silenceStopMillis);
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

    /** 空格键在 {@code before} 之后开始的这次聆听（含松手时才补判出的长按）不显示工具栏耳部信号。按压只是结束了已有的聆听或转交了识别窗口时什么也不做。 */
    void plainIndicator(long before) {
        if (generation == before || listening == null) return;
        listening.setMascotEnabled(false);
    }

    /** 长按空格开始的按住说话：本次本地识别不用判停灵敏度，阈值保持 0.5（松手结束，调高阈值只会丢轻声的字）。 */
    void startHeld(Runnable start) {
        startingHeld = true;
        try {
            start.run();
        } finally {
            startingHeld = false;
        }
    }

    /** 把这次按压刚开始的聆听切为长按面板，触摸仍由原空格接收。 */
    boolean hold(long before) {
        if (generation == before || !recognizing || listening == null) return false;
        held = true;
        listening.setMascotEnabled(false);
        locked = false;
        coverActionRow();
        buildHoldActions();
        listening.setHoldMode(s.imeStickers == null ? null : s.imeStickers.tabIcon());
        listening.setHint("滑到锁定后松手可继续说，滑到取消后松手放弃");
        return true;
    }

    /** 长按面板覆盖字母区与原功能行；原行保持挂载，继续接收已开始的触摸流。 */
    private void coverActionRow() {
        View panel = listeningPanel;
        if (panel == null || host == null || s.actionRow == null || !ViewPolicy.isVisible(s.actionRow)
                || !(host.getParent() instanceof LinearLayout parent)
                || s.actionRow.getParent() != parent) return;
        int height = s.actionRow.getBottom() - host.getTop();
        if (height <= 0) return;
        // 搬到共同父容器时暂时解除卸载监听的身份，避免把内部重挂载当作取消。
        listeningPanel = null;
        host.removeView(panel);
        coveredActionRow = s.actionRow;
        hidden.add(coveredActionRow);
        ViewPolicy.setInvisible(coveredActionRow);
        host = parent;
        listeningPanel = panel;
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthHeightPx(height);
        params.topMargin = -height;
        parent.addView(panel, params);
    }

    boolean coversActionRow() { return coveredActionRow != null; }

    private static VoiceHoldPolicy.Region region(View view) {
        if (view == null || !view.isShown() || view.getWidth() <= 0 || view.getHeight() <= 0) return null;
        int[] location = new int[2];
        view.getLocationOnScreen(location);
        return new VoiceHoldPolicy.Region(location[0], location[1],
            location[0] + view.getWidth(), location[1] + view.getHeight());
    }

    /** 实际按钮命中与高亮使用同一份范围，滑出立即取消该按钮的高亮。 */
    void moveHold(float x, float y) {
        if (!held || listening == null) return;
        VoiceHoldPolicy.Region cancel = region(holdCancelButton), lock = region(holdLockButton);
        if (holdCancelButton != null) holdCancelButton.setPressed(cancel != null && cancel.contains(x, y));
        if (holdLockButton != null) holdLockButton.setPressed(lock != null && lock.contains(x, y));
        VoiceHoldPolicy.Region send = region(holdSendButton);
        if (holdSendButton != null) holdSendButton.setPressed(send != null && send.contains(x, y));
        listening.setReleaseToCancel(cancel != null && cancel.contains(x, y));
    }

    /** 松手重新命中最终坐标；系统取消无条件丢弃，锁定不停止识别器。 */
    void releaseHold(float x, float y, boolean interrupted) {
        if (!held) return;
        VoiceHoldPolicy.Action action = VoiceHoldPolicy.release(x, y,
            region(holdCancelButton), region(holdLockButton), interrupted);
        held = false;
        for (Button button : new Button[] {holdCancelButton, holdLockButton, holdSendButton})
            if (button != null) button.setPressed(false);
        if (listening != null) listening.setReleaseToCancel(false);
        if (action == VoiceHoldPolicy.Action.CANCEL) cancel();
        else if (action == VoiceHoldPolicy.Action.LOCK) {
            locked = true;
            if (holdSpaceButton != null) {
                holdSpaceButton.setText("已锁定");
                holdSpaceButton.setContentDescription("录音已锁定，点击发送或取消结束");
                ViewPolicy.setEnabled(holdSpaceButton, false);
            }
            if (holdLockButton != null) {
                holdLockButton.setText("🔒");
                ViewPolicy.setEnabled(holdLockButton, false);
            }
            if (holdUtilities != null) enableHoldUtilities(true);
            if (listening != null) listening.setHint(pendingDelivery == null
                ? "已锁定，继续录音；可编辑文字，点发送结束" : "录音已结束，点发送或取消");
        } else finishListening();
    }

    private Button holdButton(String title, String description, Runnable action) {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        button.setText(title);
        button.setContentDescription(description);
        button.setKeyboardRole(KeyboardKeyRole.KEY);
        s.imeStyler.styleButton(button, KeyboardKeyRole.KEY, s.skin);
        ViewPolicy.clearMinimumWidth(button);
        ViewPolicy.setMinimumHeight(button, s.pixels(44));
        button.setSingleLine(true);
        button.setPadding(0, 0, 0, 0);
        KeyboardTypographyPolicy.setKeyTextSize(button, 13);
        ViewPolicy.bindClick(button, action);
        return button;
    }

    /** 控制行（🔓 | 松开发送 | 发送）排在最下面、与原功能行同高，压着原空格的手指落在「松开发送」上，水平左滑到锁定、右滑到发送（#10327）。 */
    private void buildHoldActions() {
        if (!(listeningPanel instanceof LinearLayout column)) return;
        if (actionsRow != null) column.removeView(actionsRow);
        LinearLayout actions = LayoutPolicy.column(s);
        LinearLayout cancelRow = LayoutPolicy.row(s);
        cancelRow.addView(new View(s), new LinearLayout.LayoutParams(0, 1, 1));
        holdCancelButton = holdButton("取消", "取消本次语音", this::cancel);
        cancelRow.addView(holdCancelButton, new LinearLayout.LayoutParams(0, s.pixels(44), 2));
        cancelRow.addView(new View(s), new LinearLayout.LayoutParams(0, 1, 1));
        actions.addView(cancelRow, LayoutPolicy.matchWidthWrapParams());
        LinearLayout utilities = LayoutPolicy.row(s);
        utilities.addView(holdButton("符", "锁定语音中打开符号", () -> {
            if (locked) s.imePanels.showSymbolPanel();
        }), new LinearLayout.LayoutParams(0, s.pixels(44), 1));
        utilities.addView(holdPunctuation("！", "？"), new LinearLayout.LayoutParams(0, s.pixels(44), 1));
        utilities.addView(new View(s), new LinearLayout.LayoutParams(0, 1, 1));
        utilities.addView(holdPunctuation("，", "。"), new LinearLayout.LayoutParams(0, s.pixels(44), 1));
        Button other = holdButton("其他", "锁定语音编辑操作", this::toggleEditPopup);
        utilities.addView(other, new LinearLayout.LayoutParams(0, s.pixels(44), 1));
        holdUtilities = utilities;
        enableHoldUtilities(false);
        actions.addView(utilities, LayoutPolicy.matchWidthWrapParams());
        int rowHeight = coveredActionRow != null && coveredActionRow.getHeight() > 0
            ? coveredActionRow.getHeight() : s.pixels(KeyboardGeometry.KEY_ROW_HEIGHT_DP);
        LinearLayout controls = LayoutPolicy.row(s);
        holdLockButton = holdButton("🔓", "滑到此处松手锁定录音", () -> {});
        controls.addView(holdLockButton, new LinearLayout.LayoutParams(0, rowHeight, 1));
        holdSpaceButton = holdButton("松开发送", "长按模式，松开发送", () -> {});
        controls.addView(holdSpaceButton, new LinearLayout.LayoutParams(0, rowHeight, 3));
        holdSendButton = holdButton("发送", "发送本次语音", this::finishListening);
        finishButton = holdSendButton;
        controls.addView(holdSendButton, new LinearLayout.LayoutParams(0, rowHeight, 1));
        actions.addView(controls, LayoutPolicy.matchWidthWrapParams());
        column.addView(actions, LayoutPolicy.matchWidthWrapParams());
        actionsRow = actions;
    }

    private void enableHoldUtilities(boolean enabled) {
        if (holdUtilities instanceof ViewGroup group)
            for (int index = 0; index < group.getChildCount(); index++)
                ViewPolicy.setEnabled(group.getChildAt(index), enabled);
    }

    /** 标点只在锁定后接受点按或上滑，一次触摸只插入一个符号。 */
    private Button holdPunctuation(String tap, String swipe) {
        Button button = holdButton(tap, "锁定语音：点按" + tap + "，上滑" + swipe, () -> {
            if (locked) s.commitText(tap, TypingSource.LOCAL);
        });
        float[] down = new float[1];
        button.setOnTouchListener((view, event) -> {
            if (!locked) return true;
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> { down[0] = event.getY(); button.setPressed(true); }
                case MotionEvent.ACTION_UP -> {
                    button.setPressed(false);
                    if (down[0] - event.getY() >= s.pixels(16)) s.commitText(swipe, TypingSource.LOCAL);
                    else button.performClick();
                }
                case MotionEvent.ACTION_CANCEL -> button.setPressed(false);
                default -> { }
            }
            return true;
        });
        return button;
    }

    /** 编辑弹层位于「其他」上方，重复点击关闭；不抢编辑器焦点。 */
    private void toggleEditPopup() {
        if (editPopup != null) { editPopup.dismiss(); editPopup = null; return; }
        if (!locked || !(holdUtilities instanceof ViewGroup group)) return;
        View anchor = group.getChildAt(group.getChildCount() - 1);
        LinearLayout content = LayoutPolicy.row(s);
        String[] labels = {"换行", "空格", "删除"};
        for (int index = 0; index < labels.length; index++) {
            int action = index;
            content.addView(holdButton(labels[index], "锁定语音" + labels[index], () -> {
                if (!locked) return;
                if (action == 2) s.deleteCodePointBeforeCursor();
                else s.commitText(action == 0 ? "\n" : " ", TypingSource.LOCAL);
            }), new LinearLayout.LayoutParams(s.pixels(56), s.pixels(44)));
        }
        s.imeStyler.applySkinBackground(content);
        PopupWindow popup = new PopupWindow(content, s.pixels(168), s.pixels(44), false);
        popup.setOutsideTouchable(false);
        editPopup = popup;
        popup.setOnDismissListener(() -> { if (editPopup == popup) editPopup = null; });
        // IME 子窗口的屏幕原点不为零，按锚点偏移定位，避免重复叠加窗口坐标。
        popup.showAsDropDown(anchor, anchor.getWidth() - s.pixels(168),
            -anchor.getHeight() - s.pixels(44));
        ScreenCaptureProtection.apply(popup);
    }

    /** 结束录音（保留已录到的部分并出结果）。 */
    private void finishListening() {
        // 锁定允许在同一输入框编辑，点发送才授权此刻的位置；之后的迟到结果仍检查快照。
        if (locked && s.connection != null) s.captureVoiceTarget();
        locked = false;
        held = false;
        if (editPopup != null) editPopup.dismiss();
        if (pendingDelivery != null) {
            Runnable delivery = pendingDelivery;
            pendingDelivery = null;
            delivery.run();
            return;
        }
        if (holdUtilities != null) enableHoldUtilities(false);
        if (!recognizing) {
            dismiss();
            return;
        }
        if (local != null) local.stop();
        if (streaming != null) {
            uploadStopped = true;
            streaming.stop();
        }
        if (upload != null) {
            uploadStopped = true;
            upload.stop();
        }
        if (finishButton != null) ViewPolicy.setEnabled(finishButton, false);
        if (platform != null) {
            platformStopped = true;
            platform.stopListening();
        }
        if (listening != null) listening.showRecognizing("正在识别…");
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
        locked = false;
        pendingDelivery = null;
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
                    // 说完后多数识别服务不再报音量，光圈收回到安静时的样子，不能停在半截。
                    listening.resetLevel();
                    listening.showRecognizing("正在识别…");
                }
            }
            @Override public void onError(int error) {
                platformFailed(session, error);
            }
            @Override public void onResults(Bundle results) {
                platformRecognized(session, startedAt, firstResult(results), language, polish);
            }
            @Override public void onPartialResults(Bundle partialResults) {
                heard(session, firstResult(partialResults), false);
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
            delivered(session, null, PlatformSpeechPolicy.emptyResult(), null, null, 0, language, PLATFORM_PROVIDER);
            return;
        }
        long elapsed = SystemClock.uptimeMillis() - startedAt;
        if (polish == null) {
            delivered(session, result, null, null, null, elapsed, language, PLATFORM_PROVIDER);
            return;
        }
        if (listening != null) listening.showRecognizing("正在润色…");
        String transcript = result;
        VoicePolisher runningPolisher = polisher;
        try {
            worker.execute(() -> {
                String polished = runningPolisher.polish(polish.endpoint(), polish.model(), polish.token(),
                    polish.prompt(), transcript);
                String finalText = TextPolicy.hasText(polished)
                    ? TextPolicy.trimmed(polished) : transcript;
                s.main.post(() -> delivered(session, finalText, null, null, null, elapsed, language, PLATFORM_PROVIDER));
            });
        } catch (RejectedExecutionException error) {
            delivered(session, transcript, null, null, null, elapsed, language, PLATFORM_PROVIDER);
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

    /** 识别结束：有文字就上屏，没有就提示 {@code failure}；{@code notice} 是文字上屏之后另外要说的一句（录音到了上限、识别途中断开）。四种识别服务和润色的结果都经过这里，语音标点在这里处理一次，锁定等待发送时显示的、存进语音结果的也是处理后的文字；只剩标点时按没有识别到文字处理。 */
    private void delivered(long session, String recognized, String failure, String notice, byte[] pcm,
                           long elapsedMillis, String language, String provider) {
        if (session != generation) return;
        String text = VoicePunctuation.apply(recognized, punctuationMode);
        if (text != null && !text.isEmpty()) recordDiagnostic(DiagnosticEventKind.VOICE_SUCCESS);
        else if (failure == null) recordDiagnostic(DiagnosticEventKind.VOICE_EMPTY);
        recognizing = false;
        if (finishButton != null) ViewPolicy.setEnabled(finishButton, false);
        local = null;
        streaming = null;
        upload = null;
        releasePlatform();
        if (held || locked) {
            pendingDelivery = () -> applyDelivery(session, text, failure, notice, pcm,
                elapsedMillis, language, provider);
            if (finishButton != null) ViewPolicy.setEnabled(finishButton, true);
            if (listening != null) {
                listening.showWaitingForSend(text);
                listening.setHint("录音已结束，点发送或取消");
                listening.setContentDescription("录音已结束，等待发送或取消");
            }
            return;
        }
        applyDelivery(session, text, failure, notice, pcm, elapsedMillis, language, provider);
    }

    /** 结果投递只经过这一处；锁定等待与普通录音保留同一份上下文保护。 */
    private void applyDelivery(long session, String text, String failure, String notice, byte[] pcm,
                              long elapsedMillis, String language, String provider) {
        if (session != generation) return;
        if (text == null || text.isEmpty()) {
            if (failure != null) s.notice(failure);
            dismiss();
            return;
        }
        // 聆听可能长达一分钟（豆包流式可到五分钟）：这期间用户点到了别处、应用改写了输入框，或者开始了组字，结果就不能盲目插在现在的光标处。与识别窗口那条路一样，先存进语音结果，由用户在结果面板里显式插入。
        if (!s.voiceInsertionReady() || !s.voiceTargetMatches()) {
            boolean kept = s.stashVoiceResult(text);
            // 保留下来的文字可能不完整（到了上限或识别途中断开），这句说明不能丢；提示只显示最后一句，所以并成一句，语音结果面板随后打开。
            s.notice(!kept ? "输入位置已变化；结果已安全清除"
                : notice != null ? notice + "，已存入语音结果"
                : "输入位置已变化，结果已保留，可在语音结果中插入");
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
        s.notice(notice);
        if (holdSpaceButton != null) {
            dismiss();
            return;
        }
        if (listening != null) {
            listening.showDone(text);
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

    /** 只在后台解码内置贴纸；结果总是缓存给后续会话，只有发起解码的会话与面板仍在时才更新面板，取消后的旧回调不能更新新面板。 */
    private void loadMascot(long session, VoiceListeningView view) {
        if (mascot != null) return;
        try {
            worker.execute(() -> {
                try (InputStream input = s.getAssets().open(StickerPackModel.ASSET_DIRECTORY + "/01.webp")) {
                    Bitmap bitmap = BitmapFactory.decodeStream(input);
                    s.main.post(() -> {
                        if (mascot == null) mascot = bitmap;
                        if (generation != session || listening != view) return;
                        view.setMascot(mascot);
                    });
                } catch (IOException error) {
                    // 素材异常不触发下载，不影响识别、结束和取消控件。
                }
            });
        } catch (RejectedExecutionException ignored) {
            // 会话正在关闭，素材任务不再启动。
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
        view.setMascotEnabled(true);
        view.setMascot(mascot);
        // 跟随应用主题的皮肤要先按当季种子推导（样式通道画键帽用的也是这一份），否则圆盘取到的是基础色，和回车键等强调色对不上。
        KeyboardSkin skin = s.imeStyler.themed(s.skin);
        view.setColors(Color.parseColor(skin.accent()), Color.parseColor(skin.onAccent()),
            Color.parseColor(skin.keyForeground()), Color.parseColor(skin.toolbarIcon()));
        view.setHint("停顿后自动上屏，也可点「说完了」");
        view.setContentDescription("正在聆听，识别结束后自动上屏");
        LinearLayout column = LayoutPolicy.column(s);
        column.addView(view, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        // 按钮行左右各让 16 dp，和下面的键行隔 10 dp，两个按钮之间 12 dp；长按入口替换为专用操作区（见 hold）。按钮直接按皮肤上色（说完了用强调色、取消用普通键面），不等下一次整树样式通道。
        LinearLayout actions = LayoutPolicy.row(s);
        KeyboardPressButton done = ViewPolicy.newPressButton(s);
        done.setText("说完了");
        done.setContentDescription("结束聆听并识别，文字自动上屏");
        done.setKeyboardRole(KeyboardKeyRole.ACCENT);
        s.imeStyler.styleButton(done, KeyboardKeyRole.ACCENT, s.skin);
        ViewPolicy.bindClick(done, this::finishListening);
        finishButton = done;
        LinearLayout.LayoutParams doneParams = LayoutPolicy.weightedWrapParams(1);
        doneParams.rightMargin = s.pixels(12);
        actions.addView(done, doneParams);
        KeyboardPressButton cancel = ViewPolicy.newPressButton(s);
        cancel.setText("取消");
        cancel.setKeyboardRole(KeyboardKeyRole.KEY);
        s.imeStyler.styleButton(cancel, KeyboardKeyRole.KEY, s.skin);
        ViewPolicy.bindClick(cancel, this::cancel);
        actions.addView(cancel, LayoutPolicy.weightedWrapParams(1));
        LinearLayout.LayoutParams actionsParams = LayoutPolicy.matchWidthWrapParams();
        actionsParams.setMargins(s.pixels(16), 0, s.pixels(16), s.pixels(10));
        column.addView(actions, actionsParams);
        actionsRow = actions;
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
        actionsRow = null;
        holdCancelButton = holdLockButton = holdSendButton = holdSpaceButton = null;
        holdUtilities = null;
        coveredActionRow = null;
        if (editPopup != null) editPopup.dismiss();
        editPopup = null;
        pendingDelivery = null;
        locked = held = false;
        listening = null;
        host = null;
        for (View child : hidden) ViewPolicy.show(child);
        hidden.clear();
        if (panel != null && parent != null && panel.getParent() == parent) parent.removeView(panel);
    }
}
