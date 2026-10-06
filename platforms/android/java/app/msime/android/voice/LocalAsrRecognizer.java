package app.msime.android;

import android.content.ComponentCallbacks2;
import android.content.Context;
import android.content.res.Configuration;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
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
 * <p>The recognizer is the shared one every desktop host uses (`shared/voice/LocalAsr`), compiled into this host's JNI library; the runtime is the pinned sherpa-onnx `.aar`'s native libraries, packaged beside it. This class owns the microphone and the two threads between it and that recognizer: capture reads the microphone into a queue so a slow decode never drops audio, and the calling worker loads the model, decodes, reports partial text and finishes.
 *
 * <p>Hotwords come from the user's own pinyin dictionary through the shared host API. A model that biases natively gets them at session start; one whose manifest says `"hotwords": "pinyin"` gets the final transcript corrected by the shared pinyin matcher instead.
 */
public final class LocalAsrRecognizer {
    /** Roughly 100 ms of 16 kHz mono audio per read. */
    private static final int CHUNK_SAMPLES = WavAudio.SAMPLE_RATE / 10;
    private static final Object MEMORY_LOCK = new Object();
    private static ScheduledExecutorService releaser;
    private static boolean watchingMemory;

    /** What the caller shows while the user speaks. Called on the recognizing worker. */
    public interface Listener {
        void onPartial(String text);
    }

    /** Why a recognition did not produce text. The caller maps these onto its own messages. */
    public enum Failure { PERMISSION, UNAVAILABLE, MODEL, RUNTIME, CANCELLED, EMPTY }

    /** Thrown rather than returning null so a caller cannot mistake a failure for silence. */
    public static final class Refused extends Exception {
        private static final long serialVersionUID = 1L;
        private final Failure failure;

        Refused(Failure failure) {
            super(failure.name());
            this.failure = failure;
        }

        public Failure failure() {
            return failure;
        }
    }

    private final AtomicBoolean stopped = new AtomicBoolean();
    private final AtomicBoolean cancelled = new AtomicBoolean();
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

    /** Stop recording and finish the transcript from what was captured. */
    public void stop() {
        stopped.set(true);
    }

    /** Abandon the dictation; a decode in progress stops at its next check. Any thread. */
    public void cancel() {
        cancelled.set(true);
        stopped.set(true);
        synchronized (handleLock) {
            NativeClient.localSpeechCancel(handle);
        }
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
        if (!LocalAsrPolicy.installed(modelDirectory, trustedRoot)) throw new Refused(Failure.MODEL);
        String hotwordMode = hotwordMode(modelDirectory, trustedRoot);
        if (hotwordMode == null) throw new Refused(Failure.MODEL);
        JSONArray hotwords = supplied(hotwordTexts, hotwordPinyin);
        if (hotwords.length() == 0) hotwords = hotwords(hostOptions);
        if (!NativeClient.localSpeechAvailable()) throw new Refused(Failure.RUNTIME);
        BlockingQueue<short[]> audio = new LinkedBlockingQueue<>();
        AtomicBoolean captureFailed = new AtomicBoolean();
        AudioRecord recorder = openRecorder();
        Thread capture = new Thread(() -> capture(recorder, audio, captureFailed),
            "msime-local-asr-capture");
        long created = NativeClient.localSpeechCreate();
        synchronized (handleLock) {
            handle = created;
        }
        try {
            // Capture starts before the model loads, so the first words are queued rather than lost while a cold model is read from storage.
            capture.start();
            String error = NativeClient.localSpeechStart(created, modelDirectory, language,
                LocalAsrPolicy.hotwordLines(texts(hotwords)), 0);
            if (cancelled.get()) throw new Refused(Failure.CANCELLED);
            if (error != null) throw new Refused(Failure.MODEL);
            String text = decode(created, capture, audio, listener);
            if (captureFailed.get()) throw new Refused(Failure.UNAVAILABLE);
            if (LocalAsrPolicy.correctsByPinyin(hotwordMode) && hotwords.length() > 0) {
                text = corrected(text, hotwords);
            }
            text = text.trim();
            if (text.isEmpty()) throw new Refused(Failure.EMPTY);
            return text;
        } catch (IllegalStateException error) {
            // The session throws once cancelled; anything else is the recognizer failing.
            throw new Refused(cancelled.get() ? Failure.CANCELLED : Failure.RUNTIME);
        } finally {
            stopped.set(true);
            joinQuietly(capture);
            synchronized (handleLock) {
                handle = 0;
                NativeClient.localSpeechDestroy(created);
            }
            releaseLater(LocalAsrPolicy.IDLE_RELEASE_MILLIS,
                LocalAsrPolicy.IDLE_RELEASE_MILLIS + 5_000);
        }
    }

