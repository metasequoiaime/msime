import app.msime.android.CandidateScrollAnchor;

/** 输入带来的新一代回到开头，晚到的换代保持位置。 */
public final class CandidateScrollAnchorSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        CandidateScrollAnchor anchor = new CandidateScrollAnchor();
        check(anchor.moved(4, 8, 0), "第一张列表从头开始");
        check(!anchor.moved(4, 8, 0), "同一代重画（释义、读音）不动");
        check(anchor.moved(4, 8, 1), "翻页回到开头");
        check(anchor.moved(4, 9, 1), "没标记的换代当作输入");

        anchor.markLate(4, 10);
        check(anchor.keepsPosition(4, 10, 1), "晚到的这一代保持位置");
        check(!anchor.keepsPosition(4, 11, 1), "别的一代不算");
        check(!anchor.moved(4, 10, 1), "停顿重排、在线候选、放出尾部不拉回开头");
        check(anchor.moved(4, 11, 1), "之后的输入照常回到开头");

        anchor.markLate(4, 12);
        check(!anchor.keepsPosition(4, 12, 2), "换了页就不是同一张列表");
        check(anchor.moved(4, 12, 2), "晚到但换页时回到开头");

        anchor.markLate(5, 1);
        check(anchor.moved(5, 1, 2), "换了会话回到开头");

        anchor.markLate(5, 2);
        anchor.forget();
        check(!anchor.keepsPosition(5, 2, 2), "没有视图之后标记失效");
        check(anchor.moved(5, 2, 2), "重新出现的列表从头开始");
        java.util.List<String> before = java.util.List.of(
            CandidateScrollAnchor.key("你好", 0), CandidateScrollAnchor.key("拟好", 0), CandidateScrollAnchor.key("你号", 0));
        check(CandidateScrollAnchor.follow(before, 1, before) == 1, "候选没变时留在同一个位置");
        java.util.List<String> cloud = java.util.List.of(CandidateScrollAnchor.key("你好", 0),
            CandidateScrollAnchor.key("你好呀", 2), CandidateScrollAnchor.key("拟好", 0), CandidateScrollAnchor.key("你号", 0));
        check(CandidateScrollAnchor.follow(before, 1, cloud) == 2, "云候选插在前面时跟着往后挪一格");
        check(CandidateScrollAnchor.follow(before, 2, java.util.List.of(CandidateScrollAnchor.key("你号", 0),
            CandidateScrollAnchor.key("你好", 0))) == 0, "重排后找到同一个候选");
        check(CandidateScrollAnchor.follow(before, 1, java.util.List.of(CandidateScrollAnchor.key("你好", 0))) == -1,
            "眼前的候选不在了就回到开头");
        check(CandidateScrollAnchor.follow(before, 5, cloud) == -1, "没有可对齐的候选");
        check(!CandidateScrollAnchor.key("你好", 0).equals(CandidateScrollAnchor.key("你好", 2)),
            "同一个词来自云端算另一个候选");
        System.out.println("Android candidate scroll anchor passed");
    }
}
