package app.msime.android;

/** Shared display rule for account and cloud clipboard device names. */
public final class DeviceNamePolicy {
    private DeviceNamePolicy() {}

    public static String display(String name) {
        return name == null || name.isEmpty() ? "未知设备" : name;
    }
}
