package app.msime.android;

import java.util.Arrays;
import java.util.Random;

/** 中文数字页的临时排列：离开页面时不计算，重新进入沿用上次键序。 */
public final class RandomDigitOrderPolicy {
    private final Random random;
    private final int[] digits = {1, 2, 3, 4, 5, 6, 7, 8, 9, 0};
    private boolean enabled;
    private boolean prepared;

    public RandomDigitOrderPolicy() { this(new Random()); }
    RandomDigitOrderPolicy(Random random) { this.random = random; }

    public boolean enabled() { return enabled; }

    public void setEnabled(boolean value) {
        if (enabled == value) return;
        enabled = value;
        prepared = false;
    }

    public int digit(int slot, boolean eligible) {
        if (slot < 0 || slot >= digits.length) throw new IllegalArgumentException("Invalid digit slot");
        if (!enabled || !eligible) return (slot + 1) % 10;
        if (!prepared) shuffle();
        return digits[slot];
    }

    public void afterDigit(boolean eligible) {
        if (enabled && eligible) shuffle();
    }

    private void shuffle() {
        int[] previous = digits.clone();
        for (int i = digits.length - 1; i > 0; i--) {
            int other = random.nextInt(i + 1);
            int temporary = digits[i];
            digits[i] = digits[other];
            digits[other] = temporary;
        }
        // 即使随机数恰好给出原排列，这次点击也要产生新的键序。
        if (Arrays.equals(previous, digits)) {
            int first = digits[0];
            digits[0] = digits[1];
            digits[1] = first;
        }
        prepared = true;
    }
}
