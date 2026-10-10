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
        check(summary.equals("java.lang.RuntimeException: first line"), "summary is the first line only: " + summary);
        check(record.contains("Caused by: java.lang.IllegalStateException: inner"), "causes are kept");
        check(record.contains("at app.msime.android.core.TelemetryHandlerSmoke.main("), "frames are kept");
        check(Telemetry.firstLine("a\r\nb").equals("a"), "CRLF first line");

        Path root = Files.createTempDirectory("msime-telemetry-");
        try {
            Path outside = Files.createDirectory(root.resolve("outside"));
            Path linked = root.resolve("telemetry-crashes");
            Files.createSymbolicLink(linked, outside);
            java.lang.reflect.Field crashDirectory = Telemetry.class
                .getDeclaredField("crashDirectory");
            crashDirectory.setAccessible(true);
            java.lang.reflect.Field sessionCrashRecord = Telemetry.class
                .getDeclaredField("sessionCrashRecord");
            sessionCrashRecord.setAccessible(true);
            java.lang.reflect.Method writeCrashRecord = Telemetry.class
                .getDeclaredMethod("writeCrashRecord", Throwable.class);
            writeCrashRecord.setAccessible(true);
            crashDirectory.set(null, linked.toFile());
            sessionCrashRecord.set(null, null);
            writeCrashRecord.invoke(null, new RuntimeException("synthetic"));
            try (Stream<Path> children = Files.list(outside)) {
                check(children.findAny().isEmpty(),
                    "crash records must not follow a telemetry directory symlink");
            }
        } finally {
            java.lang.reflect.Field crashDirectory = Telemetry.class
                .getDeclaredField("crashDirectory");
            crashDirectory.setAccessible(true);
            crashDirectory.set(null, null);
            java.lang.reflect.Field sessionCrashRecord = Telemetry.class
                .getDeclaredField("sessionCrashRecord");
            sessionCrashRecord.setAccessible(true);
            sessionCrashRecord.set(null, null);
            try (Stream<Path> paths = Files.walk(root)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception cleanupError) { throw new IllegalStateException(cleanupError); }
                });
            }
        }

        Path sessionRoot = Files.createTempDirectory("msime-telemetry-session-");
        try {
            Path outside = Files.createDirectory(sessionRoot.resolve("outside"));
            Path crashRoot = Files.createDirectory(sessionRoot.resolve("telemetry-crashes"));
            Path linkedParent = crashRoot.resolve("session-link");
            Files.createSymbolicLink(linkedParent, outside);
            java.lang.reflect.Field crashDirectory = Telemetry.class
                .getDeclaredField("crashDirectory");
            crashDirectory.setAccessible(true);
            java.lang.reflect.Field sessionCrashRecord = Telemetry.class
                .getDeclaredField("sessionCrashRecord");
            sessionCrashRecord.setAccessible(true);
            java.lang.reflect.Method writeCrashRecord = Telemetry.class
                .getDeclaredMethod("writeCrashRecord", Throwable.class);
            writeCrashRecord.setAccessible(true);
            crashDirectory.set(null, crashRoot.toFile());
            sessionCrashRecord.set(null, linkedParent.resolve("session.crash").toFile());
            writeCrashRecord.invoke(null, new RuntimeException("synthetic session"));
            try (Stream<Path> children = Files.list(outside)) {
                check(children.findAny().isEmpty(),
                    "session crash records must not follow a replaced parent symlink");
            }
        } finally {
            java.lang.reflect.Field crashDirectory = Telemetry.class
                .getDeclaredField("crashDirectory");
            crashDirectory.setAccessible(true);
            crashDirectory.set(null, null);
            java.lang.reflect.Field sessionCrashRecord = Telemetry.class
                .getDeclaredField("sessionCrashRecord");
            sessionCrashRecord.setAccessible(true);
            sessionCrashRecord.set(null, null);
            try (Stream<Path> paths = Files.walk(sessionRoot)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception cleanupError) { throw new IllegalStateException(cleanupError); }
                });
            }
        }
        System.out.println("Android telemetry crash handler and record limits passed");
    }
}
