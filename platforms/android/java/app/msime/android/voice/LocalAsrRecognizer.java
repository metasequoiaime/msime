package app.msime.android;

import android.content.ComponentCallbacks2;
import android.content.Context;
import android.content.res.Configuration;
import android.media.AudioRecord;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.Executors;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * On-device recognition with an installed sherpa-onnx model: the audio never leaves the device.
 *
 * <p>The recognizer is the shared one every desktop host uses (`shared/voice/LocalAsr`), compiled into this host's JNI library; the runtime is the pinned sherpa-onnx `.aar`'s native libraries, packaged beside it or, where the package leaves them out, taken from the downloaded voice-runtime resource pack. This class owns the microphone and the two threads between it and that recognizer: capture reads the microphone into a queue so a slow decode never drops audio, and the calling worker loads the model, decodes, reports partial text and finishes.
 *
 * <p>Hotwords come from the user's own pinyin dictionary through the shared host API. A model that biases natively gets them at session start; one whose manifest says `"hotwords": "pinyin"` gets the final transcript corrected by the shared pinyin matcher instead.
 *
 * <p>最终文本最后经共享层 `msime_client_voice_normalize_numbers` 按清单的 `kind` 把汉字数字改成数字（只改写不自带 ITN 的 transducer 模型），中间结果不改。
 */
public final class LocalAsrRecognizer {
    /** Roughly 100 ms of 16 kHz mono audio per read. */
    private static final int CHUNK_SAMPLES = WavAudio.SAMPLE_RATE / 10;
    private static final Object MEMORY_LOCK = new Object();
    private static ScheduledExecutorService releaser;
    private static boolean watchingMemory;

    /** 中间文字和说话状态在识别线程回传，音量在采集线程回传；界面更新由调用方切回主线程。 */
    public interface Listener {
        void onPartial(String text);

        /** 经过 VAD 的整段模型（SenseVoice、离线 X-ASR 等）开始或停止判定在说话时调用一次。这类模型一段说完才出文字，调用方按「多久没有新文字」判断说完时，要在说话期间推迟，否则用户接着说下去会被截断。流式模型不报。 */
        default void onSpeech(boolean speaking) { }

        /** 每次实际读取 PCM 后回传 0–1 音量，模型冷加载期间也能显示聆听反馈。 */
        default void onVolume(float level) { }
    }

    /** Why a recognition did not produce text. The caller maps these onto its own messages. */
    public enum Failure { PERMISSION, UNAVAILABLE, MODEL, RUNTIME, CANCELLED, EMPTY }

    /** Thrown rather than returning null so a caller cannot mistake a failure for silence. */
    public static final class Refused extends Exception {
        private static final long serialVersionUID = 1L;
        private final Failure failure;
        private final boolean runtimeMissing;

        Refused(Failure failure) {
            this(failure, false);
        }

        private Refused(Failure failure, boolean runtimeMissing) {
            super(failure.name());
            this.failure = failure;
            this.runtimeMissing = runtimeMissing;
        }

        /** 固定诊断类别与界面文案独立，避免把提示或异常详情写进日志。 */
        public DiagnosticEventKind diagnosticKind() {
            return switch (failure) {
                case PERMISSION -> DiagnosticEventKind.VOICE_FAILURE_PERMISSION;
                case UNAVAILABLE -> DiagnosticEventKind.VOICE_FAILURE_AUDIO;
                case MODEL -> DiagnosticEventKind.VOICE_FAILURE_MODEL;
                case RUNTIME -> runtimeMissing ? DiagnosticEventKind.VOICE_FAILURE_RUNTIME_MISSING
                    : DiagnosticEventKind.VOICE_FAILURE_RUNTIME;
                case CANCELLED -> DiagnosticEventKind.VOICE_CANCELLED;
                case EMPTY -> DiagnosticEventKind.VOICE_EMPTY;
            };
        }

        public Failure failure() {
            return failure;
        }

        /** {@link Failure#RUNTIME} 的一种：安装包不带语音运行库，voice-runtime 资源包也还没下载。调用方提示用户去设置里下载，而不是报组件损坏。 */
        public boolean runtimeMissing() {
            return runtimeMissing;
        }
    }

    private static final Object RUNTIME_LOCK = new Object();
    private static final String ONNXRUNTIME_LIBRARY = "libonnxruntime.so";
    private static final String SHERPA_LIBRARY = "libsherpa-onnx-c-api.so";

    private java.util.function.Consumer<DiagnosticEventKind> diagnostics = kind -> { };

