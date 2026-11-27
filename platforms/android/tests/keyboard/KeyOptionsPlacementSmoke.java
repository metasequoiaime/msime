import app.msime.android.KeyOptionsPlacement;

public final class KeyOptionsPlacementSmoke {
    private static void equal(int actual, int expected, String message) {
        if (actual != expected) throw new AssertionError(message + ": expected " + expected + ", got " + actual);
    }

    public static void main(String[] args) {
        // 1080×2400、420 dpi 的停靠键盘：面板上沿 1570，顶排键上沿 1675，状态栏下沿 128。
        int keyTop = 1675;
        int gap = 16;
        int safeTop = 128;
        int bottom = 2400;
        // 一行选项（38 dp = 100 px）：在键上方，上沿高过面板上沿。
        equal(KeyOptionsPlacement.top(keyTop, 100, gap, safeTop, bottom), 1559, "one row floats above the key");
        // 三行（英文 e/o/i/u，300 px）：整个浮在面板之上，不再压回键区。
        equal(KeyOptionsPlacement.top(keyTop, 300, gap, safeTop, bottom), 1359, "three rows rise above the panel");
        // 键靠近屏幕上沿时停在状态栏之下。
        equal(KeyOptionsPlacement.top(300, 300, gap, safeTop, bottom), 128, "never above the safe top");
        // 弹窗底边不伸出键盘下沿。
        equal(KeyOptionsPlacement.top(2390, 100, gap, safeTop, 2300), 2200, "never below the keyboard");

        // API 29 及以下子窗口被输入法窗口裁剪：停靠窗口上沿 1360（面板上方 80 dp），三行选项停在 1360，不再伸出去。
        int limit29 = KeyOptionsPlacement.limit(safeTop, 1360, 29);
        equal(limit29, 1360, "API 29 keeps popups inside the IME window");
        equal(KeyOptionsPlacement.top(keyTop, 300, gap, limit29, bottom), 1360, "three rows stop at the window top on API 29");
        equal(KeyOptionsPlacement.top(keyTop, 100, gap, limit29, bottom), 1559, "one row is unaffected on API 29");
        // API 28 的停靠窗口是全高，上沿就在状态栏下；API 30 起只看屏幕可用区。
        equal(KeyOptionsPlacement.limit(safeTop, 63, 28), safeTop, "API 28's full-height window adds no limit");
        equal(KeyOptionsPlacement.limit(safeTop, 1360, 30), safeTop, "API 30+ draws popups outside the IME window");
    }
}
