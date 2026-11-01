package app.msime.android.home;

import java.util.ArrayList;
import java.util.List;

/**
 * 排序面板（「文本编辑按键顺序」「符号分类顺序」）的纯逻辑：拖动和读屏「上移」「下移」都经 {@link #moved} 改草稿，行首序号和读屏描述经 {@link #ordinal} / {@link #ordinalDescription} 生成，拖动途中的位次、落点和贴边自动滚动经 {@link #dragSlot} / {@link #dragTarget} / {@link #edgeScrollStep} / {@link #scrollTowardRoom} 算。
 *
 * <p>拖动规则与「九键左侧符号」编辑页（#10396 的 `NineKeySidebarPolicy`）逐条相同，两边的手感和读屏念法一致。不引用 androidx，好让 check-host 的 JVM 冒烟（`ReorderPolicySmoke`）直接测到；画行和拖动在 {@link ReorderList} 里。
 */
final class ReorderPolicy {
    private ReorderPolicy() {}

    /**
     * 把 `from` 处的一项挪到 `to`，中间各项顺次让位。拖动一次跨过几行时也是挪动而不是两两交换，屏幕上看到的顺序就是保存的顺序。任一下标越界时原样返回；返回的列表始终不可变。
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

    /** 行首带序号的标题，例如第 0 行「复制」是「1. 复制」。读屏不念这串，念 {@link #ordinalDescription}。 */
    static String ordinal(int index, String label) {
        return (index + 1) + ". " + label;
    }

    /** 读屏念的位置：「第 3 项」。拖动把手把它当状态念，读屏用「上移」「下移」挪过之后焦点落在把手上，听得到挪到了哪一位。 */
    static String positionDescription(int index) {
        return "第 " + (index + 1) + " 项";
    }

    /** 读屏念的一行：「第 1 项，复制」。可见的「1.」不念，位置只在这里念一次。 */
    static String ordinalDescription(int index, String label) {
        return positionDescription(index) + "，" + label;
    }

    /**
     * 拖动途中第 `index` 行此刻显示在第几位：被拖的那一行（`from`）在 `to`，夹在两者之间的行朝 `from` 那一侧让一格，其余不动。行的视图在拖动中不换位置，序号和让位位移都按它算，松手后才把表按 {@link #moved} 挪好。
     */
    static int dragSlot(int index, int from, int to) {
        if (index == from) return to;
        if (from < index && index <= to) return index - 1;
        if (to <= index && index < from) return index + 1;
        return index;
    }

    /**
     * 拖动中的行该落在第几位：数其余各行里布局中线（不含让位动画的位移）在被拖行中线之上的有几行，被拖行的中线碰到哪一行的中线就算到了那一行（被拖行夹在列表上下沿之间，拖到底时中线正好落在最后一行的中线上）。各行中线都按布局取，所以结果只随手指走，不随别的行的动画抖动；被拖行自己不参与比较。
     *
     * @param draggedCenter 被拖行此刻的中线（跟手后的位置）
     * @param from 被拖行的原位
     * @param centers 各行在布局里的中线，与表同序
     */
    static int dragTarget(float draggedCenter, int from, float[] centers) {
        int target = 0;
        for (int index = 0; index < centers.length; index++) {
            if (index < from ? centers[index] < draggedCenter : index > from && centers[index] <= draggedCenter) target++;
        }
        return target;
    }

    /**
     * 拖动时手指贴近滚动区上下边缘，每一帧让滚动区滚多少（像素，负数朝上）：进入边缘带越深滚得越快，到边缘外就是最快的 `maxStep`；不在边缘带里是 0。能不能再往那个方向滚由调用方判断。
     *
     * @param y 手指的屏幕纵坐标
     * @param top 滚动区可见部分的上沿
     * @param bottom 滚动区可见部分的下沿
     * @param edge 边缘带的高度
     * @param maxStep 每帧最多滚多少
     */
    static int edgeScrollStep(float y, float top, float bottom, float edge, float maxStep) {
        if (edge <= 0 || bottom - top <= 2 * edge) return 0;
        float depth;
        if (y < top + edge) depth = -(top + edge - y);
        else if (y > bottom - edge) depth = y - (bottom - edge);
        else return 0;
        float ratio = Math.min(1f, Math.abs(depth) / edge);
        int step = Math.max(1, Math.round(maxStep * ratio));
        return depth < 0 ? -step : step;
    }

    /**
     * 被拖行还能朝 `step` 的方向挪时才照 {@link #edgeScrollStep} 自动滚动，否则是 0：它已经夹在列表下沿（`offset` 到了 `maxOffset`）时再往下滚，或夹在上沿时再往上滚，列表带着它滚走，行就离开手指了。
     *
     * @param step {@link #edgeScrollStep} 给出的一步
     * @param offset 被拖行此刻的跟手位移
     * @param minOffset 位移的下限（被拖行到了列表上沿）
     * @param maxOffset 位移的上限（被拖行到了列表下沿）
     */
    static int scrollTowardRoom(int step, float offset, float minOffset, float maxOffset) {
        if (step > 0 && offset >= maxOffset) return 0;
        if (step < 0 && offset <= minOffset) return 0;
        return step;
    }
}
