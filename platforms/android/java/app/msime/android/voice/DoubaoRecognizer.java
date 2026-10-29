package app.msime.android;

import android.media.AudioRecord;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.concurrent.atomic.AtomicBoolean;
import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.SSLParameters;
import javax.net.ssl.SSLPeerUnverifiedException;
import javax.net.ssl.SSLSocket;
import javax.net.ssl.SSLSocketFactory;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Streaming recognition over the provider's WebSocket, transcribing while the user speaks.
 *
 * <p>Different in kind from the upload path: the audio leaves as it is captured and the transcript
 * comes back in pieces, so the user sees words appear rather than waiting for a round trip after
 * they stop. That is what the protocol is for, and it is why it is worth carrying a WebSocket
 * implementation for one endpoint.
 *
 * <p>协议本身不在这里决定：鉴权头由共享策略构造好传进来，会话发出的每一帧都经 {@link NativeClient} 由共享层构造和解码。本类持有 socket 和麦克风，边录边发、同时一直读回包的收发过程在 {@link DoubaoStreamSession}。
 */
public final class DoubaoRecognizer {
    private static final int CONNECT_TIMEOUT_MILLIS = 10_000;
    /** 握手回应的读超时，也是发出最后一帧之后等最终结果的时限。 */
    private static final int READ_TIMEOUT_MILLIS = 30_000;
    /**
     * 单次录音的安全上限：五分钟。流式协议边说边识别，不像整句上传那样受请求体大小和读超时的限制，这个上限只防忘了关麦克风；以前和整句上传一样是 60 秒，到点悄悄截断（#9886）。
     */
    public static final int MAX_MILLIS = 5 * 60_000;
    /** 约 100 ms 的 16 kHz 单声道 PCM：小到能流起来，大到不至于频繁发帧。 */
    private static final int CHUNK_BYTES = 3200;
    /** 留给语音贡献的音频上限，与 {@link VoiceContributionApi#MAX_DURATION_MILLIS} 一致。 */
    private static final long RETAIN_LIMIT_BYTES =
        (long) WavAudio.SAMPLE_RATE * 2 * VoiceContributionApi.MAX_DURATION_MILLIS / 1000;

    /** What the caller does with each update; interim results arrive before the final one. */
    public interface Listener {
        void onUpdate(String text, boolean finalResult);
    }

    private final AtomicBoolean stopped = new AtomicBoolean();
    private final AtomicBoolean cancelled = new AtomicBoolean();
    private final SecureRandom random = new SecureRandom();
    /** The plain socket under TLS, so cancel() can unblock a pending read without TLS I/O. */
    private volatile java.net.Socket transport;
    /** 这次录音是否到了 {@link #MAX_MILLIS} 由识别器自己结束。 */
    private volatile boolean capped;
    /** 已经识别出文字之后连接才失败时的原因；正常结束为 null。 */
    private volatile HttpAsrRecognizer.Failure interruption;

    /** 只在用户打开「上传语音以改进识别」且隐私判断允许时由键盘打开：把这次送出的 PCM 也留一份在内存里（最多 60 秒，超过就整段不留），识别结束后经 {@link #retainedAudio()} 取走。 */
    private volatile ByteArrayOutputStream retained;

    public void retainAudio(boolean value) {
        retained = value ? new ByteArrayOutputStream(WavAudio.SAMPLE_RATE * 2 * 4) : null;
    }

    /** 留下的 16 kHz 单声道 16 位 PCM；没有打开留存或录音超过 60 秒时为 null。取走后清空。 */
    public byte[] retainedAudio() {
        ByteArrayOutputStream value = retained;
        retained = null;
        if (value == null) return null;
        synchronized (value) {
            return value.toByteArray();
        }
    }

    /** 每读到一块 PCM 在录音线程回传 0–1 音量（{@link VoiceAudioLevel#fromPcm16le}）；界面更新由调用方切回主线程。 */
    private volatile java.util.function.Consumer<Float> levels = level -> { };

    public void levels(java.util.function.Consumer<Float> sink) {
        levels = sink == null ? level -> { } : sink;
    }

    /** 录音是否到了单次上限、由识别器自己结束；{@link #recognize} 返回后读取。 */
    public boolean capped() {
        return capped;
    }

    /** {@link #recognize} 返回了文字、但连接在最终结果之前就失败时的原因；正常结束为 null。 */
    public HttpAsrRecognizer.Failure interruption() {
        return interruption;
    }

    public void stop() {
        stopped.set(true);
    }

    public void cancel() {
        cancelled.set(true);
        stopped.set(true);
        closeTransport();
    }

