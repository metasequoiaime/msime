package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * 豆包流式的一次会话：采集线程只管把音频帧发出去，另起一条读线程从会话开始就一直读服务端的帧。
 *
 * <p>以前在同一个循环里发完一块音频，再用 {@link InputStream#available()} 看有没有回包。Android 的 TLS socket（Conscrypt）的 available() 只报已经解密、留在本机缓冲里的字节，不会去网络上取，所以说话期间一帧回包也读不到：中间文字不显示，服务端的回包在接收窗口里越积越多，窗口满了服务端写不出去就会断开连接。说得越久越容易断，正是短句正常、长句报网络断开的样子（#9886）。这里不再问 available()，读线程阻塞在 read 上，到一帧处理一帧。
 *
 * <p>本类不碰 Android、org.json 和 JNI：麦克风、帧的编解码和底层发送都由 {@link DoubaoRecognizer} 注入，JVM 冒烟可以直接驱动整个收发过程。
 */
final class DoubaoStreamSession {
    /** 共享豆包解码器接受 1 MiB 的线上帧，再给 WebSocket 帧头留 10 字节。 */
    static final int MAX_INBOUND_FRAME_BYTES = 1_048_576 + 10;
    /** 分片重组后的一条消息的上限。 */
    private static final int MAX_MESSAGE_BYTES = 8 * 1024 * 1024;
    /** 发送失败后等读线程读出关闭帧或错误帧的时长：写失败多半是服务端已经断开，原因在它最后发来的帧里。 */
    static final long WRITE_FAILURE_GRACE_MILLIS = 1_000;

    /** 采集来源。 */
    interface Microphone {
        /** 读最多 {@code length} 字节 PCM 到 {@code buffer} 开头，返回读到的字节数，负数表示麦克风出错。 */
        int read(byte[] buffer, int length);

        /** 停止采集。结束录音和会话收尾时都会调用，必须可以重复调用。 */
        void stop();
    }

    /** 豆包协议的帧，由共享层构造和解码。 */
    interface Codec {
        byte[] audioFrame(int sequence, byte[] pcm, int length, boolean last);

        /** 一条完整的服务端二进制消息；错误帧返回 {@link Update#failed}，读不懂时返回 null。 */
        Update decode(byte[] payload);
    }

    /** 已经握手完毕的连接。 */
    interface Connection {
        /** 发一帧。读线程回 pong 与采集线程发音频可能同时调用，实现方负责串行写出。 */
        void send(int opcode, byte[] payload) throws IOException;

        /** 关掉连接，让阻塞中的读和写立即失败。可以重复调用。 */
        void abort();
    }

    /** 每次收到非空的识别文字时在读线程回调；中间结果在最终结果之前。 */
    interface Listener {
        void onUpdate(String text, boolean finalResult);
    }

    /** 一条服务端消息：识别文字和它是不是最终结果；{@code failure} 不为 null 时是错误帧，服务端不会再给结果。 */
    record Update(String text, boolean last, HttpAsrRecognizer.Failure failure) {
        Update(String text, boolean last) {
            this(text, last, null);
        }

        /** 错误帧，原因由 {@link VoiceFailurePolicy#doubaoError} 按错误码归类。 */
        static Update failed(HttpAsrRecognizer.Failure failure) {
            return new Update(null, true, failure);
        }
    }

    /**
     * 会话结果。{@code text} 是最后一次非空的识别文字，失败时也可能有（识别到一半连接断了）；{@code failure} 为 null 表示正常收到了最终结果，最终结果没有任何文字时是 {@code EMPTY}；{@code capped} 表示录音到了上限、由会话自己结束。
     */
    record Outcome(String text, HttpAsrRecognizer.Failure failure, boolean capped) {}

    private final InputStream in;
    private final Connection connection;
    private final Microphone microphone;
    private final Codec codec;
    private final Listener listener;
    private final AtomicBoolean stopped;
    private final AtomicBoolean cancelled;
    private final long limitBytes;
    private final int chunkBytes;
    private final long finalTimeoutMillis;

    private final CountDownLatch done = new CountDownLatch(1);
    /** 读线程上解码（共享层 JNI）或回调抛出的意外异常；会话结束后在调用线程上重新抛出。 */
    private volatile Throwable crash;
    private final Object lock = new Object();
    /** 以下两项由 {@link #lock} 保护；{@link #done} 一旦放行，failure 不再改变。 */
    private String transcript;
    private HttpAsrRecognizer.Failure failure;

    /**
     * @param limitBytes 单次录音的上限（字节），到了就发最后一帧、等最终结果
     * @param finalTimeoutMillis 发出最后一帧之后等最终结果的时限
     */
    DoubaoStreamSession(InputStream in, Connection connection, Microphone microphone, Codec codec,
                        Listener listener, AtomicBoolean stopped, AtomicBoolean cancelled,
                        long limitBytes, int chunkBytes, long finalTimeoutMillis) {
        this.in = in;
        this.connection = connection;
        this.microphone = microphone;
        this.codec = codec;
        this.listener = listener;
        this.stopped = stopped;
        this.cancelled = cancelled;
        this.limitBytes = limitBytes;
        this.chunkBytes = chunkBytes;
        this.finalTimeoutMillis = finalTimeoutMillis;
    }

    /**
     * 在调用线程上采集并发送，直到收到最终结果、失败或取消。阻塞，不能在主线程调用。
     *
     * <p>读线程上解码或回调抛出的 {@link RuntimeException} 与 {@link LinkageError} 在这里原样重新抛出：以前解码在调用线程上做，调用方为共享 JNI 的意外失败写的兜底（提示「语音识别服务无法启动」）照样生效；留在读线程上会走到进程的未捕获异常处理，整个输入法进程退出。
     */
    Outcome run() {
        Thread reader = new Thread(this::receive, "msime-doubao-reader");
        reader.setDaemon(true);
        reader.start();
        Outcome outcome;
        try {
            outcome = send();
        } finally {
            microphone.stop();
            // 读线程可能还阻塞在 read 上（失败、超时或取消时），关掉连接让它退出。
            connection.abort();
        }
        Throwable error = crash;
        if (error instanceof RuntimeException runtime) throw runtime;
        if (error instanceof LinkageError linkage) throw linkage;
        return outcome;
    }

    private Outcome send() {
        byte[] chunk = new byte[chunkBytes];
        long sent = 0;
        int sequence = 1;
        boolean capped = false;
        while (true) {
            if (cancelled.get()) return outcome(false);
            // 读线程已经结束（服务端报错、关闭连接或提前给了最终结果），不必再录。
            if (done.getCount() == 0) return outcome(false);
            boolean stopping = stopped.get();
            boolean full = sent >= limitBytes;
            boolean last = stopping || full;
            if (last) {
                capped = full && !stopping;
                // 等最终结果之前先放开麦克风。
                microphone.stop();
            }
            int requested = last ? 0 : VoiceCapturePolicy.readLength(limitBytes, sent, chunk.length);
            int read = requested == 0 ? 0 : microphone.read(chunk, requested);
            if (read < 0) {
                finish(HttpAsrRecognizer.Failure.UNAVAILABLE);
                return outcome(false);
            }
            sent += read;
            byte[] frame = codec.audioFrame(sequence++, chunk, read, last);
            if (frame == null) {
                finish(HttpAsrRecognizer.Failure.SERVICE);
                return outcome(false);
            }
            try {
                connection.send(WebSocketFrames.OPCODE_BINARY, frame);
            } catch (IOException error) {
                await(WRITE_FAILURE_GRACE_MILLIS);
                finish(cancelled.get() ? HttpAsrRecognizer.Failure.CANCELLED : HttpAsrRecognizer.Failure.NETWORK);
                return outcome(capped);
            }
            if (last) break;
        }
        if (!await(finalTimeoutMillis)) finish(HttpAsrRecognizer.Failure.TIMEOUT);
        return outcome(capped);
    }

    /** 读线程：一直读到最终结果、关闭帧、连接断开或会话被取消。 */
    private void receive() {
        byte[] inbound = new byte[MAX_INBOUND_FRAME_BYTES];
        int pending = 0;
        ByteArrayOutputStream fragmented = null;
        try {
            while (true) {
                // 一帧占满了缓冲还解不出来，说明它超出了上限。不能把 0 字节的读交给 read()：那不是可移植的「再等等」，会被当成连接结束。
                if (pending == inbound.length) {
                    fail(HttpAsrRecognizer.Failure.SERVICE);
                    return;
                }
                int got = in.read(inbound, pending, inbound.length - pending);
                if (got < 0) {
                    // 没有关闭帧，连接直接断了。
                    fail(cancelled.get() ? HttpAsrRecognizer.Failure.CANCELLED : HttpAsrRecognizer.Failure.NETWORK);
                    return;
                }
                pending += got;
                WebSocketFrames.Frame decoded;
                while ((decoded = WebSocketFrames.decode(inbound, pending)) != null) {
                    System.arraycopy(inbound, decoded.consumed(), inbound, 0, pending - decoded.consumed());
                    pending -= decoded.consumed();
                    int opcode = decoded.opcode();
                    // 服务端在给出最终结果之前主动关闭：按关闭帧的状态码区分服务结束会话（多半是时长或空闲限制）、服务端故障和超出限制。
                    if (opcode == WebSocketFrames.OPCODE_CLOSE) {
                        fail(VoiceFailurePolicy.closed(WebSocketFrames.closeStatus(decoded.payload())));
                        return;
                    }
                    if (opcode == WebSocketFrames.OPCODE_PING) {
                        connection.send(WebSocketFrames.OPCODE_PONG, decoded.payload());
                        continue;
                    }
                    if (opcode != WebSocketFrames.OPCODE_BINARY && opcode != WebSocketFrames.OPCODE_CONTINUATION) {
                        continue;
                    }
                    byte[] payload;
                    if (opcode == WebSocketFrames.OPCODE_BINARY) {
                        // 上一条分片消息还没收完时不能开始新的数据消息。
                        if (fragmented != null) {
                            fail(HttpAsrRecognizer.Failure.SERVICE);
                            return;
                        }
                        if (decoded.fin()) {
                            payload = decoded.payload();
                        } else {
                            fragmented = new ByteArrayOutputStream(decoded.payload().length);
                            if (!append(fragmented, decoded.payload())) {
                                fail(HttpAsrRecognizer.Failure.SERVICE);
                                return;
                            }
                            continue;
                        }
                    } else {
                        // 续帧只能跟在没收完的二进制帧后面。
                        if (fragmented == null || !append(fragmented, decoded.payload())) {
                            fail(HttpAsrRecognizer.Failure.SERVICE);
                            return;
                        }
                        if (!decoded.fin()) continue;
                        payload = fragmented.toByteArray();
                        fragmented = null;
                    }
                    Update update = codec.decode(payload);
                    // 读不懂的回包：服务端不会再给结果。
                    if (update == null) {
                        fail(HttpAsrRecognizer.Failure.SERVICE);
                        return;
                    }
                    // 错误帧：原因按错误码归类，服务过载或超时不再提示用户去检查配置。
                    if (update.failure() != null) {
                        fail(update.failure());
                        return;
                    }
                    if (update.text() != null && !update.text().isEmpty()) {
                        synchronized (lock) {
                            transcript = update.text();
                        }
                        if (listener != null) listener.onUpdate(update.text(), update.last());
                    }
                    if (update.last()) {
                        finish(null);
                        return;
                    }
                }
            }
        } catch (IOException error) {
            fail(cancelled.get() ? HttpAsrRecognizer.Failure.CANCELLED : HttpAsrRecognizer.Failure.NETWORK);
        } catch (RuntimeException | LinkageError error) {
            // 记下来交给 run() 在调用线程上重新抛出，并立即结束会话，不让采集线程等到用户停下再等满最终结果的时限。
            crash = error;
            fail(HttpAsrRecognizer.Failure.SERVICE);
        }
    }

    private static boolean append(ByteArrayOutputStream message, byte[] payload) {
        if (payload.length > MAX_MESSAGE_BYTES - message.size()) return false;
        message.write(payload, 0, payload.length);
        return true;
    }

    /** 记下结局并放行等待的采集线程；只有第一次生效。 */
    private void finish(HttpAsrRecognizer.Failure reason) {
        synchronized (lock) {
            if (done.getCount() == 0) return;
            failure = reason;
            done.countDown();
        }
    }

    /** 读线程失败：记下原因，再关掉连接，让可能阻塞在写上的采集线程也立即返回。 */
    private void fail(HttpAsrRecognizer.Failure reason) {
        finish(reason);
        connection.abort();
    }

    /** 等结局最多 {@code millis}；被中断（宿主在拆除）时按取消处理。 */
    private boolean await(long millis) {
        try {
            return done.await(millis, TimeUnit.MILLISECONDS);
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            finish(HttpAsrRecognizer.Failure.CANCELLED);
            return true;
        }
    }

    private Outcome outcome(boolean capped) {
        if (cancelled.get()) return new Outcome(null, HttpAsrRecognizer.Failure.CANCELLED, false);
        synchronized (lock) {
            HttpAsrRecognizer.Failure reason = failure;
            if (reason == null && (transcript == null || transcript.isEmpty())) {
                reason = HttpAsrRecognizer.Failure.EMPTY;
            }
            return new Outcome(transcript, reason, capped);
        }
    }
}
