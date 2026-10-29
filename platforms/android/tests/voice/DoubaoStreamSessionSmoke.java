package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.io.InterruptedIOException;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.IntConsumer;

/**
 * 豆包流式会话的收发（#9886）：说话期间就读到服务端的中间结果，失败按原因归类，到上限自己结束。
 *
 * <p>服务端的回包从一个 {@code available()} 永远是 0 的流里来，和 Android 的 TLS socket（Conscrypt）一样：它只报已经解密留在本机的字节。以前靠 available() 判断有没有回包的循环在这样的流上说话期间一帧也读不到，第一个用例因此会在等中间结果时失败。
 */
public final class DoubaoStreamSessionSmoke {
    private static final int CHUNK = 10;

    public static void main(String[] args) throws Exception {
        interimArrivesWhileRecording();
        serverCloseIsClassifiedByStatus();
        serverCloseAfterTextKeepsTheText();
        droppedConnectionIsNetwork();
        silentServerAfterLastFrameTimesOut();
        emptyFinalIsEmpty();
        limitEndsTheRecordingAndSaysSo();
        cancelEndsWithoutText();
        pingIsAnsweredWhileRecording();
        errorFrameIsClassifiedByCode();
        decoderCrashReachesTheCaller();
        failedWriteWithoutServerReasonIsNetwork();
        fragmentedFinalIsReassembled();
        earlyFinalStopsTheMicrophone();
        System.out.println("DoubaoStreamSessionSmoke passed");
    }

    /** 说话期间服务端回的中间结果当场送到监听者，录音不用先停下来。 */
    private static void interimArrivesWhileRecording() throws Exception {
        Harness harness = new Harness();
        CountDownLatch interim = new CountDownLatch(1);
        AtomicBoolean seenWhileRecording = new AtomicBoolean();
        List<String> updates = new CopyOnWriteArrayList<>();
        harness.listener = (text, last) -> {
            updates.add(text + (last ? "|final" : "|interim"));
            if (!last) interim.countDown();
        };
        harness.onRead = index -> {
            if (index == 2) harness.server.push(binary("你好|0"));
            if (index == 3) seenWhileRecording.set(await(interim, 5_000));
            if (index == 5) harness.stopped.set(true);
        };
        harness.connection.onLastAudio = () -> harness.server.push(binary("你好世界|1"));
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(seenWhileRecording.get(), "中间结果必须在录音还没结束时就读到");
        check("你好世界".equals(outcome.text()) && outcome.failure() == null && !outcome.capped(), "正常拿到最终结果");
        check(updates.equals(List.of("你好|interim", "你好世界|final")), "中间结果在最终结果之前");
        check(harness.microphone.stopped, "结束时放开麦克风");
        check(harness.connection.lastAudioFrames.get() == 1, "最后一帧只发一次");
    }

    /**
     * 服务端在最终结果之前发关闭帧：录音随即结束，不会一直录到上限；原因按状态码分。没报错就正常关闭（没带状态码、1000）是服务结束了会话，多半是它的时长或空闲限制，不能再提示用户去检查配置；1011 是服务端故障；1008 这类才是拒绝请求。
     */
    private static void serverCloseIsClassifiedByStatus() throws Exception {
        closesWith(new byte[0], HttpAsrRecognizer.Failure.ENDED, "没带状态码的关闭帧算服务结束会话");
        closesWith(new byte[] {0x03, (byte) 0xe8}, HttpAsrRecognizer.Failure.ENDED, "1000 正常关闭算服务结束会话");
        closesWith(new byte[] {0x03, (byte) 0xf3}, HttpAsrRecognizer.Failure.BUSY, "1011 算服务端故障");
        closesWith(new byte[] {0x03, (byte) 0xf0}, HttpAsrRecognizer.Failure.SERVICE, "1008 算拒绝请求");
    }

