package app.msime.android;

import java.util.Objects;

/** Guards an asynchronous save from replacing a newer preference snapshot. */
public final class PreferencesSavePolicy {
    private PreferencesSavePolicy() {}

    public static boolean shouldApplyResponse(long currentRevision, long savedRevision) {
        return currentRevision < 0 || savedRevision >= currentRevision;
    }

    public static boolean isCurrentOperation(long operation, long currentOperation,
                                             long targetSession, long currentSession,
                                             String targetDirectory, String currentDirectory) {
        return operation == currentOperation && targetSession == currentSession
            && Objects.equals(targetDirectory, currentDirectory);
    }

    public static boolean isCurrentOperation(long operation, long currentOperation) {
        return operation == currentOperation;
    }
}
