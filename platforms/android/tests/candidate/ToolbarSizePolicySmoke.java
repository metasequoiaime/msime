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

        // #10137：跟随候选栏时按钮最高 44，行高不够时取行高，图标按同一比例缩小，不再伸出这一行。
        check(ToolbarSizePolicy.followedButtonHeight(56, 44) == 44);
        check(ToolbarSizePolicy.followedButtonHeight(35, 44) == 35);
        check(ToolbarSizePolicy.followedButtonHeight(0, 44) == 44);
        check(ToolbarSizePolicy.followedPercent(56, 44) == 100);
        check(ToolbarSizePolicy.followedPercent(44, 44) == 100);
        check(ToolbarSizePolicy.followedPercent(35, 44) == 79);
        check(ToolbarSizePolicy.followedPercent(22, 44) == 50);
        check(ToolbarSizePolicy.followedPercent(0, 44) == 100);
        check(ToolbarSizePolicy.followedPercent(10, 0) == 100);
        // 跟随候选栏缩小时绘制比例可以低于 90%，上限仍是 150%。
        check(ToolbarSizePolicy.drawPercent(50) == 50);
        check(ToolbarSizePolicy.drawPercent(0) == 1);
        check(ToolbarSizePolicy.drawPercent(400) == 150);
        // 预览按读音行 14 加缩放后的 42 dp 候选行，与键盘的 ImeToolbar 常量一致。
        check(ImeToolbar.READING_ROW_DP == 14 && ImeToolbar.CANDIDATE_LINE_DP == 42);
        check(ToolbarSizePolicy.followedHeightDp(100) == 56);
        check(ToolbarSizePolicy.followedHeightDp(50) == 35);
        check(ToolbarSizePolicy.followedHeightDp(200) == 98);
        check(ToolbarSizePolicy.followedHeightDp(30) == 56);
        // 默认 100% 时预览和键盘的按钮仍是原来的 44 dp，没有调过的用户看不到变化。
        check(ToolbarSizePolicy.followedButtonHeight(ToolbarSizePolicy.followedHeightDp(100), 44) == 44);
        check(ToolbarSizePolicy.followedButtonHeight(ToolbarSizePolicy.followedHeightDp(50), 44) == 35);
    }
}
