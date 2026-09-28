package app.msime.client;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;

public final class BootstrapMarkerSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        Path exact = Files.createTempFile("bootstrap-marker", ".txt");
        Path oversized = Files.createTempFile("bootstrap-marker", ".txt");
        try {
            String stamp = "1234567890123";
            Files.write(exact, stamp.getBytes(StandardCharsets.UTF_8));
            Files.write(oversized, new byte[65]);
            check(stamp.equals(Bootstrap.readMarker(exact)));
            check(Bootstrap.readMarker(oversized) == null);
        } finally {
            Files.deleteIfExists(exact);
            Files.deleteIfExists(oversized);
        }
        System.out.println("Android bootstrap marker bounds passed");
    }
}
