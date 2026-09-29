import app.msime.android.KeyboardFeedbackPreferences;
import app.msime.android.KeyboardFeedbackPreferences.HapticStrength;

public final class KeyboardFeedbackSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(KeyboardFeedbackPreferences.SOUND_KEY.equals("keyboardSoundEnabled"));
        check(KeyboardFeedbackPreferences.HAPTICS_KEY.equals("keyboardHapticsEnabled"));
        check(KeyboardFeedbackPreferences.STRENGTH_KEY.equals("keyboardHapticStrength"));
        check(KeyboardFeedbackPreferences.strength("light") == HapticStrength.LIGHT);
        check(KeyboardFeedbackPreferences.strength("medium") == HapticStrength.MEDIUM);
        check(KeyboardFeedbackPreferences.strength("strong") == HapticStrength.STRONG);
        check(KeyboardFeedbackPreferences.strength("invalid") == HapticStrength.MEDIUM);
        check(HapticStrength.LIGHT.amplitude() < HapticStrength.MEDIUM.amplitude());
        check(HapticStrength.MEDIUM.amplitude() < HapticStrength.STRONG.amplitude());
        System.out.println("Android keyboard feedback: keys, strength normalization and amplitudes passed");
    }
}
