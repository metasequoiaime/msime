package app.msime.android.core;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import java.util.stream.Stream;

/** 同一套真实文件操作既在 JVM 运行，也由设备 instrumentation 在应用 UID 下执行。 */
public final class DiagnosticConsentSmoke {
    private DiagnosticConsentSmoke() {}

    private static void check(boolean accepted, String message) {
        if (!accepted) throw new AssertionError(message);
    }

    private static byte[] bytes(String summary) { return summary.getBytes(StandardCharsets.US_ASCII); }

    public static void run(Path root) throws Exception {
        String capture = UUID.randomUUID().toString();
        String[] old = new String[1];
        DiagnosticConsent.locked(root.toFile(), consent -> {
            check(consent.current() == null, "missing consent cannot claim history");
            DiagnosticConsent.State state = consent.observe(true, 1000);
            old[0] = state.summary(capture, "0.3.0", "keyboard");
            DiagnosticConsent.Identity identity = state.accept(bytes(old[0]), 1100, 1200);
            check(identity != null && identity.capture.equals(capture) && identity.version.equals("0.3.0")
                && identity.component.equals("keyboard"), "current epoch preserves original identity");
            check(state.accept(bytes(old[0]), 999, 1200) == null, "past exit rejected");
            check(state.accept(bytes(old[0]), 1201, 1200) == null, "future exit rejected");
            check(state.accept(bytes("v1|" + capture + "|0.3.0|keyboard"), 1100, 1200) == null,
                "legacy summary has no durable consent");
            check(state.summary(capture, "v".repeat(64), "application") == null, "oversized summary fails closed");
            check(state.summary("1-1-1-1-1", "0.3.0", "keyboard") == null, "short UUID rejected");
            check(state.summary(capture, "0.3.0|extra", "keyboard") == null, "summary delimiter rejected");
            check(state.summary(capture, "0.3.0", "other") == null, "foreign component rejected");
        });
        // 重新打开文件代表另一个启动；同意没有中断时保留原 epoch 和原版本。
        DiagnosticConsent.locked(root.toFile(), consent -> {
            check(consent.observe(true, 1500).accept(bytes(old[0]), 1100, 1600) != null, "restart retains consent");
            consent.observe(false, 1700);
            check(consent.current() == null, "withdrawal persists before queue clear");
        });
        DiagnosticConsent.locked(root.toFile(), consent -> {
            check(consent.current() == null, "withdrawal survives another open");
            // 即使时钟回拨，旧 epoch 仍不能被重新开启后的同意追认。
            DiagnosticConsent.State next = consent.observe(true, 900);
            check(next.accept(bytes(old[0]), 1100, 2000) == null, "re-enable cannot resurrect old history");
            check(next.accept(bytes(next.summary(capture, "0.4.0", "application")), 1000, 2000) != null,
                "new epoch can report a new exit");
            consent.observe(false, 2100);
        });
        Path ledger = root.resolve("telemetry-diagnostic-consent");
        Files.write(ledger, new byte[65]);
        try {
            DiagnosticConsent.locked(root.toFile(), consent -> consent.observe(true, 3000));
            throw new AssertionError("corrupt consent accepted");
        } catch (java.io.IOException expected) { /* 损坏文件不恢复成开启状态。 */ }
        Files.delete(ledger);
        Path outside = Files.createFile(root.resolve("synthetic-outside"));
        Files.createSymbolicLink(ledger, outside);
        try {
            DiagnosticConsent.locked(root.toFile(), consent -> consent.observe(true, 3000));
            throw new AssertionError("linked consent accepted");
        } catch (java.io.IOException expected) { /* 不跟随应用私有文件的链接。 */ }
        check(Files.size(outside) == 0, "linked target unchanged");
        Files.delete(ledger);
        Files.delete(outside);
    }

    public static void main(String[] args) throws Exception {
        Path root = Files.createTempDirectory("msime-diagnostic-consent-");
        try { run(root); }
        finally {
            try (Stream<Path> paths = Files.walk(root)) {
                for (Path path : (Iterable<Path>) paths.sorted(java.util.Comparator.reverseOrder())::iterator)
                    Files.deleteIfExists(path);
            }
        }
        System.out.println("Android durable diagnostic consent passed");
    }
}