    /** 由调用方回传到唯一的日志写入进程；识别器不保存诊断文件。 */
    public void diagnostics(java.util.function.Consumer<DiagnosticEventKind> sink) {
        diagnostics = sink == null ? kind -> { } : sink;
    }

    private void diagnostic(DiagnosticEventKind kind) {
        diagnostics.accept(kind);
    }

    private final AtomicBoolean stopped = new AtomicBoolean();
    private final AtomicBoolean cancelled = new AtomicBoolean();
    private final Object captureStartLock = new Object();
    private final Object handleLock = new Object();
    private long handle;

    /** 只在用户打开「上传语音以改进识别」且隐私判断允许时由键盘打开：把录到的 PCM 也留一份在内存里（录音本身最多 60 秒），识别结束后经 {@link #retainedAudio()} 取走。 */
    private volatile java.io.ByteArrayOutputStream retained;

    public void retainAudio(boolean value) {
        retained = value ? new java.io.ByteArrayOutputStream(WavAudio.SAMPLE_RATE * 2 * 4) : null;
    }

    /** 留下的 16 kHz 单声道 16 位小端 PCM；没有打开留存时为 null。取走后清空。 */
    public byte[] retainedAudio() {
        java.io.ByteArrayOutputStream value = retained;
        retained = null;
        if (value == null) return null;
        synchronized (value) {
            return value.toByteArray();
        }
    }

    /** 停止采集，使用已经录到的音频完成转写。 */
    public void stop() {
        synchronized (captureStartLock) {
            stopped.set(true);
        }
    }

    /** 任意线程取消；返回后不得再启动麦克风，正在解码的会话在下次检查时退出。 */
    public void cancel() {
        synchronized (captureStartLock) {
            cancelled.set(true);
            stopped.set(true);
        }
        synchronized (handleLock) {
            if (handle != 0) NativeClient.localSpeechCancel(handle);
        }
    }

    private void checkCancelled() throws Refused {
        if (cancelled.get()) throw new Refused(Failure.CANCELLED);
    }

    /**
     * Drop loaded models when the system asks this process for memory. Registered once per process on the application context, because the activity that ran the dictation is usually gone by then.
     */
    public static void watchMemory(Context context) {
        synchronized (MEMORY_LOCK) {
            if (watchingMemory) return;
            watchingMemory = true;
        }
        context.getApplicationContext().registerComponentCallbacks(new ComponentCallbacks2() {
            @Override public void onTrimMemory(int level) {
                if (level >= TRIM_MEMORY_BACKGROUND) releaseLater(0, 0);
            }
            @Override public void onConfigurationChanged(Configuration configuration) { }
            // Deprecated in favour of onTrimMemory, but still abstract in ComponentCallbacks and still delivered on older releases this app supports.
            @SuppressWarnings("deprecation")
            @Override public void onLowMemory() {
                releaseLater(0, 0);
            }
        });
    }

