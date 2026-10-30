import app.msime.android.TouchPointPolicy;

public final class TouchPointPolicySmoke {
    public static void main(String[] args) {
        int shuangpin = 1;
        check(TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", 'f', false, 3), "a 26-key double-pinyin letter carries its touch-down");
        check(TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", ';', false, 3), "the Microsoft ing key carries its touch-down");
        check(!TouchPointPolicy.usesTouch(true, 0, "none", false, "none", 'f', false, 3), "quanpin keeps msime_client_character");
        check(!TouchPointPolicy.usesTouch(true, 2, "none", false, "none", 'f', false, 3), "wubi keeps msime_client_character");
        check(!TouchPointPolicy.usesTouch(false, shuangpin, "none", false, "none", 'f', false, 3), "symbol, nine-key and other surfaces do not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "fourteen_key", false, "none", 'f', false, 3), "14-key grids do not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "eighteen_key", false, "none", 'f', false, 3), "18-key grids do not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", true, "none", 'f', false, 3), "dedicated English does not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "url", 'f', false, 3), "a local mode does not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", 'F', true, 3), "a Shift (helpcode) capital does not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", 'f', true, 3), "any Shift keeps the plain path");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", '\'', false, 3), "the separator does not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", '1', false, 3), "digits do not");
        check(!TouchPointPolicy.usesTouch(true, shuangpin, "none", false, "none", 'f', false, 0), "without a registered geometry nothing is sent");

        TouchPointPolicy points = new TouchPointPolicy();
        Object f = "f";
        Object g = "g";
        float[] point = new float[2];
        check(!points.take(f, 100, point), "a key never pressed has no touch-down");
        points.record(f, 10.5f, 20.25f, 100);
        check(points.take(f, 150, point) && point[0] == 10.5f && point[1] == 20.25f, "the click takes the key's own touch-down");
        check(!points.take(f, 160, point), "taking clears it: a second click (accessibility) gets nothing");
        points.record(f, 1, 2, 100);
        points.record(g, 3, 4, 110);
        check(points.take(g, 120, point) && point[0] == 3, "rollover: the later finger's key keeps its own point");
        check(points.take(f, 130, point) && point[0] == 1, "rollover: the earlier finger's key is not overwritten");
        points.record(f, 1, 2, 100);
        points.record(f, 5, 6, 200);
        check(points.take(f, 210, point) && point[0] == 5, "a new press on the same key replaces the old one");
        points.record(f, 1, 2, 100);
        check(!points.take(f, 100 + TouchPointPolicy.MAX_AGE_MS + 1, point), "a stale touch-down is dropped");
        check(!points.take(f, 100, point), "and it is cleared by the attempt");
        points.record(f, 1, 2, 100);
        check(points.take(f, 100 + TouchPointPolicy.MAX_AGE_MS, point), "exactly the maximum age still counts");
        points.record(f, 1, 2, 100);
        check(!points.take(f, 99, point), "a clock going backwards is not trusted");
        points.record(f, Float.NaN, 2, 100);
        check(!points.take(f, 100, point), "a non-finite coordinate is never sent");
        points.record(f, 1, 2, 100);
        points.forget(f);
        check(!points.take(f, 100, point), "a swallowed gesture forgets the key");
        points.record(f, 1, 2, 100);
        points.record(g, 1, 2, 100);
        points.clear();
        check(!points.take(f, 100, point) && !points.take(g, 100, point), "a cancelled touch clears every key");
        Object[] many = new Object[TouchPointPolicy.SLOTS + 1];
        for (int key = 0; key < many.length; key++) {
            many[key] = new Object();
            points.record(many[key], key, key, 100);
        }
        check(!points.take(many[0], 100, point), "the oldest key gives way when more keys than slots are held");
        check(points.take(many[TouchPointPolicy.SLOTS], 100, point) && point[0] == TouchPointPolicy.SLOTS, "the newest key is kept");
        check(points.take(many[1], 100, point) && point[0] == 1, "keys are matched by identity, not by equality");
        check(!points.take(null, 100, point), "a null key never matches");

        // 槽位满时让出的是最早按下的那个键，而不是轮转游标恰好指着的槽：空位被重新用上时游标不动，新按下的键会落在游标上。
        TouchPointPolicy held = new TouchPointPolicy();
        Object[] stale = new Object[TouchPointPolicy.SLOTS];
        for (int key = 0; key < stale.length; key++) {
            stale[key] = new Object();
            held.record(stale[key], key, key, key);
        }
        held.take(stale[0], 20, point);
        Object a = new Object();
        Object b = new Object();
        held.record(a, 1, 1, 100);
        held.record(b, 2, 2, 110);
        check(held.take(b, 120, point) && point[0] == 2, "the second finger keeps its touch-down");
        check(held.take(a, 130, point) && point[0] == 1, "a held letter in a reused slot is not the one evicted");
        check(!held.take(stale[1], 130, point), "the oldest record gave way instead");

        // 数字行、Shift、退格这类永远不取按下点的 12 个键先占满槽位，之后 200 次快速两指连按（每次之后一半的机会按一下这些键）不丢任何一个字母的按下点。种子固定，旧的轮转游标在这串按键里会丢掉两个按着的字母。
        TouchPointPolicy rollover = new TouchPointPolicy();
        java.util.Random presses = new java.util.Random(0);
        long now = 0;
        Object[] otherKeys = new Object[12];
        for (int key = 0; key < otherKeys.length; key++) {
            otherKeys[key] = new Object();
            rollover.record(otherKeys[key], key, key, now++);
        }
        Object[] letterKeys = new Object[26];
        for (int key = 0; key < letterKeys.length; key++) letterKeys[key] = new Object();
        for (int round = 0; round < 200; round++) {
            int firstLetter = presses.nextInt(26);
            Object first = letterKeys[firstLetter];
            Object second = letterKeys[(firstLetter + 1 + presses.nextInt(25)) % 26];
            rollover.record(first, round, 1, now += 30);
            rollover.record(second, round, 2, now += 30);
            check(rollover.take(first, now += 30, point) && point[0] == round && point[1] == 1, "rollover " + round + ": the held letter keeps its touch-down");
            check(rollover.take(second, now += 30, point) && point[0] == round && point[1] == 2, "rollover " + round + ": the second letter keeps its touch-down");
            if (presses.nextInt(2) == 0) rollover.record(otherKeys[presses.nextInt(otherKeys.length)], 0, 0, now += 30);
        }

        check(TouchPointPolicy.releasedOnKey(-7.9f, 50, 100, 120, 8), "lifting within the slop is still on the key");
        check(!TouchPointPolicy.releasedOnKey(-8.1f, 50, 100, 120, 8), "lifting beyond the slop is off the key");
        check(!TouchPointPolicy.releasedOnKey(50, 128, 100, 120, 8), "the far edge is exclusive");

        float[] letters = new float[26 * 4];
        for (int letter = 0; letter < 26; letter++) {
            letters[letter * 4] = letter * 10;
            letters[letter * 4 + 1] = 0;
            letters[letter * 4 + 2] = letter * 10 + 8;
            letters[letter * 4 + 3] = 20;
        }
        String json = TouchPointPolicy.geometryJson(7, letters, null, new char[0], new float[0], 0);
        check(json.startsWith("{\"id\":7,\"keys\":[[4.0,10.0,8.0,20.0],[14.0,10.0,8.0,20.0],"), json);
        check(json.endsWith("[254.0,10.0,8.0,20.0]],\"semicolon\":null,\"duplicates\":[]}"), json);
        String split = TouchPointPolicy.geometryJson(8, letters, new float[] {300, 0, 310, 20},
            new char[] {'g', 'v', 'x'}, new float[] {400, 30, 410, 50, 420, 60, 430, 80}, 2);
        check(split.contains("\"semicolon\":[305.0,10.0,10.0,20.0]"), split);
        check(split.endsWith("\"duplicates\":[[\"g\",405.0,40.0,10.0,20.0],[\"v\",425.0,70.0,10.0,20.0]]}"), split);

        expectFailure(() -> TouchPointPolicy.geometryJson(0, letters, null, new char[0], new float[0], 0), "id 0 clears and is never a registration");
        expectFailure(() -> TouchPointPolicy.geometryJson(1, new float[100], null, new char[0], new float[0], 0), "fewer than 26 keys");
        float[] empty = letters.clone();
        empty[2] = empty[0];
        expectFailure(() -> TouchPointPolicy.geometryJson(1, empty, null, new char[0], new float[0], 0), "an unmeasured (zero-width) key");
        float[] nan = letters.clone();
        nan[0] = Float.NaN;
        expectFailure(() -> TouchPointPolicy.geometryJson(1, nan, null, new char[0], new float[0], 0), "a non-finite key");
        expectFailure(() -> TouchPointPolicy.geometryJson(1, letters, null, new char[] {';'}, new float[] {0, 0, 1, 1}, 1), "a duplicate that is not a letter");
        expectFailure(() -> TouchPointPolicy.geometryJson(1, letters, null, new char[] {'g'}, new float[0], 1), "a duplicate without its rectangle");
        System.out.println("Android double-pinyin touch points: routing, take-and-clear, expiry, rollover and geometry JSON passed");
    }

    private static void expectFailure(Runnable action, String message) {
        try {
            action.run();
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError("Expected rejection: " + message);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
