package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * AI 候选座位（#6888）的设备验收：开着 AI 时组字，候选条上没有占位行；AI 回答为空时候选一个都不变，同一组字也不再向 AI 发请求；回答到达时答案插在第一页最后一格（这里每页 6 个，第 6 位），前 5 位不挪位。
 *
 * <p>AI 接口是本进程里的一个 OpenAI 兼容桩服务，只监听回环地址：本套件以 `app.msime.android` 为目标运行在应用的主进程里，输入法的 `:ime` 进程与它同一个 UID，`http://127.0.0.1:<端口>` 是 AI 设置允许的本机地址（`AiEndpointPolicy`）。桩服务收到请求后等测试给出回答再回复，所以回答什么时候到由测试决定，不靠网络快慢。密钥是合成值，请求不出这台模拟器。
 */
public final class AiCandidateSeatDeviceSmoke extends DeviceSmoke {
    /** 共享内核以前在 AI 座位上放的占位行文字，现在不应再出现。 */
    private static final String WAITING = "…";
    private static final String RESOLVED = "—";
    /** 本套件把每页候选数定成 6：答案坐第一页最后一格，下标 5。 */
    private static final int PAGE_SIZE = 6;
    /** 桩服务给出的 AI 候选：不是词库里的词，免得它本来就在列表里而被引擎跳过。 */
    private static final String ANSWER = "倪浩答案";

