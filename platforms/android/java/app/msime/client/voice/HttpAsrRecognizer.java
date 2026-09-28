package app.msime.client;

import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Records with the microphone this host owns and uploads it to the configured provider.
 *
 * <p>This is the path the platform recognizer cannot take: it keeps its audio inside the system
 * service and transcribes with whatever engine the device ships. A user who has configured a
 * provider has chosen a different transcriber, and reaching it means holding the audio here.
 *
 * <p>Nothing about the provider is decided here. The shared layer resolved the endpoint, model and
 * token from the settings document and validated them; {@link HttpAsrPolicy} says whether this
 * host can speak that protocol and builds the body. What is left is the recording, the request and
 * the one field read back out of the response.
 */
public final class HttpAsrRecognizer {
    /** Long enough for a sentence, short enough that a forgotten session cannot fill storage. */
    private static final int MAX_MILLIS = 60_000;
    private static final int CONNECT_TIMEOUT_MILLIS = 15_000;
    private static final int READ_TIMEOUT_MILLIS = 60_000;

    /** Why a recognition did not produce text. The caller maps these onto its own outcomes. */
    public enum Failure { PERMISSION, UNAVAILABLE, CANCELLED, NETWORK, EMPTY }

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
    /** The in-flight upload, if recording has already finished. Cancel must unblock its read. */
    private volatile HttpURLConnection connection;

    /** Stop recording and transcribe what has been captured so far. */
    public void stop() {
        stopped.set(true);
    }

    /** Abandon the recording; nothing is uploaded and no text is produced. */
    public void cancel() {
        cancelled.set(true);
        stopped.set(true);
        HttpURLConnection active = connection;
        if (active != null) active.disconnect();
    }

    /**
     * Record until {@link #stop()} and return the provider's transcription.
     *
     * <p>Blocking, and never called on the main thread: it holds the microphone for as long as the
     * user is speaking and then waits on a network round trip.
     */
    public String recognize(String requestId, String language, String endpoint, String model,
                            String token) throws Refused {
        byte[] pcm = record();
        if (cancelled.get()) throw new Refused(Failure.CANCELLED);
        byte[] wav = WavAudio.wrap(pcm, pcm.length, WavAudio.SAMPLE_RATE);
        if (wav == null) throw new Refused(Failure.EMPTY);
        String boundary = HttpAsrPolicy.boundary(requestId);
        byte[] body = HttpAsrPolicy.multipartBody(boundary, model, language, wav);
        if (body.length > HttpAsrPolicy.MAX_AUDIO_BYTES) throw new Refused(Failure.EMPTY);
        return upload(endpoint, token, boundary, body);
    }

    private byte[] record() throws Refused {
        int minimum = AudioRecord.getMinBufferSize(WavAudio.SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT);
        if (minimum <= 0) throw new Refused(Failure.UNAVAILABLE);
        int buffer = Math.max(minimum, WavAudio.SAMPLE_RATE);
        AudioRecord recorder;
        try {
            recorder = new AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION,
                WavAudio.SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT, buffer);
        } catch (IllegalArgumentException | SecurityException error) {
            throw new Refused(Failure.PERMISSION);
        }
        try {
            if (recorder.getState() != AudioRecord.STATE_INITIALIZED) {
                // An uninitialised recorder here is almost always the missing runtime permission;
                // the constructor does not throw for it on every release.
                throw new Refused(Failure.PERMISSION);
            }
            try {
                recorder.startRecording();
            } catch (IllegalStateException error) {
                throw new Refused(Failure.UNAVAILABLE);
            }
            if (recorder.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) {
                throw new Refused(Failure.UNAVAILABLE);
            }
            ByteArrayOutputStream captured = new ByteArrayOutputStream();
            byte[] chunk = new byte[buffer];
            // Two bytes per sample: the byte budget is the time budget.
            int limit = WavAudio.SAMPLE_RATE * 2 / 1000 * MAX_MILLIS;
            while (!stopped.get() && captured.size() < limit) {
                int read = recorder.read(chunk, 0, chunk.length);
                if (read < 0) throw new Refused(Failure.UNAVAILABLE);
                captured.write(chunk, 0, read);
            }
            return captured.toByteArray();
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

    private String upload(String endpoint, String token, String boundary, byte[] body)
            throws Refused {
        HttpURLConnection opened = null;
        try {
            opened = (HttpURLConnection) new URL(endpoint).openConnection();
            connection = opened;
            if (cancelled.get()) throw new Refused(Failure.CANCELLED);
            // The bearer token belongs to this configured origin. Never let HttpURLConnection
            // replay it after a redirect to another host or protocol.
            opened.setInstanceFollowRedirects(false);
            opened.setConnectTimeout(CONNECT_TIMEOUT_MILLIS);
            opened.setReadTimeout(READ_TIMEOUT_MILLIS);
            opened.setRequestMethod("POST");
            opened.setDoOutput(true);
            opened.setFixedLengthStreamingMode(body.length);
            opened.setRequestProperty("Authorization", "Bearer " + token);
            opened.setRequestProperty("Content-Type",
                "multipart/form-data; boundary=" + boundary);
            opened.setRequestProperty("Accept", "application/json");
            try (OutputStream out = opened.getOutputStream()) {
                out.write(body);
            }
            if (cancelled.get()) throw new Refused(Failure.CANCELLED);
            int status = opened.getResponseCode();
            if (status < 200 || status >= 300) throw new Refused(Failure.NETWORK);
            String text;
            try (InputStream input = opened.getInputStream()) {
                String response = read(input);
                if (response == null) throw new Refused(Failure.NETWORK);
                text = text(response);
            }
            if (text.isEmpty()) throw new Refused(Failure.EMPTY);
            return text;
        } catch (IOException error) {
            throw new Refused(Failure.NETWORK);
        } finally {
            if (connection == opened) connection = null;
            if (opened != null) opened.disconnect();
        }
    }

    /** Returns null when the response exceeds the bound, without retaining the overflow. */
    private static String read(InputStream stream) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] chunk = new byte[8192];
        int read;
        // Bounded: a transcription response is text, and an unbounded read is how a wrong endpoint
        // becomes an out-of-memory failure in the input method's own process. Read one extra byte
        // when the limit is reached so a response that is exactly a valid prefix plus more data is
        // rejected instead of being parsed as if it were complete.
        while (out.size() <= 1024 * 1024) {
            int remaining = 1024 * 1024 - out.size();
            int requested = Math.min(chunk.length, remaining + 1);
            read = stream.read(chunk, 0, requested);
            if (read <= 0) break;
            if (read > remaining) return null;
            out.write(chunk, 0, read);
        }
        return out.toString(StandardCharsets.UTF_8.name());
    }

    /** The one field these APIs agree on. Anything else in the response is ignored. */
    private static String text(String response) {
        try {
            return new JSONObject(response).optString("text", "").trim();
        } catch (JSONException error) {
            return "";
        }
    }
}
