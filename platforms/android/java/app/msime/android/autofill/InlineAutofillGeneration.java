package app.msime.android;

/** 输入框和响应代次共同隔离异步填充视图，不保存任何填充内容。 */
public final class InlineAutofillGeneration {
    private long generation;
    private boolean active;

    public void startInput() { generation++; active = true; }
    public void finishInput() { generation++; active = false; }
    public void showInput() { active = true; }
    public long nextResponse() { return ++generation; }
    public boolean accepts(long expected) { return active && generation == expected; }
}