    @Override protected String successDescription() {
        return "no AI placeholder, an empty answer changes nothing and is not asked again, and a real answer takes the last first-page slot without moving the earlier candidates";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try (StubAiServer server = new StubAiServer("\\\"hao\\\"")) {
            String origin = "http://127.0.0.1:" + server.port();
            JSONObject assistant = new JSONObject().put("enabled", true).put("provider", "custom")
                .put("model", "msime-test-model").put("endpoint", origin + "/v1/chat/completions")
                .put("tokens", new JSONObject().put(origin, "msime-test-token")).put("candidate_limit", 1);
            JSONObject snapshot = new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", new JSONObject(options.getJSONObject("preferences").toString())
                    .put("ai_assistant", assistant).put("cloud_candidates", false).put("learning", false)
                    .put("candidate_page_size", PAGE_SIZE));
            try {
                stage = "AI preferences";
                publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
                shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                stage = "editor focus";
                tap(field("msime-test-plain"));
                await(field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));

                // 第一次组字：回答为空。
                stage = "no placeholder while waiting";
                typePhrase();
                stage = "AI request sent";
                server.awaitRequest();
                List<String> before = strip();
                if (before.isEmpty() || before.contains(WAITING) || before.contains(RESOLVED))
                    throw new AssertionError("Unexpected candidates while waiting for AI: " + before);

                stage = "empty answer changes nothing";
                server.answer(new JSONArray());
                SystemClock.sleep(1500);
                List<String> after = strip();
                if (!before.equals(after))
                    throw new AssertionError("Candidates changed when the answer was empty: " + before + " -> " + after);

                // 同一组字问过 AI 而没有结果，之后的重画（释义到达、每秒的设置轮询）不再向 AI 发请求。
                stage = "no request loop after an empty answer";
                server.answerImmediately(new JSONArray());
                int requests = server.requests();
                SystemClock.sleep(3000);
                if (server.requests() != requests)
                    throw new AssertionError("Host kept asking AI for the same composition: "
                        + (server.requests() - requests) + " more requests in 3 s");
                after = strip();
                if (!before.equals(after))
                    throw new AssertionError("Candidates changed after an empty answer: " + before + " -> " + after);

                stage = "commit after an empty answer";
                tap(key("空格"));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && !node.getText().toString().contains("nihao")));

                // 第二次组字：回答到达，答案插在第一页最后一格，前面的候选不动。
                stage = "candidates before an answer";
                server.holdNext();
                int committed = fieldText().length();
                typePhrase();
                server.awaitRequest();
                before = strip();
                if (before.isEmpty() || before.contains(WAITING))
                    throw new AssertionError("Unexpected candidates while waiting for AI: " + before);
                int seat = Math.min(PAGE_SIZE - 1, before.size());
                stage = "answer takes the last first-page slot";
                server.answer(new JSONArray().put(new JSONObject().put("text", ANSWER)));
                await(candidateText(ANSWER));
                after = strip();
                if (!seatedAt(before, after, seat, ANSWER))
                    throw new AssertionError("Answer did not take slot " + (seat + 1) + " without moving earlier candidates: "
                        + before + " -> " + after);
                stage = "answer commits";
                tap(candidateText(ANSWER));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && node.getText().length() > committed
                    && node.getText().toString().endsWith(ANSWER)));
            } catch (Exception | AssertionError error) {
                shellDiscard("screencap -p /data/local/tmp/msime-ai-seat-failure.png");
                throw error;
            } finally {
                shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
                if (original == null) Files.deleteIfExists(preferences.toPath());
                else publish(preferences, original);
            }
        }
    }

    private void typePhrase() throws Exception {
        String prefix = stage;
        for (String key : new String[] {"n", "i", "h", "a", "o"}) { stage = prefix + ": " + key; tap(key(key)); }
        stage = prefix;
    }

    private String fieldText() {
        AccessibilityNodeInfo editor = await(field("msime-test-plain"));
        return editor.getText() == null ? "" : editor.getText().toString();
    }

    /** 候选条上画着这个文字的候选按钮（第一行是候选字，下面可能有释义）。 */
    private Predicate<AccessibilityNodeInfo> candidateText(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("android.widget.Button", node.getClassName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 ")
            && node.getText() != null && firstLine(node.getText()).equals(text);
    }

    /** 候选条第一页上的候选字，按「候选 N：」排好；空位是 null。 */
    private List<String> strip() {
        List<String> slots = new ArrayList<>();
        for (int index = 0; index < PAGE_SIZE; index++) slots.add(null);
        for (AccessibilityWindowInfo window : automation.getWindows())
            collect(window.getRoot(), slots);
        while (!slots.isEmpty() && slots.get(slots.size() - 1) == null) slots.remove(slots.size() - 1);
        return slots;
    }

    private void collect(AccessibilityNodeInfo node, List<String> slots) {
        if (node == null) return;
        CharSequence description = node.getContentDescription();
        if (equalsText("app.msime.android", node.getPackageName()) && description != null
                && node.getText() != null) {
            String value = description.toString();
            int colon = value.indexOf('：');
            if (value.startsWith("候选 ") && colon > 3) {
                try {
                    int slot = Integer.parseInt(value.substring(3, colon)) - 1;
                    if (slot >= 0 && slot < slots.size() && slots.get(slot) == null)
                        slots.set(slot, firstLine(node.getText()));
                } catch (NumberFormatException ignored) {
                    // 「候选面板 删除」这类控件不是候选。
                }
            }
        }
        for (int index = 0; index < node.getChildCount(); index++) collect(node.getChild(index), slots);
    }

    private static String firstLine(CharSequence text) {
        String value = text.toString();
        int newline = value.indexOf('\n');
        return newline < 0 ? value : value.substring(0, newline);
    }

    /** 答案坐在 `seat`，它前面的候选都没变。 */
    private static boolean seatedAt(List<String> before, List<String> after, int seat, String seated) {
        if (after.size() <= seat || !seated.equals(after.get(seat))) return false;
        for (int index = 0; index < seat; index++) {
            String expected = before.get(index);
            if (expected == null || !expected.equals(after.get(index))) return false;
        }
        return true;
    }

    /**
     * 只监听回环地址的 OpenAI 兼容桩服务，每个连接一个线程。
     *
     * <p>逐键点按比宿主 350 ms 的安静间隔慢，打 `nihao` 的途中 `niha` 也会问一次 AI；这些过期的请求立刻得到空回答（宿主按代次丢掉它们），只有问的是完整组字（请求正文里有 {@link #marker}）的请求才算数：它挂着等测试经 {@link #answer} 给出候选，最多 8 秒（宿主的读超时），超时就断开，宿主当作请求失败。
     */
    private static final class StubAiServer implements AutoCloseable {
        private final ServerSocket socket;
        private final String marker;
        private final AtomicInteger requests = new AtomicInteger();
        private final BlockingQueue<Integer> received = new LinkedBlockingQueue<>();
        private final BlockingQueue<JSONArray> answers = new LinkedBlockingQueue<>();
        private volatile JSONArray immediate;

        StubAiServer(String marker) throws IOException {
            this.marker = marker;
            socket = new ServerSocket(0, 8, InetAddress.getByName("127.0.0.1"));
            Thread thread = new Thread(this::serve, "msime-test-ai-stub");
            thread.setDaemon(true);
            thread.start();
        }

        int port() { return socket.getLocalPort(); }

        /** 问完整组字的请求一共收到了几个。 */
        int requests() { return requests.get(); }

        void awaitRequest() throws InterruptedException {
            if (received.poll(15, TimeUnit.SECONDS) == null)
                throw new AssertionError("The keyboard never asked the AI endpoint about the composition");
        }

        void answer(JSONArray candidates) { answers.add(candidates); }

        /** 之后的请求不等测试，立刻用这些候选回答。 */
        void answerImmediately(JSONArray candidates) { immediate = candidates; }

        /** 之后的请求重新挂着，等测试给回答；之前没用掉的回答和请求记录一并丢掉。 */
        void holdNext() {
            immediate = null;
            answers.clear();
            received.clear();
        }

        private void serve() {
            while (!socket.isClosed()) {
                try {
                    Socket client = socket.accept();
                    Thread thread = new Thread(() -> respond(client), "msime-test-ai-stub-client");
                    thread.setDaemon(true);
                    thread.start();
                } catch (IOException ignored) {
                    // socket 关闭时 accept 抛出，循环随之结束。
                }
            }
        }

        private void respond(Socket client) {
            try (client) {
                client.setSoTimeout(10000);
                String request = readRequest(client.getInputStream());
                if (request == null) return;
                JSONArray candidates = new JSONArray();
                if (request.contains(marker)) {
                    requests.incrementAndGet();
                    received.add(requests.get());
                    candidates = immediate;
                    if (candidates == null) candidates = answers.poll(8, TimeUnit.SECONDS);
                    if (candidates == null) return;
                }
                String content = new JSONObject().put("candidates", candidates).toString();
                byte[] body = new JSONObject().put("choices", new JSONArray().put(new JSONObject()
                    .put("index", 0).put("message", new JSONObject().put("role", "assistant")
                        .put("content", content)))).toString().getBytes(StandardCharsets.UTF_8);
                OutputStream output = client.getOutputStream();
                output.write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                    + body.length + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
                output.write(body);
                output.flush();
            } catch (Exception ignored) {
                // 宿主断开连接（换了组字、超时）：这个请求就此作罢。
            }
        }

        /** 读完请求头和 Content-Length 指明的正文，返回正文；不是 chat completions 的 POST 返回 null。 */
        private static String readRequest(InputStream input) throws IOException {
            ByteArrayOutputStream head = new ByteArrayOutputStream();
            int matched = 0;
            while (matched < 4) {
                int next = input.read();
                if (next < 0) return null;
                head.write(next);
                matched = next == "\r\n\r\n".charAt(matched) ? matched + 1 : (next == '\r' ? 1 : 0);
                if (head.size() > 16384) return null;
            }
            String header = head.toString(StandardCharsets.US_ASCII.name());
            int length = 0;
            for (String line : header.split("\r\n")) {
                if (line.toLowerCase(Locale.ROOT).startsWith("content-length:"))
                    length = Integer.parseInt(line.substring("content-length:".length()).trim());
            }
            if (length < 0 || length > 65536) return null;
            byte[] body = new byte[length];
            int read = 0;
            while (read < length) {
                int count = input.read(body, read, length - read);
                if (count < 0) return null;
                read += count;
            }
            return header.startsWith("POST /v1/chat/completions ")
                ? new String(body, StandardCharsets.UTF_8) : null;
        }

        @Override public void close() throws IOException {
            socket.close();
        }
    }
}
