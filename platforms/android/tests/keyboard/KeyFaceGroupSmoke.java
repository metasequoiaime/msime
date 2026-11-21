package app.msime.android;

import java.util.EnumMap;
import java.util.Map;
import java.util.Set;

/** 键组：全组按最小的键格算一份参数，提示按带它的最小键格算；结果变了才通知、同样的最小键格不重算，没布局好或拿下来的键不参与。 */
public final class KeyFaceGroupSmoke {
    private KeyFaceGroupSmoke() { }

    private static final class Key implements KeyFaceGroup.Member {
        final Map<KeyFaceSlot, String> texts = new EnumMap<>(KeyFaceSlot.class);
        int changes;

        @Override public void faceParamsChanged() { changes++; }

        @Override public String faceText(KeyFaceSlot slot) { return texts.get(slot); }
    }

    public static void main(String[] args) {
        KeyFaceGroup group = new KeyFaceGroup(KeyFaceSolverSmoke.template(true, true), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(true, true), KeyFaceSolverSmoke.MEASURER);
        Key wide = new Key();
        Key narrow = new Key();
        Key plain = new Key();
        group.attach(wide, Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM));
        group.attach(narrow, Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM));
        group.attach(plain, Set.of());
        check(group.params() == null, "还没有布局好的键时不应有参数");
        check(group.measureCount() == 0, "键都还没布局时不应量字");

        group.cellChanged(wide, 300, 150);
        KeyFaceParams first = group.params();
        check(first != null && wide.changes == 1 && narrow.changes == 1, "第一份参数应通知组里每个键");
        group.cellChanged(narrow, 80, 150);
        KeyFaceParams small = group.params();
        check(!small.equals(first), "更窄的键加入后应按它重算");
        check(small.equals(expected(80, 150, 80, 150)), "全组应按最小键格算");

        // 更大的键变化不改变最小键格：不重算、不通知。
        int solves = group.solveCount();
        int changes = wide.changes;
        group.cellChanged(wide, 320, 160);
        check(group.solveCount() == solves && wide.changes == changes, "最小键格没变时不应重算或通知");

        // 主标签按所有键里最小的算；提示只按带它的键算。
        group.cellChanged(plain, 60, 150);
        KeyFaceParams mixed = group.params();
        check(mixed.equals(expected(60, 150, 80, 150)), "主标签按最小键格、提示按带提示的最小键格算");

        // 拿下来或尺寸为 0 的键不参与；回到算过的最小键格时直接查表。
        solves = group.solveCount();
        group.detach(plain);
        check(group.params().equals(small), "拿下来的键不应再算进最小键格");
        group.cellChanged(narrow, 0, 0);
        check(group.params().equals(expected(320, 160, 320, 160)), "尺寸为 0 的键不应算进最小键格");
        group.cellChanged(narrow, 80, 150);
        check(group.params().equals(small), "回到同样的最小键格应得到同样的参数");
        check(group.solveCount() == solves + 1, "算过的最小键格应直接查表，实际重算 " + (group.solveCount() - solves) + " 次");

        // 内容域决定预留：同样的键格，内容域里有角标时主标签按角标退让，没有时照常居中。
        KeyFaceGroup bare = new KeyFaceGroup(KeyFaceSolverSmoke.template(true, true), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(false, true), KeyFaceSolverSmoke.MEASURER);
        Key only = new Key();
        bare.attach(only, Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM));
        bare.cellChanged(only, 120, 150);
        check(bare.params().topRight() == null && bare.params().strategy() == KeyFaceTemplate.Strategy.CENTERED,
            "内容域没有角标时不应为它留位置");

        // 键换上内容域里没有的文字：补进内容域、只重量一遍；已有的文字不重量。
        int measured = group.measureCount();
        group.include(KeyFaceSlot.TOP_RIGHT, "!");
        check(group.measureCount() == measured, "内容域已有的文字不应重量");
        group.include(KeyFaceSlot.MAIN, "WW");
        check(group.domain().contains(KeyFaceSlot.MAIN, "WW") && group.measureCount() == measured + 1,
            "新文字应补进内容域并重量一遍");
        check(group.params().main().sizePx() < small.main().sizePx(), "更宽的主标签补进来后全组应收小字号");

        // 整个换掉内容域时，组里每个键此刻画着的文字并进来。
        Key hinted = new Key();
        hinted.texts.put(KeyFaceSlot.BOTTOM, "uang");
        group.attach(hinted, Set.of(KeyFaceSlot.BOTTOM));
        check(group.domain().contains(KeyFaceSlot.BOTTOM, "uang"), "放进组里的键的文字应进内容域");
        group.replace(KeyFaceSolverSmoke.ENVIRONMENT, KeyFaceSolverSmoke.MEASURER, KeyFaceSolverSmoke.domain(true, false));
        check(group.domain().contains(KeyFaceSlot.BOTTOM, "uang") && !group.domain().contains(KeyFaceSlot.BOTTOM, "ong"),
            "换内容域时应保留键此刻的文字、丢掉不再可能出现的文字");
        System.out.println("KeyFaceGroupSmoke passed");
    }

    private static KeyFaceParams expected(int mainWidth, int mainHeight, int hintWidth, int hintHeight) {
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        cells.put(KeyFaceSlot.MAIN, new KeyFaceSolver.Cell(mainWidth, mainHeight));
        cells.put(KeyFaceSlot.TOP_RIGHT, new KeyFaceSolver.Cell(hintWidth, hintHeight));
        cells.put(KeyFaceSlot.BOTTOM, new KeyFaceSolver.Cell(hintWidth, hintHeight));
        return KeyFaceSolver.solve(KeyFaceSolverSmoke.template(true, true),
            KeyFaceSolverSmoke.domain(true, true).measure(KeyFaceSolverSmoke.MEASURER, 0), KeyFaceSolverSmoke.ENVIRONMENT, cells);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
