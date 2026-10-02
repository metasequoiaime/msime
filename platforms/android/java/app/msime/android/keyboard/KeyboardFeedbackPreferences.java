package app.msime.android;

/** Persisted Android equivalents of Apple's keyboard sound and haptic settings. */
public final class KeyboardFeedbackPreferences {
    public enum HapticStrength {
        LIGHT("light", 64), MEDIUM("medium", 160), STRONG("strong", 255);

        private final String id;
        private final int amplitude;

        HapticStrength(String id, int amplitude) {
            this.id = id;
            this.amplitude = amplitude;
        }

        public String id() { return id; }
        public int amplitude() { return amplitude; }
    }

    private KeyboardFeedbackPreferences() {}

    public static HapticStrength strength(String value) {
        for (HapticStrength strength : HapticStrength.values())
            if (strength.id().equals(value)) return strength;
        return HapticStrength.MEDIUM;
    }
}
