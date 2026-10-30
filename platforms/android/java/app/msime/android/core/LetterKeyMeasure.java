package app.msime.android;

import android.graphics.Rect;
import android.view.View;
import android.view.ViewGroup;
import java.util.Arrays;
import java.util.List;

/**
 * 量 26 键字母层各键在按键区里的矩形（left、top、right、bottom），滑行输入（{@link ImeGlideTyping}）和双拼按下点的键位几何（{@link ImeTouchPoints}）共用。
 *
 * <p>位置按布局算（`offsetDescendantRectToMyCoords` 只看各级的 left、top 和滚动），按下动画的缩放和位移不算在内，和按键区记下的按下点同一个坐标系。每个字母的第一个键记进 {@link #letters}；分离式键盘右半边重复的 G、V（#6022）建得比左边那个晚，记进 {@link #duplicates}；微软双拼的 `;` 键只在显示时记进 {@link #semicolon}。
 */
final class LetterKeyMeasure {
    static final int LETTERS = 26;
    /** 分离式键盘最多重复两三个键，留足余量。 */
    static final int MAX_DUPLICATES = 8;

    /** 'a'..'z' 各键的矩形，每个 4 个数；没量到的键全是 0。 */
    final float[] letters = new float[LETTERS * 4];
    /** 微软双拼 `;` 键的矩形，{@link #hasSemicolon} 为假时无意义。 */
    final float[] semicolon = new float[4];
    boolean hasSemicolon;
    /** 重复的字母键：字母和矩形，前 {@link #duplicateCount} 个有效。 */
    final char[] duplicateLetters = new char[MAX_DUPLICATES];
    final float[] duplicates = new float[MAX_DUPLICATES * 4];
    int duplicateCount;
    /** 量到的字母个数（每个字母只算第一个键）。 */
    int found;
    /** 量到的字母键的平均宽、高；一个都没量到时为 0。 */
    float keyWidth;
    float keyHeight;
    private final Rect rect = new Rect();

    /** 按 `keys` 与 `inputs`（同一下标是同一个键和它送的字符）重新量一遍，返回量到的字母个数。没挂在窗口上、宽高为 0（键行刚重建、还没布局）或不可见的键不算。 */
    int measure(ViewGroup area, List<? extends View> keys, List<String> inputs) {
        Arrays.fill(letters, 0);
        hasSemicolon = false;
        duplicateCount = 0;
        found = 0;
        float widths = 0;
        float heights = 0;
        for (int index = 0; index < keys.size() && index < inputs.size(); index++) {
            String input = inputs.get(index);
            if (input.length() != 1) continue;
            char character = input.charAt(0);
            boolean letter = character >= 'a' && character <= 'z';
            if (!letter && character != ';') continue;
            View key = keys.get(index);
            if (key.getWindowToken() == null || key.getVisibility() != View.VISIBLE
                    || key.getWidth() <= 0 || key.getHeight() <= 0) continue;
            rect.set(0, 0, key.getWidth(), key.getHeight());
            area.offsetDescendantRectToMyCoords(key, rect);
            if (!letter) {
                put(semicolon, 0);
                hasSemicolon = true;
                continue;
            }
            int slot = (character - 'a') * 4;
            if (letters[slot + 2] > 0) {
                if (duplicateCount < MAX_DUPLICATES) {
                    duplicateLetters[duplicateCount] = character;
                    put(duplicates, duplicateCount * 4);
                    duplicateCount++;
                }
                continue;
            }
            put(letters, slot);
            widths += rect.width();
            heights += rect.height();
            found++;
        }
        keyWidth = found == 0 ? 0 : widths / found;
        keyHeight = found == 0 ? 0 : heights / found;
        return found;
    }

    private void put(float[] target, int at) {
        target[at] = rect.left;
        target[at + 1] = rect.top;
        target[at + 2] = rect.right;
        target[at + 3] = rect.bottom;
    }
}
