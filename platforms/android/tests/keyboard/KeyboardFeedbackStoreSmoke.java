package app.msime.android;

import app.msime.android.KeyboardFeedbackPreferences.HapticStrength;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

public final class KeyboardFeedbackStoreSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        KeyboardFeedbackStore.Settings settings = new KeyboardFeedbackStore.Settings(
            false, true, HapticStrength.STRONG);
        check(!settings.soundEnabled());
        check(settings.hapticsEnabled());
        check(settings.hapticStrength() == HapticStrength.STRONG);

        KeyboardFeedbackStore.Settings defaults = new KeyboardFeedbackStore.Settings(
            true, false, null);
        check(defaults.soundEnabled());
        check(!defaults.hapticsEnabled());
        check(defaults.hapticStrength() == HapticStrength.MEDIUM);
        try {
            Path root = Files.createTempDirectory("keyboard-feedback-root");
            Path outside = Files.createTempDirectory("keyboard-feedback-outside");
            try {
                Files.createDirectories(outside.resolve("state"));
                Files.createSymbolicLink(root.resolve("bootstrap"), outside);
                boolean rejected = false;
                try {
                    KeyboardFeedbackStore.ensureSafeDirectory(root.resolve("bootstrap/state"));
                } catch (IOException expected) {
                    rejected = true;
                }
                check(rejected);
                check(!Files.exists(outside.resolve("state/keyboard-feedback.json")));
            } finally {
                Files.deleteIfExists(root.resolve("bootstrap"));
                Files.deleteIfExists(root);
                Files.deleteIfExists(outside.resolve("state/keyboard-feedback.json"));
                Files.deleteIfExists(outside.resolve("state"));
                Files.deleteIfExists(outside);
            }
        } catch (IOException error) {
            throw new AssertionError(error);
        }
        try {
            Path exact = Files.createTempFile("keyboard-feedback", ".json");
            Path oversized = Files.createTempFile("keyboard-feedback", ".json");
            try {
                Files.write(exact, new byte[4096]);
                Files.write(oversized, new byte[4097]);
                check(KeyboardFeedbackFileReader.read(exact).length == 4096);
                try {
                    KeyboardFeedbackFileReader.read(oversized);
                    throw new AssertionError("oversized feedback file accepted");
                } catch (java.io.IOException expected) {
                    // Expected: the reader must stop at the configured limit.
                }
                Path external = Files.createTempFile("keyboard-feedback-external", ".json");
                Path link = Files.createTempFile("keyboard-feedback-link", ".json");
                try {
                    Files.writeString(external, "synthetic");
                    Files.delete(link);
                    Files.createSymbolicLink(link, external);
                    check(Files.isRegularFile(link));
                    try {
                        KeyboardFeedbackFileReader.read(link);
                        throw new AssertionError("symlinked feedback file accepted");
                    } catch (java.io.IOException expected) {
                        // Expected: feedback paths must not follow links.
                    }
                } finally {
                    Files.deleteIfExists(link);
                    Files.deleteIfExists(external);
                }
            } finally {
                Files.deleteIfExists(exact);
                Files.deleteIfExists(oversized);
            }
        } catch (java.io.IOException error) {
            throw new AssertionError(error);
        }
        System.out.println("Android keyboard feedback: shared settings value contract passed");
    }
}