    private String decode(long session, Thread capture, BlockingQueue<short[]> audio,
                          Listener listener) {
        String last = "";
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
            String partial = NativeClient.localSpeechAccept(session, chunk, chunk.length);
            if (partial != null && !partial.equals(last)) {
                last = partial;
                if (listener != null) listener.onPartial(partial);
            }
        }
        return NativeClient.localSpeechFinish(session);
    }

    private static AudioRecord openRecorder() throws Refused {
        int minimum = AudioRecord.getMinBufferSize(WavAudio.SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
        if (minimum <= 0) throw new Refused(Failure.UNAVAILABLE);
        AudioRecord recorder;
        try {
            recorder = new AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION,
                WavAudio.SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                BoundsPolicy.atLeast(WavAudio.SAMPLE_RATE * 2, minimum));
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

    /** Runs on its own thread until stopped or the time cap; always releases the microphone. */
    private void capture(AudioRecord recorder, BlockingQueue<short[]> audio,
                         AtomicBoolean failed) {
        try {
            recorder.startRecording();
            if (recorder.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) {
                failed.set(true);
                return;
            }
            long limit = (long) WavAudio.SAMPLE_RATE * LocalAsrPolicy.MAX_MILLIS / 1000;
            long captured = 0;
            while (!stopped.get() && captured < limit) {
                short[] chunk = new short[CHUNK_SAMPLES];
                int read = recorder.read(chunk, 0, chunk.length);
                if (read < 0) {
                    failed.set(true);
                    return;
                }
                if (read == 0) continue;
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
            failed.set(true);
        } finally {
            try {
                if (recorder.getRecordingState() == AudioRecord.RECORDSTATE_RECORDING) {
                    recorder.stop();
                }
            } catch (IllegalStateException ignored) {
                // Already stopped; release is what matters.
            }
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

    /** The manifest's hotword mode ("native", "pinyin" or ""), or null when it is not a manifest. */
    private static String hotwordMode(String modelDirectory, Path trustedRoot) {
        try {
            byte[] bytes = LocalAsrPolicy.readManifest(modelDirectory, trustedRoot);
            JSONObject manifest = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
            if (manifest.isNull("hotwords")) return "";
            return manifest.optString("hotwords", "");
        } catch (IOException | JSONException error) {
            return null;
        }
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
            if (!Boolean.TRUE.equals(LocalAsrPolicy.strictBoolean(response.opt("ok")))) {
                return new JSONArray();
            }
            JSONObject value = response.optJSONObject("value");
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
                out.put(new JSONObject().put("text", texts[index].trim()).put("pinyin", pinyin[index]));
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
            if (word != null && !word.isNull("text")) out.add(word.optString("text", ""));
        }
        return out;
    }

    /** The shared pinyin correction, or the transcript unchanged if it cannot run. */
    private static String corrected(String text, JSONArray hotwords) {
        try {
            JSONObject request = new JSONObject().put("text", text).put("hotwords", hotwords);
            JSONObject response = new JSONObject(NativeClient.voiceHotwordCorrect(request.toString()));
            JSONObject value = Boolean.TRUE.equals(LocalAsrPolicy.strictBoolean(response.opt("ok")))
                ? response.optJSONObject("value") : null;
            if (value == null || value.isNull("text")) return text;
            String corrected = LocalAsrPolicy.strictText(value.opt("text"));
            return corrected == null ? text : corrected;
        } catch (JSONException | RuntimeException error) {
            return text;
        }
    }

    /** Drop idle models after `delayMillis` on a daemon thread that outlives the activity. */
    private static void releaseLater(long idleMillis, long delayMillis) {
        ScheduledExecutorService executor;
        synchronized (MEMORY_LOCK) {
            if (releaser == null) {
                releaser = Executors.newSingleThreadScheduledExecutor(runnable -> {
                    Thread thread = new Thread(runnable, "msime-local-asr-release");
                    thread.setDaemon(true);
                    return thread;
                });
            }
            executor = releaser;
        }
        executor.schedule(() -> NativeClient.localSpeechRelease(idleMillis), delayMillis,
            TimeUnit.MILLISECONDS);
    }
}
