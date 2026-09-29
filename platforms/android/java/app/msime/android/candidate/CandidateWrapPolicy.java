package app.msime.android;

/** Pure row-allocation policy shared by the Android candidate flow layout and JVM tests. */
public final class CandidateWrapPolicy {
    private CandidateWrapPolicy() {}

    static boolean shouldWrap(int occupiedWidth, int childWidth, int availableWidth, int spacing) {
        return occupiedWidth > 0
            && (long) occupiedWidth + spacing + childWidth > availableWidth;
    }

    public static int[] rows(int availableWidth, int spacing, int[] childWidths) {
        if (availableWidth < 0 || spacing < 0 || childWidths == null)
            throw new IllegalArgumentException("Invalid candidate wrap dimensions");
        int[] rows = new int[childWidths.length];
        int row = 0;
        int occupied = 0;
        for (int index = 0; index < childWidths.length; index++) {
            int width = childWidths[index];
            if (width < 0) throw new IllegalArgumentException("Invalid candidate width");
            if (shouldWrap(occupied, width, availableWidth, spacing)) {
                row++;
                occupied = width;
            } else {
                occupied = occupied == 0 ? width : occupied + spacing + width;
            }
            rows[index] = row;
        }
        return rows;
    }
}
