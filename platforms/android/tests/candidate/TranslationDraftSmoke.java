import app.msime.android.TranslationDraft;

/** 用户改动原文或语言后，迟到的成功与失败都不能复活旧译文。 */
public final class TranslationDraftSmoke {
    public static void main(String[] args) {
        TranslationDraft draft = new TranslationDraft();
        long first = draft.invalidate();
        long edited = draft.invalidate();
        check(!draft.complete(first, "old"), "过期回答不得显示");
        check(!draft.current(first), "过期失败不得显示");
        check(draft.complete(edited, "synthetic translation"), "当前回答应可预览");
        check(draft.consume().equals("synthetic translation"), "手动插入领取一次");
        check(draft.consume().isEmpty(), "重复插入不得复用结果");
        check(!draft.complete(edited, "duplicate"), "已领取请求不得再次发布");
        long next = draft.invalidate();
        check(!draft.complete(next, null) && !draft.complete(next, "  "), "空回答不可插入");
        check(draft.answer().isEmpty(), "失败不显示旧回答");
        System.out.println("translation draft passed");
    }
    private static void check(boolean value, String description) {
        if (!value) throw new AssertionError(description);
    }
}
