package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * 一组看起来应当一致的键（同一副键盘的字母键、底行功能键……）：共用一个模板和内容域，按组里最小的键格算一份 {@link KeyFaceParams}，组里每个键都用这一份，字号、对齐和退让方式全组相同。
 *
 * <p>生命周期跟一次建键盘：建键行时新建，键放进行里时 {@link #attach}，拿下来时 {@link #detach}。键的尺寸在布局之后才知道，所以键在尺寸变化或隐藏时调 {@link #cellChanged}；结果变了，组里每个键都收到 {@link Member#faceParamsChanged} 去重画。键面不影响键的尺寸，所以这里只会引起重画，不会引起重新布局。
 *
 * <p>内容域在第一次需要时量一遍（键都还没布局时不量），之后键格变化只做算术，同样的最小键格直接查表。键换上内容域里没有的文字时（{@link #include}，例如按方案画上的双拼提示）补进内容域、重量一次；皮肤字体、字体缩放或设置变了用 {@link #replace} 整个换掉，组里每个键此刻的文字会并进新的内容域。只在主线程用。
 */
public final class KeyFaceGroup {
    /** 组里的一个键。 */
    public interface Member {
        /** 组的参数变了：重画键面。 */
        void faceParamsChanged();

        /** 这个键此刻在 {@code slot} 上画的文字；不画时为空串或 {@code null}。 */
        String faceText(KeyFaceSlot slot);
    }

    private record Entry(Set<KeyFaceSlot> slots, int width, int height) {
        boolean sized() { return width > 0 && height > 0; }
    }

    private static final int CACHE_SIZE = 8;

    private final KeyFaceTemplate template;
    private KeyFaceEnvironment environment;
    private KeyFaceMeasurer measurer;
    private KeyFaceDomain domain;
    private Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics;
    private final Map<Member, Entry> members = new IdentityHashMap<>();
    private final Map<Map<KeyFaceSlot, KeyFaceSolver.Cell>, KeyFaceParams> solved =
        new LinkedHashMap<>(CACHE_SIZE * 2, 0.75f, true);
    private KeyFaceParams params;
    private int solveCount;
    private int measureCount;

    public KeyFaceGroup(KeyFaceTemplate template, KeyFaceEnvironment environment, KeyFaceDomain domain,
            KeyFaceMeasurer measurer) {
        this.template = template;
        this.environment = environment;
        this.domain = domain;
        this.measurer = measurer;
    }

    public KeyFaceTemplate template() { return template; }

    public KeyFaceDomain domain() { return domain; }

    public KeyFaceEnvironment environment() { return environment; }

    public KeyFaceMeasurer measurer() { return measurer; }

    /** 键放进组里，{@code slots} 是它可能用到的位置（主标签总算在内）。尺寸未知前不参与计算。 */
    public void attach(Member member, Set<KeyFaceSlot> slots) {
        EnumSet<KeyFaceSlot> used = EnumSet.of(KeyFaceSlot.MAIN);
        used.addAll(slots);
        Entry previous = members.get(member);
        members.put(member, new Entry(Collections.unmodifiableSet(used),
            previous == null ? 0 : previous.width(), previous == null ? 0 : previous.height()));
        for (KeyFaceSlot slot : used) domain = domain.with(slot, member.faceText(slot));
        if (previous == null || !previous.slots().equals(used)) changed();
    }

    /** 键拿下来：不再参与最小键格。 */
    public void detach(Member member) {
        if (members.remove(member) != null) update();
    }

    /** 键的尺寸（像素）变了；0 表示不画（隐藏、离开窗口、还没布局），不参与最小键格。 */
    public void cellChanged(Member member, int width, int height) {
        Entry entry = members.get(member);
        if (entry == null || entry.width() == width && entry.height() == height) return;
        members.put(member, new Entry(entry.slots(), width, height));
        update();
    }

    /** 键要在 {@code slot} 上画 {@code text}：内容域里没有就补进去、重算。 */
    public void include(KeyFaceSlot slot, String text) {
        if (domain.contains(slot, text)) return;
        domain = domain.with(slot, text);
        changed();
    }

    /** 换环境、量字方式和内容域（皮肤字体、字体缩放或设置变了）；组里每个键此刻的文字并进新的内容域。 */
    public void replace(KeyFaceEnvironment environment, KeyFaceMeasurer measurer, KeyFaceDomain domain) {
        this.environment = environment;
        this.measurer = measurer;
        KeyFaceDomain merged = domain;
        for (Map.Entry<Member, Entry> entry : members.entrySet())
            for (KeyFaceSlot slot : entry.getValue().slots()) merged = merged.with(slot, entry.getKey().faceText(slot));
        this.domain = merged;
        changed();
    }

    /** 当前参数；组里还没有布局好的键时为 {@code null}。 */
    public KeyFaceParams params() { return params; }

    /** 实际算过几次（同样的最小键格只算一次），冒烟用。 */
    int solveCount() { return solveCount; }

    /** 量过几遍内容域，冒烟用。 */
    int measureCount() { return measureCount; }

    private void changed() {
        metrics = null;
        solved.clear();
        update();
    }

    private void update() {
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = smallestCells();
        KeyFaceParams next = null;
        if (cells.containsKey(KeyFaceSlot.MAIN)) {
            next = solved.get(cells);
            if (next == null) {
                if (metrics == null) {
                    metrics = domain.measure(measurer, template.pairGapEm());
                    measureCount++;
                }
                next = KeyFaceSolver.solve(template, metrics, environment, cells);
                solveCount++;
                solved.put(cells, next);
                if (solved.size() > CACHE_SIZE) solved.remove(solved.keySet().iterator().next());
            }
        }
        if (Objects.equals(next, params)) return;
        params = next;
        List<Member> notify = new ArrayList<>(members.keySet());
        for (Member member : notify) member.faceParamsChanged();
    }

    /** 每个位置：带这个位置、已经布局好的键里最小的宽和最小的高。 */
    private Map<KeyFaceSlot, KeyFaceSolver.Cell> smallestCells() {
        EnumMap<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        for (Entry entry : members.values()) {
            if (!entry.sized()) continue;
            for (KeyFaceSlot slot : entry.slots()) {
                KeyFaceSolver.Cell cell = cells.get(slot);
                cells.put(slot, cell == null ? new KeyFaceSolver.Cell(entry.width(), entry.height())
                    : new KeyFaceSolver.Cell(Math.min(cell.width(), entry.width()), Math.min(cell.height(), entry.height())));
            }
        }
        return cells;
    }
}