    /**
     * Record until {@link #stop()} and return the transcript.
     *
     * <p>Blocking, and never called on the main thread: it loads a model of hundreds of megabytes the first time, holds the microphone while the user speaks and decodes as the audio arrives.
     *
     * @param hostOptions the runtime options document (HostOptions) the hotwords are read with when none are supplied; empty skips hotwords
     * @param hotwordTexts the words the shared layer already resolved (the Tauri request's `hotwords`), parallel to `hotwordPinyin`; null or empty reads them through `hostOptions` instead
     */
    public String recognize(String modelDirectory, Path trustedRoot, String language, String hostOptions,
                            String[] hotwordTexts, String[] hotwordPinyin,
                            Listener listener) throws Refused {
        checkCancelled();
        diagnostic(DiagnosticEventKind.VOICE_LOCAL_MODEL_CHECK);
        checkCancelled();
        if (!LocalAsrPolicy.installed(modelDirectory, trustedRoot)) throw new Refused(Failure.MODEL);
        JSONObject manifest = manifest(modelDirectory, trustedRoot);
        if (manifest == null) throw new Refused(Failure.MODEL);
        String hotwordMode = manifestString(manifest, "hotwords");
        String kind = manifestString(manifest, "kind");
        JSONArray hotwords = supplied(hotwordTexts, hotwordPinyin);
        if (hotwords.length() == 0) hotwords = hotwords(hostOptions);
        checkCancelled();
        diagnostic(DiagnosticEventKind.VOICE_LOCAL_RUNTIME_CHECK);
        checkCancelled();
        requireRuntime(trustedRoot);
        checkCancelled();
        BlockingQueue<short[]> audio = new LinkedBlockingQueue<>();
        AtomicBoolean captureFailed = new AtomicBoolean();
        diagnostic(DiagnosticEventKind.VOICE_LOCAL_RECORDER_OPEN);
        AudioRecord recorder;
        synchronized (captureStartLock) {
            checkCancelled();
            if (stopped.get()) throw new Refused(Failure.EMPTY);
            recorder = openRecorder();
        }
        Thread capture = ThreadPolicy.namedThread("msime-local-asr-capture",
            () -> capture(recorder, audio, captureFailed, listener));
        long created = 0;
        boolean captureStarted = false;
        try {
            // 句柄创建失败也必须释放已打开的录音资源，两者共用清理范围。
            diagnostic(DiagnosticEventKind.VOICE_LOCAL_NATIVE_START);
            synchronized (handleLock) {
                checkCancelled();
                created = NativeClient.localSpeechCreate();
                handle = created;
            }
            // Capture starts before the model loads, so the first words are queued rather than lost while a cold model is read from storage.
            capture.start();
            captureStarted = true;
            String error = NativeClient.localSpeechStart(created, modelDirectory, language,
                LocalAsrPolicy.hotwordLines(texts(hotwords)), 0);
            if (cancelled.get()) throw new Refused(Failure.CANCELLED);
            if (error != null) throw new Refused(Failure.MODEL);
            String text = decode(created, capture, audio, listener);
            if (captureFailed.get()) throw new Refused(Failure.UNAVAILABLE);
            if (LocalAsrPolicy.correctsByPinyin(hotwordMode) && hotwords.length() > 0) {
                text = corrected(text, hotwords);
            }
            text = numbersNormalized(text, kind);
            text = TextPolicy.trimmed(text);
            if (text.isEmpty()) throw new Refused(Failure.EMPTY);
            return text;
        } catch (IllegalStateException error) {
            // The session throws once cancelled; anything else is the recognizer failing.
            throw new Refused(cancelled.get() ? Failure.CANCELLED : Failure.RUNTIME);
        } finally {
            stop();
            joinQuietly(capture);
            if (!captureStarted) recorder.release();
            synchronized (handleLock) {
                handle = 0;
                if (created != 0) NativeClient.localSpeechDestroy(created);
            }
            releaseLater(LocalAsrPolicy.IDLE_RELEASE_MILLIS,
                LocalAsrPolicy.IDLE_RELEASE_MILLIS + 5_000);
        }
    }

    /**
     * 载入 sherpa-onnx 运行库，载入不了时报 {@link Failure#RUNTIME}。
     *
     * <p>安装包带着运行库时按名字就能载入，Play 版（{@link AppVersionPolicy#playDistribution()}）只走这一条。不带时（full 版改为按需下载）从 {@code files} 下已安装的 voice-runtime 资源包按绝对路径载入：libsherpa-onnx-c-api.so 按 soname 依赖 libonnxruntime.so 又没有 RUNPATH，链接器只认同一命名空间里已经载入的那份，所以先 {@link System#load} 它，再把 sherpa 的路径交给共享识别器。资源包没装时报 {@link Refused#runtimeMissing()}。这里只读不装：输入法进程从不下载资源包。载入成功后整个进程一直用这一份。
     */
    private static void requireRuntime(Path files) throws Refused {
        synchronized (RUNTIME_LOCK) {
            if (NativeClient.localSpeechAvailable()) return;
            // Play 版只用包里自带的运行库，从不载入 files 下的 .so：Play 的政策不允许执行从别处下载的代码。
            if (AppVersionPolicy.playDistribution()) throw new Refused(Failure.RUNTIME);
            File directory = files == null ? null
                : ResourcePacks.directory(files.toFile(), ResourcePacks.VOICE_RUNTIME);
            if (directory == null) throw new Refused(Failure.RUNTIME, true);
            File onnxruntime = new File(directory, ONNXRUNTIME_LIBRARY);
            File sherpa = new File(directory, SHERPA_LIBRARY);
            if (!readOnlyLibrary(onnxruntime) || !readOnlyLibrary(sherpa)) throw new Refused(Failure.RUNTIME);
            try {
                System.load(onnxruntime.getAbsolutePath());
            } catch (UnsatisfiedLinkError error) {
                throw new Refused(Failure.RUNTIME);
            }
            NativeClient.localSpeechRuntime(sherpa.getAbsolutePath());
            if (!NativeClient.localSpeechAvailable()) throw new Refused(Failure.RUNTIME);
        }
    }

