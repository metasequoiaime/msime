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

    /** 画图标的键（⌫、⇧ 这类）。 */
    private static final class IconKey implements KeyFaceGroup.Member {
        @Override public void faceParamsChanged() { }

        @Override public String faceText(KeyFaceSlot slot) { return ""; }

        @Override public boolean drawsIcons() { return true; }
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

        // 键换上内容域里没有的文字是列表漏了：照样补进内容域、只重量一遍，并记一次漏网；已有的文字不重量、不记。
        int measured = group.measureCount();
        int strays = KeyFaceGroup.strayCount();
        group.include(KeyFaceSlot.TOP_RIGHT, "!");
        check(group.measureCount() == measured && KeyFaceGroup.strayCount() == strays, "内容域已有的文字不应重量，也不算漏网");
        group.include(KeyFaceSlot.MAIN, "WW");
        check(group.domain().contains(KeyFaceSlot.MAIN, "WW") && group.measureCount() == measured + 1,
            "新文字应补进内容域并重量一遍");
        check(KeyFaceGroup.strayCount() == strays + 1, "列表里没有的文字应记一次漏网");
        check(group.params().main().sizePx() < small.main().sizePx(), "更宽的主标签补进来后全组应收小字号");

        // 整个换掉内容域时，文字固定的键放进来时的文字并进来；键面由规格给出的键此刻画着的字不并（可能是上一副键盘留下的）。
        Key hinted = new Key();
        hinted.texts.put(KeyFaceSlot.BOTTOM, "uang");
        group.attach(hinted, Set.of(KeyFaceSlot.BOTTOM));
        check(group.domain().contains(KeyFaceSlot.BOTTOM, "uang"), "放进组里的键的文字应进内容域");
        Key stateful = new Key();
        stateful.texts.put(KeyFaceSlot.MAIN, "WWWW");
        group.attach(stateful, Set.of(), false);
        check(!group.domain().contains(KeyFaceSlot.MAIN, "WWWW"), "键面由规格给出的键放进来时不应并进它此刻的字");
        hinted.texts.put(KeyFaceSlot.BOTTOM, "iong");
        group.replace(KeyFaceSolverSmoke.ENVIRONMENT, KeyFaceSolverSmoke.MEASURER, KeyFaceSolverSmoke.domain(true, false));
        check(group.domain().contains(KeyFaceSlot.BOTTOM, "uang") && !group.domain().contains(KeyFaceSlot.BOTTOM, "ong")
            && !group.domain().contains(KeyFaceSlot.BOTTOM, "iong") && !group.domain().contains(KeyFaceSlot.MAIN, "WWWW"),
            "换内容域时应保留文字固定的键放进来时的文字，丢掉不再可能出现的文字和键此刻画着的字");

        // 图标这一维：图标按画图标的键的键格算（窄的文字键不压小它），并且不大过领头组的图标。
        KeyFaceGroup icons = new KeyFaceGroup(KeyFaceSolverSmoke.iconTemplate(), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(false, false), KeyFaceSolverSmoke.MEASURER);
        Key word = new Key();
        IconKey delete = new IconKey();
        icons.attach(word, Set.of());
        icons.attach(delete, Set.of());
        icons.cellChanged(word, 48, 156);
        icons.cellChanged(delete, 156, 156);
        float iconWide = icons.params().iconPx();
        KeyFaceGroup textOnly = new KeyFaceGroup(KeyFaceSolverSmoke.iconTemplate(), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(false, false), KeyFaceSolverSmoke.MEASURER);
        Key alone = new Key();
        textOnly.attach(alone, Set.of());
        textOnly.cellChanged(alone, 48, 156);
        check(iconWide > textOnly.params().iconPx(), "图标应按画图标的键算，不被窄的文字键压小");
        KeyFaceGroup iconLeader = new KeyFaceGroup(KeyFaceSolverSmoke.iconTemplate(), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(false, false), KeyFaceSolverSmoke.MEASURER);
        IconKey shortKey = new IconKey();
        iconLeader.attach(shortKey, Set.of());
        iconLeader.cellChanged(shortKey, 120, 90);
        icons.capMainAt(iconLeader);
        check(icons.params().iconPx() <= iconLeader.params().iconPx() && iconLeader.params().iconPx() < iconWide,
            "图标应不大过领头组的图标：" + icons.params().iconPx() + " " + iconLeader.params().iconPx());
        icons.capMainAt(null);
        check(icons.params().iconPx() == iconWide, "取消上限后图标应回到自己的边长");

        // 主标签以另一组为上限：跟着那一组的参数重算，取消后回到自己的字号。
        KeyFaceGroup leader = new KeyFaceGroup(KeyFaceSolverSmoke.template(false, false), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(false, false), KeyFaceSolverSmoke.MEASURER);
        KeyFaceGroup follower = new KeyFaceGroup(KeyFaceSolverSmoke.template(false, false), KeyFaceSolverSmoke.ENVIRONMENT,
            KeyFaceSolverSmoke.domain(false, false), KeyFaceSolverSmoke.MEASURER);
        Key letter = new Key();
        Key function = new Key();
        leader.attach(letter, Set.of());
        follower.attach(function, Set.of());
        follower.cellChanged(function, 300, 150);
        float own = follower.params().main().sizePx();
        follower.capMainAt(leader);
        check(follower.params().main().sizePx() == own, "上限那一组还没有参数时不应设上限");
        leader.cellChanged(letter, 60, 150);
        float cap = leader.params().main().sizePx();
        check(cap < own && KeyFaceSolverSmoke.near(follower.params().main().sizePx(), cap),
            "上限那一组的参数变了，这一组应跟着按它封顶：" + follower.params().main().sizePx() + " " + cap);
        int notified = function.changes;
        leader.cellChanged(letter, 300, 150);
        check(follower.params().main().sizePx() == own && function.changes == notified + 1,
            "上限放宽后应回到自己的字号并重画");
        leader.cellChanged(letter, 60, 150);
        follower.capMainAt(null);
        check(follower.params().main().sizePx() == own, "取消上限后应回到自己的字号");
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
