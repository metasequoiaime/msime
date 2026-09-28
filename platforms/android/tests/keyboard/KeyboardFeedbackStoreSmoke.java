package app.msime.client;

import app.msime.client.KeyboardFeedbackPreferences.HapticStrength;
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
