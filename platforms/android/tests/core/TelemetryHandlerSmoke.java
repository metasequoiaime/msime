package app.msime.android.core;

import app.msime.android.JsonPolicy;
import app.msime.android.TextPolicy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.stream.Stream;

public final class TelemetryHandlerSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        check(!JsonPolicy.strictBoolean("true", false), "string consent must not enable telemetry");
        check(JsonPolicy.strictBoolean(Boolean.TRUE, false), "typed consent is accepted");
        Thread.UncaughtExceptionHandler previous = Thread.getDefaultUncaughtExceptionHandler();
        try {
            check(Telemetry.installCrashHandler(null), "first installation must succeed");
            Thread.UncaughtExceptionHandler installed = Thread.getDefaultUncaughtExceptionHandler();
            check(installed != previous, "installation must replace the prior handler");
            check(!Telemetry.installCrashHandler(null), "reinstallation must be ignored");
            check(Thread.getDefaultUncaughtExceptionHandler() == installed,
                "reinstallation must not wrap the handler again");
        } finally {
            Thread.setDefaultUncaughtExceptionHandler(previous);
        }

        // The server counts the message in Unicode scalars; cutting at a char index can split a surrogate pair and leave invalid UTF-16.
        String emoji = "😀".repeat(1001);
        String clipped = TextPolicy.clipCodePoints(emoji, Telemetry.MAX_MESSAGE_CODE_POINTS);
        check(clipped.codePointCount(0, clipped.length()) == 1000, "message is clipped to 1000 code points");
        check(!Character.isHighSurrogate(clipped.charAt(clipped.length() - 1)), "no split surrogate pair");
        check(TextPolicy.clipCodePoints("short", 1000).equals("short"), "short text is kept");

        StringBuilder frames = new StringBuilder();
        for (int index = 0; index < 2000; index++) frames.append("at app.msime.Frame").append(index).append("(Frame.java:1)\n");
        String stack = Telemetry.clipStack(frames.toString());
        check(stack.getBytes(StandardCharsets.UTF_8).length <= Telemetry.MAX_STACK_BYTES, "stack fits the byte cap");
        check(stack.endsWith("\n"), "stack is cut at a line boundary");
        check(Telemetry.clipStack("at a\n").equals("at a\n"), "a short stack is kept");

        IllegalStateException cause = new IllegalStateException("inner");
        RuntimeException error = new RuntimeException("first line\nsecond line", cause);
        String record = Telemetry.crashRecord(error);
        String summary = record.substring(0, record.indexOf('\n'));
        check(summary.equals("java.lang.RuntimeException"), "summary is the first line only: " + summary);
        check(record.contains("Caused by: java.lang.IllegalStateException"), "causes are kept");
        check(record.contains("at app.msime.android.core.TelemetryHandlerSmoke.main("), "frames are kept");
        check(Telemetry.firstLine("a\r\nb").equals("a"), "CRLF first line");

        Path root = Files.createTempDirectory("msime-telemetry-");
        java.lang.reflect.Field crashDirectory = Telemetry.class.getDeclaredField("crashDirectory");
        crashDirectory.setAccessible(true);
        java.lang.reflect.Method safe = Telemetry.class.getDeclaredMethod("isSafeSessionCrashRecord", java.io.File.class);
        safe.setAccessible(true);
        try {
            Path outside = Files.createDirectory(root.resolve("outside"));
            Path crashRoot = Files.createDirectory(root.resolve("telemetry-crashes"));
            crashDirectory.set(null, crashRoot.toFile());
            check((Boolean) safe.invoke(null, crashRoot.resolve("synthetic.crash").toFile()),
                "reserved record directly inside a real crash directory is accepted");
            check(!(Boolean) safe.invoke(null, outside.resolve("synthetic.crash").toFile()),
                "outside record is rejected");
            Path linkedRecord = crashRoot.resolve("linked.crash");
            Files.createSymbolicLink(linkedRecord, outside.resolve("record"));
            check(!(Boolean) safe.invoke(null, linkedRecord.toFile()), "linked record is rejected");
            Path linkedParent = crashRoot.resolve("session-link");
            Files.createSymbolicLink(linkedParent, outside);
            check(!(Boolean) safe.invoke(null, linkedParent.resolve("synthetic.crash").toFile()),
                "replaced session parent is rejected");
            Path linked = root.resolve("linked-crashes");
            Files.createSymbolicLink(linked, outside);
            crashDirectory.set(null, linked.toFile());
            check(!(Boolean) safe.invoke(null, linked.resolve("synthetic.crash").toFile()),
                "linked crash directory is rejected");
            try (Stream<Path> children = Files.list(outside)) {
                check(children.findAny().isEmpty(), "rejected paths leave the outside directory unchanged");
            }
        } finally {
            crashDirectory.set(null, null);
            try (Stream<Path> paths = Files.walk(root)) {
                for (Path path : (Iterable<Path>) paths.sorted(java.util.Comparator.reverseOrder())::iterator)
                    Files.deleteIfExists(path);
            }
        }

        // 故障线程即使碰到另一线程持有生命周期锁，也必须交还系统异常处理器。
        java.util.concurrent.CountDownLatch held = new java.util.concurrent.CountDownLatch(1);
        java.util.concurrent.CountDownLatch release = new java.util.concurrent.CountDownLatch(1);
        Thread holder = new Thread(() -> {
            synchronized (Telemetry.class) {
                held.countDown();
                try { release.await(); }
                catch (InterruptedException interrupted) { Thread.currentThread().interrupt(); }
            }
        });
        holder.start();
        java.util.concurrent.atomic.AtomicReference<Throwable> failure = new java.util.concurrent.atomic.AtomicReference<>();
        java.util.concurrent.CountDownLatch returned = new java.util.concurrent.CountDownLatch(1);
        Thread fatal = new Thread(() -> {
            try {
                java.lang.reflect.Method write = Telemetry.class.getDeclaredMethod("writeCrashRecord", Throwable.class);
                write.setAccessible(true);
                write.invoke(null, new RuntimeException("synthetic"));
            } catch (Throwable errorInTest) { failure.set(errorInTest); }
            finally { returned.countDown(); }
        });
        try {
            check(held.await(5, java.util.concurrent.TimeUnit.SECONDS), "lifecycle lock holder ready");
            fatal.start();
            check(returned.await(2, java.util.concurrent.TimeUnit.SECONDS), "fatal path does not wait for lifecycle lock");
            check(failure.get() == null, "inactive fatal path returns normally");
        } finally { release.countDown(); holder.join(5000); fatal.join(5000); }
        System.out.println("Android telemetry crash handler and record limits passed");
    }
}
