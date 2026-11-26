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
 * <p>内容域在第一次需要时量一遍（键都还没布局时不量），之后键格变化只做算术，同样的最小键格直接查表。内容域是列出来的文字（{@link KeyFaceSpec}、各 {@code *KeyFaceDomains}）加上文字固定的键放进来时自己的文字（{@link #attach}）；皮肤字体、字体缩放或设置变了用 {@link #replace} 整个换掉，只并进文字固定的键放进来时的文字，不并键此刻画着的字（那可能是上一副键盘留下的）。键换上列表里没有的文字（{@link #include}）是列表漏了：照样补进去、重量一次，不让字画出键外，但记作一次漏网（{@link #strayCount}），设备验收要求它为 0。只在主线程用。
 */
public final class KeyFaceGroup {
    /** 组里的一个键。 */
    public interface Member {
        /** 组的参数变了：重画键面。 */
        void faceParamsChanged();

        /** 这个键此刻在 {@code slot} 上画的文字；不画时为空串或 {@code null}。 */
        String faceText(KeyFaceSlot slot);

        /** 这个键会在主标签的位置画图标（⌫、⇧、↵ 这类键，此刻画的也许是文字）：图标的边长按这些键的键格算，与文字分开（{@link KeyFaceSolver.Icons}）。 */
        default boolean drawsIcons() { return false; }
    }

    /** {@code fixed} 是文字固定的键放进来时每个位置的文字（{@link #attach(Member, Set, boolean)}），状态由 {@link KeyFaceSpec} 给出的键为 {@code null}。 */
    private record Entry(Set<KeyFaceSlot> slots, int width, int height, Map<KeyFaceSlot, String> fixed, boolean icons) {
        boolean sized() { return width > 0 && height > 0; }
    }

    /** 收到列表里没有的文字时通知（{@code ImeKeyFaces} 记一行日志）；JVM 冒烟里没有。 */
    public interface StrayListener {
        void stray(KeyFaceGroup group, KeyFaceSlot slot, String text);
    }

    private static StrayListener strayListener;
    private static int strays;

    /** 查表用：最小、最大键格、主标签的上限和图标这一维都相同时参数相同。 */
    private record Solved(Map<KeyFaceSlot, KeyFaceSolver.Cell> cells, Map<KeyFaceSlot, KeyFaceSolver.Cell> largest,
            float mainCapPx, KeyFaceSolver.Icons icons) { }

    private static final int CACHE_SIZE = 8;

    private final KeyFaceTemplate template;
    private KeyFaceEnvironment environment;
    private KeyFaceMeasurer measurer;
    private KeyFaceDomain domain;
    private Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics;
    private final Map<Member, Entry> members = new IdentityHashMap<>();
    private final Map<Solved, KeyFaceParams> solved = new LinkedHashMap<>(CACHE_SIZE * 2, 0.75f, true);
    /** 主标签不大过这一组的主标签（{@link #capMainAt}）；没有时不另设上限。 */
    private KeyFaceGroup leader;
    private final List<KeyFaceGroup> followers = new ArrayList<>();
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

    /** 文字固定的键放进组里（{@link #attach(Member, Set, boolean)}，{@code fixedText} 为真）。 */
    public void attach(Member member, Set<KeyFaceSlot> slots) {
        attach(member, slots, true);
    }

    /**
     * 键放进组里，{@code slots} 是它可能用到的位置（主标签总算在内）。尺寸未知前不参与计算。
     *
     * @param fixedText 为真时这个键的文字放进来以后不变（或只在列出来的文字之间换），它此刻的文字并进内容域；为假时它的全部状态已经列在内容域里（{@link KeyFaceSpec}），此刻的文字不并进来：常驻的键放进新组时可能还画着上一副键盘的字
     */
    public void attach(Member member, Set<KeyFaceSlot> slots, boolean fixedText) {
        EnumSet<KeyFaceSlot> used = EnumSet.of(KeyFaceSlot.MAIN);
        used.addAll(slots);
        Entry previous = members.get(member);
        Map<KeyFaceSlot, String> fixed = null;
        if (fixedText) {
            fixed = new EnumMap<>(KeyFaceSlot.class);
            for (KeyFaceSlot slot : used) {
                String text = member.faceText(slot);
                if (text != null && !text.isEmpty()) fixed.put(slot, text);
            }
        }
        members.put(member, new Entry(Collections.unmodifiableSet(used),
            previous == null ? 0 : previous.width(), previous == null ? 0 : previous.height(), fixed, member.drawsIcons()));
        KeyFaceDomain grown = fixed == null ? domain : with(domain, fixed);
        boolean grew = grown != domain;
        domain = grown;
        if (grew) changed();
        else if (previous == null || !previous.slots().equals(used)) update();
    }

    /** 键拿下来：不再参与最小键格。 */
    public void detach(Member member) {
        if (members.remove(member) != null) update();
    }

    /** 键的尺寸（像素）变了；0 表示不画（隐藏、离开窗口、还没布局），不参与最小键格。 */
    public void cellChanged(Member member, int width, int height) {
        Entry entry = members.get(member);
        if (entry == null || entry.width() == width && entry.height() == height) return;
        members.put(member, new Entry(entry.slots(), width, height, entry.fixed(), entry.icons()));
        update();
    }

    /**
     * 键要在 {@code slot} 上画 {@code text}：内容域里有就什么也不做。没有就是列表漏了（列表应当列出键能画的每段字，见 {@link KeyFaceSpec}）：照样补进去、重算，字不会画出键外，但记一次漏网（{@link #strayCount}）。
     */
    public void include(KeyFaceSlot slot, String text) {
        if (domain.contains(slot, text)) return;
        strays++;
        if (strayListener != null) strayListener.stray(this, slot, text);
        domain = domain.with(slot, text);
        changed();
    }

    /**
     * 换环境、量字方式和列出来的文字（皮肤字体、字体缩放或设置变了）。文字固定的键放进来时的文字照样并进来；键此刻画着的字不并：常驻的键这时可能还画着上一副键盘的字，随后的渲染会按新列表换上。
     */
    public void replace(KeyFaceEnvironment environment, KeyFaceMeasurer measurer, KeyFaceDomain domain) {
        this.environment = environment;
        this.measurer = measurer;
        KeyFaceDomain merged = domain;
        for (Entry entry : members.values()) if (entry.fixed() != null) merged = with(merged, entry.fixed());
        this.domain = merged;
        changed();
    }

    /** 收到列表里没有的文字的次数（进程内全部键组），设备验收用。 */
    public static int strayCount() { return strays; }

    /** 设上收到列表里没有的文字时的通知；{@code null} 取消。 */
    public static void setStrayListener(StrayListener listener) { strayListener = listener; }

    private static KeyFaceDomain with(KeyFaceDomain domain, Map<KeyFaceSlot, String> texts) {
        for (Map.Entry<KeyFaceSlot, String> text : texts.entrySet()) domain = domain.with(text.getKey(), text.getValue());
        return domain;
    }

    /**
     * 这一组主标签的首选字号不大过 {@code leader} 此刻的主标签（{@code null} 取消）：功能键的字不比同一副键盘的字母显得更重。{@code leader} 画图标时，这一组的图标也不大过它的图标（文字和图标是并列的两维，各按各的上限）。{@code leader} 的参数变了，这一组跟着重算；它还没有参数时不设上限。两组各自按自己的键格算，除此之外互不影响。
     */
    public void capMainAt(KeyFaceGroup leader) {
        if (this.leader == leader) return;
        if (this.leader != null) this.leader.followers.remove(this);
        this.leader = leader;
        if (leader != null) leader.followers.add(this);
        update();
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
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = cells(true);
        KeyFaceParams next = null;
        if (cells.containsKey(KeyFaceSlot.MAIN)) {
            float cap = leader == null || leader.params == null ? Float.POSITIVE_INFINITY : leader.params.main().sizePx();
            // 图标与文字并列：上限取领头组的图标（它画图标时），键格取画图标的键。
            float iconCap = leader == null || leader.params == null || leader.params.iconPx() <= 0 ? Float.POSITIVE_INFINITY
                : leader.params.iconPx();
            KeyFaceSolver.Icons icons = new KeyFaceSolver.Icons(iconCell(false), iconCell(true), iconCap);
            // 最大键格只在轮廓随键宽缩放时影响结果（右上角提示在大键上挪得更远）；没有轮廓时不进表键，免得键宽一变就重算。
            Map<KeyFaceSlot, KeyFaceSolver.Cell> largest = environment.outline().none() ? cells : cells(false);
            Solved key = new Solved(cells, largest, cap, icons);
            next = solved.get(key);
            if (next == null) {
                if (metrics == null) {
                    metrics = domain.measure(measurer, template.pairGapEm(), template.glyphs());
                    measureCount++;
                }
                next = KeyFaceSolver.solve(template, metrics, environment, cells, largest, cap, icons);
                solveCount++;
                solved.put(key, next);
                if (solved.size() > CACHE_SIZE) solved.remove(solved.keySet().iterator().next());
            }
        }
        if (Objects.equals(next, params)) return;
        params = next;
        List<Member> notify = new ArrayList<>(members.keySet());
        for (Member member : notify) member.faceParamsChanged();
        for (KeyFaceGroup follower : new ArrayList<>(followers)) follower.update();
    }

    /** 画图标的、已经布局好的键里最小的宽和高（{@code corner} 为真时只算带右上角提示的）；没有时为 {@code null}。 */
    private KeyFaceSolver.Cell iconCell(boolean corner) {
        int width = Integer.MAX_VALUE, height = Integer.MAX_VALUE;
        for (Entry entry : members.values()) {
            if (!entry.sized() || !entry.icons() || corner && !entry.slots().contains(KeyFaceSlot.TOP_RIGHT)) continue;
            width = Math.min(width, entry.width());
            height = Math.min(height, entry.height());
        }
        return width == Integer.MAX_VALUE ? null : new KeyFaceSolver.Cell(width, height);
    }

    /** 每个位置：带这个位置、已经布局好的键里最小（{@code smallest}）或最大的宽和高。 */
    private Map<KeyFaceSlot, KeyFaceSolver.Cell> cells(boolean smallest) {
        EnumMap<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        for (Entry entry : members.values()) {
            if (!entry.sized()) continue;
            for (KeyFaceSlot slot : entry.slots()) {
                KeyFaceSolver.Cell cell = cells.get(slot);
                cells.put(slot, cell == null ? new KeyFaceSolver.Cell(entry.width(), entry.height())
                    : smallest ? new KeyFaceSolver.Cell(Math.min(cell.width(), entry.width()), Math.min(cell.height(), entry.height()))
                    : new KeyFaceSolver.Cell(Math.max(cell.width(), entry.width()), Math.max(cell.height(), entry.height())));
            }
        }
        return cells;
    }
}
