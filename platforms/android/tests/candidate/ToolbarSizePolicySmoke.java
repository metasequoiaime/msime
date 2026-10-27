package app.msime.android;

public final class ToolbarSizePolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    public static void main(String[] args) {
        check(ToolbarSizePolicy.percent(90) == 90);
        check(ToolbarSizePolicy.percent(150) == 150);
        check(ToolbarSizePolicy.percent(89) == 100);
        check(ToolbarSizePolicy.percent(151) == 100);
        check(ToolbarSizePolicy.heightDp(90) == 50);
        check(ToolbarSizePolicy.heightDp(100) == 56);
        check(ToolbarSizePolicy.heightDp(150) == 84);
        check(ToolbarSizePolicy.buttonHeightDp(90) == 44);
        check(ToolbarSizePolicy.buttonHeightDp(150) == 66);
        check(ToolbarSizePolicy.buttonWidthDp(40, 90) == 40);
        check(ToolbarSizePolicy.buttonWidthDp(40, 150) == 60);
        check(ToolbarSizePolicy.idleSpacerPx(50, 56) == 6);
        check(ToolbarSizePolicy.composingSpacerPx(50, 56) == 0);
        check(ToolbarSizePolicy.idleSpacerPx(84, 56) == 0);
        check(ToolbarSizePolicy.composingSpacerPx(84, 56) == 28);
    }
}
