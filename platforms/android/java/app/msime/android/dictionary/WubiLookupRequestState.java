package app.msime.android;

/** 五笔查码的输入、结果与请求代次；页面重建时只保留已经完成的结果。 */
public final class WubiLookupRequestState<T> {
    private String text = "";
    private T result;
    private boolean busy;
    private long generation;

    public String text() { return text; }
    public T result() { return result; }
    public boolean busy() { return busy; }

    /** 输入变化后旧结果不再对应当前文字，未完成的请求也不能挡住新查询。 */
    public boolean edit(String next) {
        if (text.equals(next)) return false;
        text = next;
        result = null;
        busy = false;
        generation++;
        return true;
    }

    /** 返回新请求的代次；空白输入或当前请求未结束时返回 0。 */
    public long begin() {
        if (busy || !WubiCodeLookupPolicy.searchable(text)) return 0;
        busy = true;
        return ++generation;
    }

    /** 只有属于当前文字与当前视图的请求可以更新结果。 */
    public boolean complete(long requestGeneration, T answer) {
        if (!busy || requestGeneration != generation) return false;
        result = answer;
        busy = false;
        return true;
    }

    /** HostTask 不会向已销毁的视图交回结果，因而必须解除旧请求的忙碌状态。 */
    public void viewDestroyed() {
        if (!busy) return;
        busy = false;
        generation++;
    }
}
