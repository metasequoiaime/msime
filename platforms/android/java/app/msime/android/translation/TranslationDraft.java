package app.msime.android;

/** 只在窗口内存保存的译文代次；编辑原文、语言变化和重复插入都撤销旧结果。 */
public final class TranslationDraft {
    private long generation;
    private String answer = "";
    public long invalidate() { answer = ""; return ++generation; }
    public boolean complete(long request, String text) {
        if (request != generation || text == null || text.isBlank()) return false;
        answer = text;
        return true;
    }
    public boolean current(long request) { return request == generation; }
    public String answer() { return answer; }
    public String consume() {
        String result = answer;
        invalidate();
        return result;
    }
}