    private static void closesWith(byte[] payload, HttpAsrRecognizer.Failure expected, String message)
            throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        harness.onRead = index -> {
            if (index == 2) harness.server.push(frame(WebSocketFrames.OPCODE_CLOSE, payload, true));
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(outcome.text() == null && outcome.failure() == expected, message);
        check(harness.microphone.reads.get() < 1_000, "服务端断开后不再继续录音");
        check(harness.microphone.stopped, "放开麦克风");
        check(!"语音服务拒绝请求，请检查服务配置".equals(VoiceFailurePolicy.message(outcome.failure()))
            || expected == HttpAsrRecognizer.Failure.SERVICE, "只有真被拒绝时才提示检查配置");
    }

    /** 已经识别出文字之后服务端断开：文字保留，原因一并给出。 */
    private static void serverCloseAfterTextKeepsTheText() throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        CountDownLatch interim = new CountDownLatch(1);
        harness.listener = (text, last) -> interim.countDown();
        harness.onRead = index -> {
            if (index == 2) harness.server.push(binary("说到一半|0"));
            if (index == 3) {
                await(interim, 5_000);
                harness.server.push(frame(WebSocketFrames.OPCODE_CLOSE, new byte[0], true));
            }
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check("说到一半".equals(outcome.text()), "已识别的文字保留");
        check(outcome.failure() == HttpAsrRecognizer.Failure.ENDED, "原因是服务结束了会话");
    }

    /** 连接没发关闭帧就断了：网络问题。 */
    private static void droppedConnectionIsNetwork() throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        harness.onRead = index -> {
            if (index == 2) harness.server.eof();
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(outcome.text() == null && outcome.failure() == HttpAsrRecognizer.Failure.NETWORK, "断线算网络");
    }

