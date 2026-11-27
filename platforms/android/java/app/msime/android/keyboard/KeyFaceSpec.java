package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 一种键面会变的键（⇧、14/18 键的分词键、九键的 1 键……）能画什么的唯一来源：在一副键盘上它能处在哪些状态，此刻处在哪个状态，每个状态画什么。内容域是全部状态的键面（{@link #texts}），建键时的文字是第一个状态的键面，渲染时由 {@code ImeKeyFaces.show} 按此刻的状态把文字或图标和提示一起设上，键面上的字只在 {@link #face} 里写一次。
 *
 * <p>三个方法都是纯函数，JVM 冒烟把每种布局、每种 {@link KeyFaceLive} 都过一遍：此刻的状态总在 {@link #states} 里，{@link #states} 里的每个状态也总有一种情形会走到（列多了的字会让键平白缩小，列少了的字会在画上时被键组当作漏网的字记下来）。
 *
 * @param <S> 状态
 */
public interface KeyFaceSpec<S> {
    /** 在 {@code layout} 上能处在的全部状态；第一个是建键时的状态。 */
    List<S> states(KeyFaceLayout layout);

    /** 此刻的状态；总在 {@link #states} 里。 */
    S state(KeyFaceLayout layout, KeyFaceLive live);

    /** 这个状态画什么。 */
    KeyFace face(S state);

    /** 这种键会不会出现在 {@code layout} 上；不会时它的字不进那副键盘的内容域。 */
    default boolean on(KeyFaceLayout layout) {
        return true;
    }

    /** 这种键的右上角由它自己的状态决定（{@link KeyFace#corner}）；为假时设键面不碰右上角（九键 1 键右上角的数字另有来处）。 */
    default boolean drawsCorner() {
        return false;
    }

    /** 画图标时由规格定图标种类；为假时只切换画文字还是画图标，图标种类由键自己管（回车在剪贴板模式里换图标）。 */
    default boolean ownsIconKind() {
        return true;
    }

    /** 此刻画什么。 */
    default KeyFace face(KeyFaceLayout layout, KeyFaceLive live) {
        return face(state(layout, live));
    }

    /** 建键时画什么（第一个状态）。 */
    default KeyFace initial(KeyFaceLayout layout) {
        return face(states(layout).get(0));
    }

    /** 在 {@code layout} 上 {@code slot} 位置可能画的全部文字（图标不算），按状态的顺序、去重。 */
    default List<String> texts(KeyFaceLayout layout, KeyFaceSlot slot) {
        List<String> texts = new ArrayList<>();
        for (S state : states(layout)) {
            KeyFace face = face(state);
            String text = slot == KeyFaceSlot.MAIN ? face.mainText() : slot == KeyFaceSlot.TOP_RIGHT ? face.corner() : "";
            if (!text.isEmpty() && !texts.contains(text)) texts.add(text);
        }
        return texts;
    }
}