    /** 关掉 TLS 下面的那层 socket：阻塞中的读写都会立即失败，而且不需要 TLS 的收尾 I/O。 */
    private void closeTransport() {
        java.net.Socket active = transport;
        if (active != null) {
            try {
                active.close();
            } catch (IOException | RuntimeException ignored) {
                // 阻塞的读写反正都会失败，这正是要的结果。
            }
        }
    }

    /**
     * 跑一次流式识别，返回识别出的文字；没有文字时抛出 {@link HttpAsrRecognizer.Refused}，原因和整句上传归在同一组里，调用方用 {@link VoiceFailurePolicy} 翻成提示。
     *
     * <p>识别到一半连接断了，已经识别出的文字照样返回，原因经 {@link #interruption()} 取；录音到了 {@link #MAX_MILLIS} 由 {@link #capped()} 说明。
     *
     * <p>阻塞，不能在主线程调用：用户说话期间它一直占着麦克风和连接。
     */
    public String recognize(String endpoint, String[] headers, boolean itn, boolean punctuation,
                            boolean ddc, String boostingTableId, Listener listener)
            throws HttpAsrRecognizer.Refused {
        capped = false;
        interruption = null;
        // 地址或起始帧在本机就构造不出来，是服务配置的问题。
        Endpoint descriptor = Endpoint.parse(endpoint);
        if (descriptor == null) throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.SERVICE);
        byte[] start = NativeClient.doubaoStartFrame(itn, punctuation, ddc, boostingTableId);
        if (start == null) throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.SERVICE);
        AudioRecord recorder = null;
        SSLSocket socket = null;
        try {
            socket = connect(descriptor, headers);
            if (cancelled.get()) throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.CANCELLED);
            // 握手之后读不再设超时：说话期间服务端可以很久不回包（整句流式只在说完后回），等最终结果的时限由会话自己计。
            socket.setSoTimeout(0);
            OutputStream out = socket.getOutputStream();
            sendFrame(out, WebSocketFrames.OPCODE_BINARY, start);
            recorder = record();
            DoubaoStreamSession.Listener updates = listener == null ? null : listener::onUpdate;
            long limit = (long) WavAudio.SAMPLE_RATE * 2 * MAX_MILLIS / 1000;
            DoubaoStreamSession.Outcome outcome = new DoubaoStreamSession(socket.getInputStream(),
                connection(out), microphone(recorder), codec(), updates, stopped, cancelled,
                limit, CHUNK_BYTES, READ_TIMEOUT_MILLIS).run();
            return transcript(outcome);
        } catch (SocketTimeoutException error) {
            throw new HttpAsrRecognizer.Refused(cancelled.get()
                ? HttpAsrRecognizer.Failure.CANCELLED : HttpAsrRecognizer.Failure.TIMEOUT);
        } catch (IOException error) {
            throw new HttpAsrRecognizer.Refused(cancelled.get()
                ? HttpAsrRecognizer.Failure.CANCELLED : HttpAsrRecognizer.Failure.NETWORK);
        } finally {
            if (recorder != null) {
                VoicePcmRecorder.stopIfRecording(recorder);
                recorder.release();
            }
            transport = null;
            if (socket != null) {
                try {
                    socket.close();
                } catch (IOException ignored) {
                    // 会话已经结束。
                }
            }
        }
    }

    /** 会话结果交给调用方：有文字就返回文字，没有文字按原因抛出。 */
    private String transcript(DoubaoStreamSession.Outcome outcome) throws HttpAsrRecognizer.Refused {
        capped = outcome.capped();
        if (outcome.failure() == HttpAsrRecognizer.Failure.CANCELLED) {
            throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.CANCELLED);
        }
        String text = outcome.text();
        if (text == null || text.isEmpty()) {
            throw new HttpAsrRecognizer.Refused(outcome.failure() == null
                ? HttpAsrRecognizer.Failure.EMPTY : outcome.failure());
        }
        interruption = outcome.failure();
        return text;
    }

    private DoubaoStreamSession.Connection connection(OutputStream out) {
        return new DoubaoStreamSession.Connection() {
            @Override public void send(int opcode, byte[] payload) throws IOException {
                sendFrame(out, opcode, payload);
            }

            @Override public void abort() {
                closeTransport();
            }
        };
    }

    private DoubaoStreamSession.Microphone microphone(AudioRecord recorder) {
        return new DoubaoStreamSession.Microphone() {
            @Override public int read(byte[] buffer, int length) {
                int read = recorder.read(buffer, 0, length);
                if (read > 0) {
                    levels.accept(VoiceAudioLevel.fromPcm16le(buffer, read));
                    retain(buffer, read);
                }
                return read;
            }

            @Override public void stop() {
                VoicePcmRecorder.stopIfRecording(recorder);
            }
        };
    }

    private DoubaoStreamSession.Codec codec() {
        return new DoubaoStreamSession.Codec() {
            @Override public byte[] audioFrame(int sequence, byte[] pcm, int length, boolean last) {
                return NativeClient.doubaoAudioFrame(sequence, pcm, length, last);
            }

            @Override public DoubaoStreamSession.Update decode(byte[] payload) {
                return update(payload);
            }
        };
    }

    /** 语音贡献只收 60 秒以内的整段音频；超过就整段不留，截掉后半段的音频和整段识别文字对不上。 */
    private void retain(byte[] chunk, int length) {
        ByteArrayOutputStream keep = retained;
        if (keep == null) return;
        synchronized (keep) {
            if (keep.size() + (long) length > RETAIN_LIMIT_BYTES) {
                retained = null;
                return;
            }
            keep.write(chunk, 0, length);
        }
    }

    /** One decoded response frame, read through the shared decoder. */
    private DoubaoStreamSession.Update update(byte[] payload) {
        try {
            JSONObject response = new JSONObject(NativeClient.doubaoDecodeFrame(payload));
            if (!JsonPolicy.strictTrue(response.opt("ok"))) return null;
            JSONObject value = JsonPolicy.objectValue(response);
            if (value == null) return null;
            // 错误帧结束会话。错误码不显示给用户，只用来区分空音频、服务端超时、服务过载和拒绝请求。
            if (value.has("error_code")) {
                return DoubaoStreamSession.Update.failed(
                    VoiceFailurePolicy.doubaoError(JsonPolicy.strictInteger(value.opt("error_code"))));
            }
            String payloadText = JsonPolicy.strictString(value.opt("payload"));
            if (payloadText == null) return null;
            String text = resultText(new JSONObject(payloadText));
            Boolean last = JsonPolicy.strictBoolean(value.opt("last"));
            return last == null ? null : new DoubaoStreamSession.Update(text, last);
        } catch (JSONException error) {
            return null;
        }
    }

    /**
     * 回包里的识别文字。双向流式（{@code bigmodel_async}）的 {@code result} 是一个对象；整句流式（{@code bigmodel_nostream}）的文档把它写成分句列表，iOS 和 Windows 两种都读，这里也按顺序拼起分句。以前只读对象，服务按列表回时读出来是空的，现在会被当成「没有听到内容」。
     */
    private static String resultText(JSONObject document) {
        Object result = document.opt("result");
        if (result instanceof JSONObject object) return HttpAsrPolicy.strictText(object.opt("text"));
        if (!(result instanceof JSONArray segments)) return "";
        StringBuilder joined = new StringBuilder();
        for (int index = 0; index < segments.length(); index++) {
            JSONObject segment = segments.optJSONObject(index);
            if (segment != null) joined.append(HttpAsrPolicy.strictText(segment.opt("text")));
        }
        return HttpAsrPolicy.strictText(joined.toString());
    }

    private SSLSocket connect(Endpoint endpoint, String[] headers)
            throws IOException, HttpAsrRecognizer.Refused {
        int port = endpoint.port;
        // Connect first, then hand the connected socket to TLS, so the connect attempt is bounded:
        // SSLSocketFactory.createSocket(host, port) connects with no timeout at all.
        java.net.Socket plain = new java.net.Socket();
        transport = plain;
        SSLSocket socket = null;
        try {
            plain.connect(new java.net.InetSocketAddress(endpoint.socketHost, port),
                CONNECT_TIMEOUT_MILLIS);
            SSLSocketFactory factory = (SSLSocketFactory) SSLSocketFactory.getDefault();
            socket = (SSLSocket) factory.createSocket(plain, endpoint.socketHost, port, true);
            // A raw SSLSocket checks the chain but not the hostname unless asked to.
            SSLParameters parameters = socket.getSSLParameters();
            parameters.setEndpointIdentificationAlgorithm("HTTPS");
            socket.setSSLParameters(parameters);
            socket.setSoTimeout(READ_TIMEOUT_MILLIS);
            socket.startHandshake();
            if (!HttpsURLConnection.getDefaultHostnameVerifier()
                    .verify(endpoint.socketHost, socket.getSession())) {
                throw new SSLPeerUnverifiedException("certificate does not match " + endpoint.socketHost);
            }
            String key = Base64.getEncoder().encodeToString(randomBytes(16));
            socket.getOutputStream().write(WebSocketFrames
                .handshakeRequest(endpoint.hostHeader, endpoint.requestTarget, key, headers)
                .getBytes(StandardCharsets.US_ASCII));
            socket.getOutputStream().flush();
            String response = readHandshake(socket.getInputStream());
            if (!WebSocketFrames.handshakeAccepted(response, key)) {
                // 握手被拒：按状态码区分鉴权被拒、服务超时和超出限制，读不出状态码是连接在握手中途断了。
                HttpAsrRecognizer.Failure failure = VoiceFailurePolicy.handshake(response);
                socket.close();
                throw new HttpAsrRecognizer.Refused(failure);
            }
            return socket;
        } catch (IOException | RuntimeException error) {
            try {
                if (socket != null) {
                    socket.close();
                } else {
                    plain.close();
                }
            } catch (IOException ignored) {
                // Already failing; the original error is the one worth reporting.
            }
            throw error;
        }
    }

    /** Read exactly the response head, leaving any frame bytes that followed it in the stream. */
    private static String readHandshake(InputStream in) throws IOException {
        StringBuilder head = new StringBuilder(8192);
        int matched = 0;
        while (head.length() < 8192) {
            int value = in.read();
            if (value < 0) break;
            head.append((char) value);
            char expected = "\r\n\r\n".charAt(matched);
            matched = value == expected ? matched + 1 : value == '\r' ? 1 : 0;
            if (matched == 4) break;
        }
        return head.toString();
    }

    /** 打开麦克风；失败原因与整句上传的采集一致：构造失败或没初始化多半是没有录音权限，开始录音失败是麦克风被占用。 */
    private AudioRecord record() throws HttpAsrRecognizer.Refused {
        int minimum = VoicePcmRecorder.minimumBufferBytes();
        if (minimum <= 0) throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.UNAVAILABLE);
        AudioRecord recorder;
        try {
            recorder = VoicePcmRecorder.create(BoundsPolicy.atLeast(CHUNK_BYTES * 4, minimum));
        } catch (IllegalArgumentException | SecurityException error) {
            throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.PERMISSION);
        }
        if (recorder.getState() != AudioRecord.STATE_INITIALIZED) {
            recorder.release();
            throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.PERMISSION);
        }
        try {
            recorder.startRecording();
        } catch (IllegalStateException error) {
            recorder.release();
            throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.UNAVAILABLE);
        }
        if (recorder.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) {
            recorder.release();
            throw new HttpAsrRecognizer.Refused(HttpAsrRecognizer.Failure.UNAVAILABLE);
        }
        return recorder;
    }

    /** 读线程回 pong 和采集线程发音频共用一条连接，整帧串行写出，两帧的字节不能交错。 */
    private void sendFrame(OutputStream out, int opcode, byte[] payload) throws IOException {
        byte[] frame = WebSocketFrames.clientFrame(opcode, payload, payload.length, mask());
        synchronized (out) {
            out.write(frame);
            out.flush();
        }
    }

    private byte[] mask() {
        return randomBytes(4);
    }

    private byte[] randomBytes(int count) {
        byte[] bytes = new byte[count];
        random.nextBytes(bytes);
        return bytes;
    }

    private static final class Endpoint {
        private final String socketHost;
        private final int port;
        private final String hostHeader;
        private final String requestTarget;

        private Endpoint(String socketHost, int port, String hostHeader, String requestTarget) {
            this.socketHost = socketHost;
            this.port = port;
            this.hostHeader = hostHeader;
            this.requestTarget = requestTarget;
        }

        /** URL 语法只由共享 Rust 解析器决定；Java 只校验类型化信封。 */
        private static Endpoint parse(String endpoint) {
            try {
                String response = NativeClient.voiceWebSocketEndpoint(endpoint);
                if (response == null || response.length() > 32768
                        || TextPolicy.utf8Length(response) > 32768) return null;
                JSONObject root = new JSONObject(response);
                JSONObject value = JsonPolicy.objectValue(root);
                if (value == null) return null;
                String socketHost = JsonPolicy.strictString(value.opt("socket_host"));
                Integer port = JsonPolicy.strictInteger(value.opt("port"));
                String hostHeader = JsonPolicy.strictString(value.opt("host_header"));
                String requestTarget = JsonPolicy.strictString(value.opt("request_target"));
                if (!validField(socketHost, 8192) || port == null || port < 0 || port > 65535
                        || !validField(hostHeader, 8192) || !validField(requestTarget, 8192)
                        || !requestTarget.startsWith("/")) return null;
                return new Endpoint(socketHost, port, hostHeader, requestTarget);
            } catch (JSONException | RuntimeException | LinkageError error) {
                return null;
            }
        }

        private static boolean validField(String value, int maximumBytes) {
            if (value == null || value.isEmpty() || TextPolicy.utf8Length(value) > maximumBytes) return false;
            for (int index = 0; index < value.length(); index++) {
                char unit = value.charAt(index);
                if (unit > 0x7e || unit < 0x21) return false;
            }
            return true;
        }
    }
}