    /** 发完最后一帧服务端一直不回：按时限报超时，不会挂住。 */
    private static void silentServerAfterLastFrameTimesOut() throws Exception {
        Harness harness = new Harness();
        harness.finalTimeoutMillis = 200;
        harness.onRead = index -> {
            if (index == 2) harness.stopped.set(true);
        };
        long started = System.nanoTime();
        DoubaoStreamSession.Outcome outcome = harness.run();
        long elapsed = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started);
        check(outcome.failure() == HttpAsrRecognizer.Failure.TIMEOUT, "等最终结果超时");
        check(elapsed >= 200 && elapsed < 5_000, "按给定的时限结束");
    }

    /** 最终结果没有任何文字：空结果。 */
    private static void emptyFinalIsEmpty() throws Exception {
        Harness harness = new Harness();
        harness.onRead = index -> {
            if (index == 2) harness.stopped.set(true);
        };
        harness.connection.onLastAudio = () -> harness.server.push(binary("|1"));
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(outcome.text() == null && outcome.failure() == HttpAsrRecognizer.Failure.EMPTY, "空结果");
    }

    /** 录音到了上限：发出最后一帧、等最终结果，并标明是上限截断的。 */
    private static void limitEndsTheRecordingAndSaysSo() throws Exception {
        Harness harness = new Harness();
        harness.limit = CHUNK * 4;
        harness.connection.onLastAudio = () -> harness.server.push(binary("满了|1"));
        DoubaoStreamSession.Outcome outcome = harness.run();
        check("满了".equals(outcome.text()) && outcome.failure() == null, "到上限照样拿到结果");
        check(outcome.capped(), "标明是到了上限");
        check(harness.microphone.reads.get() == 4, "不多读一块");
        check(harness.connection.lastAudioFrames.get() == 1, "最后一帧只发一次");

        Harness stopped = new Harness();
        stopped.onRead = index -> {
            if (index == 2) stopped.stopped.set(true);
        };
        stopped.connection.onLastAudio = () -> stopped.server.push(binary("说完了|1"));
        check(!stopped.run().capped(), "用户自己结束时不算到上限");
    }

    /** 取消：关掉连接，没有文字也没有提示。 */
    private static void cancelEndsWithoutText() throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        harness.onRead = index -> {
            if (index == 2) {
                harness.server.push(binary("不要了|0"));
            }
            if (index == 3) {
                harness.cancelled.set(true);
                harness.stopped.set(true);
                harness.connection.abort();
            }
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(outcome.text() == null && outcome.failure() == HttpAsrRecognizer.Failure.CANCELLED, "取消");
    }

    /** 说话期间服务端的 ping 当场回 pong，保活不因为没人读而失效。 */
    private static void pingIsAnsweredWhileRecording() throws Exception {
        Harness harness = new Harness();
        CountDownLatch pong = harness.connection.pong;
        AtomicBoolean answered = new AtomicBoolean();
        harness.onRead = index -> {
            if (index == 1) harness.server.push(frame(WebSocketFrames.OPCODE_PING, bytes("p"), true));
            if (index == 3) {
                answered.set(await(pong, 5_000));
                harness.stopped.set(true);
            }
        };
        harness.connection.onLastAudio = () -> harness.server.push(binary("在|1"));
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(answered.get(), "录音期间回 pong");
        check("p".equals(harness.connection.pongPayload.get()), "pong 原样带回 ping 的内容");
        check("在".equals(outcome.text()), "保活不影响结果");
    }

    /** 错误帧按错误码归类：服务过载、服务端等包超时和拒绝请求各不相同；读不懂的回包按拒绝请求。 */
    private static void errorFrameIsClassifiedByCode() throws Exception {
        errorsWith("ERR:55000031", HttpAsrRecognizer.Failure.BUSY, "服务过载");
        errorsWith("ERR:55000002", HttpAsrRecognizer.Failure.BUSY, "服务内部错误");
        errorsWith("ERR:45000081", HttpAsrRecognizer.Failure.SERVICE_TIMEOUT, "服务端等包超时");
        errorsWith("ERR:45000001", HttpAsrRecognizer.Failure.SERVICE, "请求参数无效");
        errorsWith("ERR", HttpAsrRecognizer.Failure.SERVICE, "读不懂的回包");
    }

    private static void errorsWith(String payload, HttpAsrRecognizer.Failure expected, String message)
            throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        harness.onRead = index -> {
            if (index == 2) harness.server.push(binary(payload));
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(outcome.text() == null && outcome.failure() == expected, message);
        check(harness.microphone.reads.get() < 1_000, "错误帧之后不再继续录音");
    }

    /**
     * 读线程上解码（真机上是共享层 JNI）抛出意外异常：会话立即结束，异常回到调用线程，由调用方的兜底提示「语音识别服务无法启动」。留在读线程上会走到进程的未捕获异常处理，整个输入法退出；会话也不会结束，要等用户停下再等满最终结果的时限。
     */
    private static void decoderCrashReachesTheCaller() throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        harness.onRead = index -> {
            if (index == 2) harness.server.push(binary("BOOM"));
        };
        AtomicReference<Throwable> uncaught = new AtomicReference<>();
        Thread.UncaughtExceptionHandler previous = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((thread, error) -> uncaught.set(error));
        try {
            Throwable thrown = harness.runExpectingThrow();
            check(thrown instanceof IllegalStateException, "解码的意外异常在调用线程上抛出");
            check(harness.microphone.reads.get() < 1_000, "解码失败后不再继续录音");
            check(uncaught.get() == null, "异常不能走到未捕获异常处理");
        } finally {
            Thread.setDefaultUncaughtExceptionHandler(previous);
        }
    }

    /** 发送失败、服务端也没留下关闭帧：网络问题。 */
    private static void failedWriteWithoutServerReasonIsNetwork() throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        harness.onRead = index -> {
            if (index == 2) harness.connection.failWrites = true;
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check(outcome.failure() == HttpAsrRecognizer.Failure.NETWORK, "写失败算网络");
    }

    /** 分片的最终结果拼起来再解码，中文字节在分片边界上切开也不受影响。 */
    private static void fragmentedFinalIsReassembled() throws Exception {
        Harness harness = new Harness();
        harness.onRead = index -> {
            if (index == 2) harness.stopped.set(true);
        };
        byte[] whole = bytes("你好|1");
        byte[] head = java.util.Arrays.copyOfRange(whole, 0, 4);
        byte[] tail = java.util.Arrays.copyOfRange(whole, 4, whole.length);
        harness.connection.onLastAudio = () -> {
            harness.server.push(frame(WebSocketFrames.OPCODE_BINARY, head, false));
            harness.server.push(frame(WebSocketFrames.OPCODE_CONTINUATION, tail, true));
        };
        check("你好".equals(harness.run().text()), "分片拼回完整的最终结果");
    }

    /** 服务端在录音还没结束时就给了最终结果：放开麦克风收尾，不等用户。 */
    private static void earlyFinalStopsTheMicrophone() throws Exception {
        Harness harness = new Harness();
        harness.limit = 100_000;
        CountDownLatch delivered = new CountDownLatch(1);
        harness.listener = (text, last) -> delivered.countDown();
        harness.onRead = index -> {
            if (index == 2) harness.server.push(binary("提前|1"));
            if (index == 3) await(delivered, 5_000);
        };
        DoubaoStreamSession.Outcome outcome = harness.run();
        check("提前".equals(outcome.text()) && outcome.failure() == null, "提前的最终结果");
        check(harness.microphone.stopped && harness.microphone.reads.get() < 1_000, "不再继续录音");
    }

    /** 一次会话的假环境：服务端回包流、连接、麦克风和帧编解码。 */
    private static final class Harness {
        final ServerStream server = new ServerStream();
        final FakeConnection connection = new FakeConnection(server);
        final FakeMicrophone microphone = new FakeMicrophone();
        final AtomicBoolean stopped = new AtomicBoolean();
        final AtomicBoolean cancelled = new AtomicBoolean();
        volatile IntConsumer onRead = index -> { };
        DoubaoStreamSession.Listener listener;
        long limit = 100_000;
        long finalTimeoutMillis = 5_000;

        DoubaoStreamSession.Outcome run() throws InterruptedException {
            microphone.harness = this;
            DoubaoStreamSession session = new DoubaoStreamSession(server, connection, microphone, CODEC,
                listener, stopped, cancelled, limit, CHUNK, finalTimeoutMillis);
            AtomicReference<DoubaoStreamSession.Outcome> result = new AtomicReference<>();
            Thread runner = new Thread(() -> result.set(session.run()), "smoke-doubao-session");
            runner.setDaemon(true);
            runner.start();
            runner.join(15_000);
            check(!runner.isAlive(), "会话必须结束，不能挂住");
            check(result.get() != null, "会话给出结果");
            return result.get();
        }

        /** 跑一次会话，返回 run() 抛出的异常；没有抛出时失败。 */
        Throwable runExpectingThrow() throws InterruptedException {
            microphone.harness = this;
            DoubaoStreamSession session = new DoubaoStreamSession(server, connection, microphone, CODEC,
                listener, stopped, cancelled, limit, CHUNK, finalTimeoutMillis);
            AtomicReference<Throwable> thrown = new AtomicReference<>();
            Thread runner = new Thread(() -> {
                try {
                    session.run();
                } catch (RuntimeException | LinkageError error) {
                    thrown.set(error);
                }
            }, "smoke-doubao-session");
            runner.setDaemon(true);
            runner.start();
            runner.join(15_000);
            check(!runner.isAlive(), "会话必须结束，不能挂住");
            return thrown.get();
        }
    }

    /** 帧编解码：音频帧只带序号和是否最后一帧；回包是「文字|是否最终」，「ERR:错误码」是错误帧，ERR 是读不懂的回包，BOOM 让解码抛出意外异常。 */
    private static final DoubaoStreamSession.Codec CODEC = new DoubaoStreamSession.Codec() {
        @Override public byte[] audioFrame(int sequence, byte[] pcm, int length, boolean last) {
            return new byte[] {(byte) sequence, (byte) (last ? 1 : 0)};
        }

        @Override public DoubaoStreamSession.Update decode(byte[] payload) {
            String value = new String(payload, StandardCharsets.UTF_8);
            if (value.equals("BOOM")) throw new IllegalStateException("synthetic decoder failure");
            if (value.startsWith("ERR:")) {
                return DoubaoStreamSession.Update.failed(
                    VoiceFailurePolicy.doubaoError(Integer.valueOf(value.substring(4))));
            }
            int separator = value.lastIndexOf('|');
            if (separator < 0) return null;
            return new DoubaoStreamSession.Update(value.substring(0, separator),
                value.substring(separator + 1).equals("1"));
        }
    };

    private static final class FakeMicrophone implements DoubaoStreamSession.Microphone {
        final AtomicInteger reads = new AtomicInteger();
        volatile boolean stopped;
        Harness harness;

        @Override public int read(byte[] buffer, int length) {
            int index = reads.incrementAndGet();
            harness.onRead.accept(index);
            // 真麦克风每块要等约 100 ms 才读满；这里稍等一下，免得空转的采集循环把读线程挤得来不及收尾。
            try {
                Thread.sleep(2);
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
            }
            return length;
        }

        @Override public void stop() {
            stopped = true;
        }
    }

    private static final class FakeConnection implements DoubaoStreamSession.Connection {
        private final ServerStream server;
        final AtomicInteger lastAudioFrames = new AtomicInteger();
        final CountDownLatch pong = new CountDownLatch(1);
        final AtomicReference<String> pongPayload = new AtomicReference<>();
        volatile Runnable onLastAudio = () -> { };
        volatile boolean failWrites;
        private volatile boolean aborted;

        FakeConnection(ServerStream server) {
            this.server = server;
        }

        @Override public synchronized void send(int opcode, byte[] payload) throws IOException {
            if (aborted || failWrites) throw new IOException("synthetic closed connection");
            if (opcode == WebSocketFrames.OPCODE_PONG) {
                pongPayload.set(new String(payload, StandardCharsets.UTF_8));
                pong.countDown();
                return;
            }
            if (opcode == WebSocketFrames.OPCODE_BINARY && payload.length == 2 && payload[1] == 1) {
                lastAudioFrames.incrementAndGet();
                onLastAudio.run();
            }
        }

        @Override public void abort() {
            aborted = true;
            server.abort();
        }
    }

    /** 服务端回包；available() 永远是 0，和 Conscrypt 的 TLS socket 一样。 */
    private static final class ServerStream extends InputStream {
        private static final byte[] END = new byte[0];
        private static final byte[] CLOSED = new byte[0];
        private final LinkedBlockingQueue<byte[]> chunks = new LinkedBlockingQueue<>();
        private byte[] current;
        private int offset;

        void push(byte[] value) {
            chunks.add(value);
        }

        void eof() {
            chunks.add(END);
        }

        void abort() {
            chunks.add(CLOSED);
        }

        @Override public int available() {
            return 0;
        }

        @Override public int read() throws IOException {
            byte[] one = new byte[1];
            return read(one, 0, 1) < 0 ? -1 : one[0] & 0xff;
        }

        @Override public int read(byte[] buffer, int off, int length) throws IOException {
            while (current == null || offset == current.length) {
                try {
                    current = chunks.take();
                } catch (InterruptedException error) {
                    Thread.currentThread().interrupt();
                    throw new InterruptedIOException();
                }
                offset = 0;
                if (current == END) {
                    chunks.add(END);
                    current = null;
                    return -1;
                }
                if (current == CLOSED) {
                    chunks.add(CLOSED);
                    current = null;
                    throw new IOException("synthetic socket closed");
                }
            }
            int count = Math.min(length, current.length - offset);
            System.arraycopy(current, offset, buffer, off, count);
            offset += count;
            return count;
        }
    }

    private static byte[] binary(String value) {
        return frame(WebSocketFrames.OPCODE_BINARY, bytes(value), true);
    }

    /** 服务端帧不加掩码；载荷都小于 126 字节。 */
    private static byte[] frame(int opcode, byte[] payload, boolean fin) {
        byte[] frame = new byte[2 + payload.length];
        frame[0] = (byte) ((fin ? 0x80 : 0) | opcode);
        frame[1] = (byte) payload.length;
        System.arraycopy(payload, 0, frame, 2, payload.length);
        return frame;
    }

    private static byte[] bytes(String value) {
        return value.getBytes(StandardCharsets.UTF_8);
    }

    private static boolean await(CountDownLatch latch, long millis) {
        try {
            return latch.await(millis, TimeUnit.MILLISECONDS);
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            return false;
        }
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
