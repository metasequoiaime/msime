package app.msime.android;

/**
 * 主标签下面还有一行小字的键：注音九键的音键，注音符号下面是它发送的数字。小字画在键组的底部位置（{@link KeyFaceSlot#BOTTOM}），颜色与主标签相同（{@link KeyHintButton} 的底部提示用皮肤的强调色，是另一种东西）。只在键组里画，键组外的键只画主标签。
 */
public final class BottomLabelKey extends KeyboardPressButton {
    private String bottomText = "";

    public BottomLabelKey(android.content.Context context) {
        super(context);
    }

    /** 底部的小字；空串表示不画。 */
    public void setBottomText(String value) {
        String next = value == null ? "" : value;
        if (bottomText.equals(next)) return;
        bottomText = next;
        faceTextChanged(KeyFaceSlot.BOTTOM, next);
    }

    @Override public String faceText(KeyFaceSlot slot) {
        return slot == KeyFaceSlot.BOTTOM ? bottomText : super.faceText(slot);
    }
}
