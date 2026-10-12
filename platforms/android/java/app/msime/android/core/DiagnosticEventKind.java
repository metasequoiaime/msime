package app.msime.android;

/** P19 diagnostic event whitelist shared by local input logs and uploaded snapshots. */
public enum DiagnosticEventKind {
    KEY_DOWN("key_down"),
    KEY_UP("key_up"),
    CANDIDATE_SHOWN("candidate_shown"),
    CANDIDATE_SELECTED("candidate_selected"),
    COMMIT("commit"),
    BACKSPACE("backspace"),
    PANEL_OPEN("panel_open"),
    PANEL_CLOSE("panel_close"),
    IME_START("ime_start"),
    IME_FINISH("ime_finish");

    private final String wire;

    DiagnosticEventKind(String wire) { this.wire = wire; }

    public String wire() { return wire; }

    /** Unknown kinds are dropped when reading a diagnostic bundle. */
    public static DiagnosticEventKind fromWire(String value) {
        for (DiagnosticEventKind kind : values()) {
            if (kind.wire.equals(value)) return kind;
        }
        return null;
    }
}
