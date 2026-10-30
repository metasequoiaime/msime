package app.msime.android;

import java.util.Arrays;
import java.util.Random;

/** 校验排列完整性、输入后换序和页面外不消耗随机数。 */
public final class RandomDigitOrderPolicySmoke {
    private static final class CountedRandom extends Random {
        private static final long serialVersionUID = 1L;
        int calls;
        CountedRandom() { super(10254); }
        @Override public int nextInt(int bound) { calls++; return super.nextInt(bound); }
    }

    public static void main(String[] args) {
        CountedRandom random = new CountedRandom();
        RandomDigitOrderPolicy policy = new RandomDigitOrderPolicy(random);
        int[] standard = {1, 2, 3, 4, 5, 6, 7, 8, 9, 0};
        check(!policy.enabled() && Arrays.equals(order(policy, true), standard), "默认不重排");
        policy.setEnabled(true);
        check(random.calls == 0, "字母页启用时不计算键序");
        check(Arrays.equals(order(policy, false), standard), "英文和字母页保留标准排列");
        policy.afterDigit(false);
        check(random.calls == 0, "页面外数字输入不取随机数");
        int[] previous = order(policy, true);
        for (int round = 0; round < 1000; round++) {
            boolean[] seen = new boolean[10];
            for (int digit : previous) {
                check(digit >= 0 && digit < 10 && !seen[digit], "十个数字完整且无重复");
                seen[digit] = true;
            }
            int calls = random.calls;
            check(Arrays.equals(previous, order(policy, true)) && calls == random.calls, "普通重画不换序");
            order(policy, false);
            policy.afterDigit(false);
            check(random.calls == calls && policy.enabled(), "离开数字页不计算也不关闭");
            check(Arrays.equals(previous, order(policy, true)), "返回数字页沿用上次键序");
            policy.afterDigit(true);
            int[] next = order(policy, true);
            check(!Arrays.equals(previous, next), "每次数字输入后换序");
            previous = next;
        }
        int calls = random.calls;
        policy.setEnabled(false);
        check(Arrays.equals(order(policy, true), standard) && random.calls == calls, "关闭立即恢复且不计算");
        policy.afterDigit(true);
        check(random.calls == calls, "关闭后输入不计算");
        RandomDigitOrderPolicy unchangedShuffle = new RandomDigitOrderPolicy(new Random(10254) {
            private static final long serialVersionUID = 1L;
            @Override public int nextInt(int bound) { return bound - 1; }
        });
        unchangedShuffle.setEnabled(true);
        int[] first = order(unchangedShuffle, true);
        check(!Arrays.equals(first, standard), "随机源给出恒等排列时首次仍换序");
        unchangedShuffle.afterDigit(true);
        check(!Arrays.equals(first, order(unchangedShuffle, true)), "恒等排列不能使输入后的换序失效");
        System.out.println("RandomDigitOrderPolicySmoke ok");
    }

    private static int[] order(RandomDigitOrderPolicy policy, boolean eligible) {
        int[] result = new int[10];
        for (int slot = 0; slot < result.length; slot++) result[slot] = policy.digit(slot, eligible);
        return result;
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