    /** 普通文件（不是符号链接），并且在载入前是只读的。Android 14 起要求动态载入的代码文件只读；共享层发布资源包时已经设成只读，这里仍可写就先改成只读，改不了就不载入。 */
    private static boolean readOnlyLibrary(File file) {
        Path path = file.toPath();
        if (!Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)) return false;
        if (Files.isWritable(path) && !file.setReadOnly()) return false;
        return !Files.isWritable(path);
    }

    private String decode(long session, Thread capture, BlockingQueue<short[]> audio,
                          Listener listener) {
        String last = "";
        boolean speaking = false;
        while (!cancelled.get()) {
            short[] chunk;
            try {
                chunk = audio.poll(100, TimeUnit.MILLISECONDS);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                cancel();
                break;
            }
            if (chunk == null) {
                if (!capture.isAlive() && audio.isEmpty()) break;
                continue;
            }
            String partial = LocalAsrPolicy.transcript(
                NativeClient.localSpeechAccept(session, chunk, chunk.length));
            if (partial != null && !partial.equals(last)) {
                last = partial;
                if (listener != null) listener.onPartial(partial);
            }
            boolean now = NativeClient.localSpeechSpeaking(session);
            if (now != speaking) {
                speaking = now;
                if (listener != null) listener.onSpeech(now);
            }
        }
        return LocalAsrPolicy.transcript(NativeClient.localSpeechFinish(session));
    }

    private static AudioRecord openRecorder() throws Refused {
        int minimum = VoicePcmRecorder.minimumBufferBytes();
        if (minimum <= 0) throw new Refused(Failure.UNAVAILABLE);
        AudioRecord recorder;
        try {
            recorder = VoicePcmRecorder.create(BoundsPolicy.atLeast(WavAudio.SAMPLE_RATE * 2, minimum));
        } catch (IllegalArgumentException | SecurityException error) {
            throw new Refused(Failure.PERMISSION);
        }
        if (recorder.getState() != AudioRecord.STATE_INITIALIZED) {
            // An uninitialised recorder here is almost always the missing runtime permission; the constructor does not throw for it on every release.
            recorder.release();
            throw new Refused(Failure.PERMISSION);
        }
        return recorder;
    }

    /** 独立线程采集到停止或时限，始终释放麦克风；启动与取消共用同一把锁。 */
    private void capture(AudioRecord recorder, BlockingQueue<short[]> audio,
                         AtomicBoolean failed, Listener listener) {
        try {
            diagnostic(DiagnosticEventKind.VOICE_LOCAL_CAPTURE_START);
            synchronized (captureStartLock) {
                if (stopped.get()) return;
                recorder.startRecording();
            }
            if (recorder.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) {
                diagnostic(DiagnosticEventKind.VOICE_LOCAL_CAPTURE_START_FAILURE);
                failed.set(true);
                return;
            }
            long limit = (long) WavAudio.SAMPLE_RATE * LocalAsrPolicy.MAX_MILLIS / 1000;
            long captured = 0;
            while (!stopped.get() && captured < limit) {
                short[] chunk = new short[CHUNK_SAMPLES];
                int requested = VoiceCapturePolicy.readLength(limit, captured, chunk.length);
                if (requested == 0) break;
                int read = recorder.read(chunk, 0, requested);
                if (read < 0) {
                    diagnostic(DiagnosticEventKind.VOICE_LOCAL_CAPTURE_READ_FAILURE);
                    failed.set(true);
                    return;
                }
                if (read == 0) continue;
                if (listener != null) listener.onVolume(VoiceAudioLevel.fromPcm(chunk, read));
                captured += read;
                java.io.ByteArrayOutputStream keep = retained;
                if (keep != null) {
                    byte[] bytes = new byte[read * 2];
                    for (int index = 0; index < read; index++) {
                        bytes[index * 2] = (byte) chunk[index];
                        bytes[index * 2 + 1] = (byte) (chunk[index] >> 8);
                    }
                    synchronized (keep) { keep.write(bytes, 0, bytes.length); }
                }
                audio.add(read == chunk.length ? chunk : java.util.Arrays.copyOf(chunk, read));
            }
        } catch (IllegalStateException error) {
            diagnostic(DiagnosticEventKind.VOICE_LOCAL_CAPTURE_EXCEPTION);
            failed.set(true);
        } finally {
            VoicePcmRecorder.stopIfRecording(recorder);
            recorder.release();
        }
    }

    private static void joinQuietly(Thread thread) {
        if (thread.getState() == Thread.State.NEW) return;
        try {
            thread.join();
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
        }
    }

    /** 已安装模型的清单；读不出来或不是 JSON 对象时为 null。 */
    private static JSONObject manifest(String modelDirectory, Path trustedRoot) {
        try {
            byte[] bytes = LocalAsrPolicy.readManifest(modelDirectory, trustedRoot);
            return new JSONObject(TextPolicy.utf8(bytes));
        } catch (IOException | JSONException error) {
            return null;
        }
    }

    /** 清单里的一个字符串字段（`hotwords` 是 "native"、"pinyin" 或 ""，`kind` 是模型类型）；缺失、null 或不是字符串时为空串。 */
    private static String manifestString(JSONObject manifest, String key) {
        if (manifest.isNull(key)) return "";
        return JsonPolicy.strictStringOrEmpty(manifest.opt(key));
    }

    /**
     * The user's pinyin words, heaviest first. Best effort: a busy or unavailable dictionary costs only the bias, so any failure is an empty list rather than a failed dictation.
     */
    private static JSONArray hotwords(String hostOptions) {
        if (hostOptions == null || hostOptions.isEmpty()) return new JSONArray();
        try {
            JSONObject request = new JSONObject()
                .put("options", new JSONObject(hostOptions))
                .put("limit", LocalAsrPolicy.HOTWORD_LIMIT);
            JSONObject response = new JSONObject(NativeClient.voiceHotwords(request.toString()));
            if (!JsonPolicy.strictTrue(response.opt("ok"))) {
                return new JSONArray();
            }
            JSONObject value = JsonPolicy.objectValue(response);
            JSONArray words = value == null ? null : value.optJSONArray("hotwords");
            return words == null ? new JSONArray() : words;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return new JSONArray();
        }
    }

    /** The caller's resolved hotwords as the `[{text, pinyin}]` the shared correction takes; mismatched or invalid entries are dropped. */
    private static JSONArray supplied(String[] texts, String[] pinyin) {
        JSONArray out = new JSONArray();
        if (texts == null || pinyin == null || texts.length != pinyin.length) return out;
        try {
            for (int index = 0; index < texts.length && out.length() < LocalAsrPolicy.HOTWORD_LIMIT; index++) {
                if (!LocalAsrPolicy.suppliedHotword(texts[index], pinyin[index])) continue;
                out.put(new JSONObject().put("text", TextPolicy.trimmed(texts[index]))
                    .put("pinyin", pinyin[index]));
            }
        } catch (JSONException error) {
            return new JSONArray();
        }
        return out;
    }

    private static List<String> texts(JSONArray hotwords) {
        List<String> out = new ArrayList<>(hotwords.length());
        for (int index = 0; index < hotwords.length(); index++) {
            JSONObject word = hotwords.optJSONObject(index);
            if (word == null || word.isNull("text")) continue;
            String text = JsonPolicy.strictString(word.opt("text"));
            if (text != null) out.add(text);
        }
        return out;
    }

    /** The shared pinyin correction, or the transcript unchanged if it cannot run. */
    private static String corrected(String text, JSONArray hotwords) {
        try {
            JSONObject request = new JSONObject().put("text", text).put("hotwords", hotwords);
            JSONObject response = new JSONObject(NativeClient.voiceHotwordCorrect(request.toString()));
            JSONObject value = JsonPolicy.objectValue(response);
            if (value == null || value.isNull("text")) return text;
            String corrected = LocalAsrPolicy.transcript(value.opt("text"));
            return corrected.isEmpty() ? text : corrected;
        } catch (JSONException | RuntimeException error) {
            return text;
        }
    }

    /** 共享层按模型 kind 把最终文本里的汉字数字改成数字；kind 不需要改写时共享层原样返回。调用失败时退回未改写的文本，数字写法不值得让一次识别失败。 */
    private static String numbersNormalized(String text, String kind) {
        if (kind.isEmpty() || text.isEmpty()) return text;
        try {
            JSONObject request = new JSONObject().put("text", text).put("kind", kind);
            JSONObject response = new JSONObject(NativeClient.voiceNormalizeNumbers(request.toString()));
            JSONObject value = JsonPolicy.objectValue(response);
            if (value == null || value.isNull("text")) return text;
            String normalized = LocalAsrPolicy.transcript(value.opt("text"));
            return normalized.isEmpty() ? text : normalized;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return text;
        }
    }

    /** Drop idle models after `delayMillis` on a daemon thread that outlives the activity. */
    private static void releaseLater(long idleMillis, long delayMillis) {
        ScheduledExecutorService executor;
        synchronized (MEMORY_LOCK) {
            if (releaser == null) {
                releaser = Executors.newSingleThreadScheduledExecutor(
                    ThreadPolicy.namedDaemonFactory("msime-local-asr-release"));
            }
            executor = releaser;
        }
        executor.schedule(() -> NativeClient.localSpeechRelease(idleMillis), delayMillis,
            TimeUnit.MILLISECONDS);
    }
}
