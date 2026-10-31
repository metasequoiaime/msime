package app.msime.android.home;

import java.util.ArrayList;
import java.util.List;

/**
 * 排序面板（「文本编辑按键顺序」「符号分类顺序」）的纯逻辑：拖动和读屏「上移」「下移」都经 {@link #moved} 改草稿，行首序号经 {@link #ordinal} 生成。
 *
 * <p>不引用 androidx，好让 check-host 的 JVM 冒烟（`ReorderPolicySmoke`）直接测到；画行和拖动在 {@link ReorderList} 里。
 */
final class ReorderPolicy {
    private ReorderPolicy() {}

    /**
     * 把 `from` 处的一项挪到 `to`，中间各项顺次让位，与 `RecyclerView.Adapter#notifyItemMoved` 的语义一致。拖动一次跨过几行时也是挪动而不是两两交换，屏幕上看到的顺序就是保存的顺序。任一下标越界时原样返回；返回的列表始终不可变。
     */
    static <T> List<T> moved(List<T> order, int from, int to) {
        if (from < 0 || from >= order.size() || to < 0 || to >= order.size()) return List.copyOf(order);
        List<T> result = new ArrayList<>(order);
        result.add(to, result.remove(from));
        return List.copyOf(result);
    }

    /** 第 `index` 行（从 0 数）往前或往后挪 `delta` 格后是否仍在表内；读屏的「上移」「下移」只在为真时提供。 */
    static boolean canMove(int index, int delta, int size) {
        int target = index + delta;
        return index >= 0 && index < size && target >= 0 && target < size;
    }

    /** 行首带序号的标题，例如第 0 行「复制」是「1. 复制」。同一串文字也是这一行的读屏描述。 */
    static String ordinal(int index, String label) {
        return (index + 1) + ". " + label;
    }
}
