package app.msime.android;

/**
 * 候选条和竖排候选的滚动位置跟着谁走。用户自己的输入（打字、翻页、选拼音、换会话）带来的新一代从头开始；
 * 不是用户引起的换代（停顿重排、在线候选、放出暂扣的尾部）保持用户已经滚到的位置，内容晚到不把视线拉回开头。
 *
 * <p>晚到的那几条路径在换上新视图前调用 {@link #markLate}；没标记的换代一律当作输入，回到开头，所以漏标只会退回旧行为。
 */
public final class CandidateScrollAnchor {
    private long session = -1;
    private long generation = -1;
    private int page = -1;
    private long lateSession = Long.MIN_VALUE;
    private long lateGeneration = Long.MIN_VALUE;

    /** 记下这一代是晚到的结果换上的，不是用户的输入。 */
    public void markLate(long session, long generation) {
        lateSession = session;
        lateGeneration = generation;
    }

    /** 这一代是不是晚到的结果、且与上次定位的列表同一会话同一页：是的话保持已有的位置和已建的数量。 */
    public boolean keepsPosition(long session, long generation, int page) {
        return session == this.session && page == this.page
            && session == lateSession && generation == lateGeneration;
    }

    /** 画完这一代后调用：列表该回到开头时为真，并记下这一代。同一代重画（释义、读音）为假。 */
    public boolean moved(long session, long generation, int page) {
        if (session == this.session && generation == this.generation && page == this.page) return false;
        boolean keep = keepsPosition(session, generation, page);
        this.session = session;
        this.generation = generation;
        this.page = page;
        return !keep;
    }

    /**
     * 晚到的换代改了候选时，用户眼前的第一个候选应该留在原处：返回它（按文字和来源，{@code before.get(first)}）在新列表里的位置。
     * 停顿重排没变时就是同一个位置；云候选插在前面时往后挪一格；它不在新列表里（或没有可对齐的）时为 -1，列表回到开头。
     */
    public static int follow(java.util.List<String> before, int first, java.util.List<String> after) {
        if (first < 0 || first >= before.size()) return -1;
        String key = before.get(first);
        if (first < after.size() && key.equals(after.get(first))) return first;
        return after.indexOf(key);
    }

    /** {@link #follow} 比较用的键：同一个词来自不同来源（本地、云、AI）算不同的候选。 */
    public static String key(String text, int source) {
        return source + ":" + text;
    }

    /** 没有视图了：下一次出现的列表从头开始。 */
    public void forget() {
        session = -1;
        generation = -1;
        page = -1;
        lateSession = Long.MIN_VALUE;
        lateGeneration = Long.MIN_VALUE;
    }
}
