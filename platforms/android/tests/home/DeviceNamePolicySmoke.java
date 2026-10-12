import app.msime.android.DeviceNamePolicy;

public final class DeviceNamePolicySmoke {
    public static void main(String[] arguments) {
        check("未知设备".equals(DeviceNamePolicy.display(null)), "missing device name uses fallback");
        check("未知设备".equals(DeviceNamePolicy.display("")), "empty device name uses fallback");
        check("合成手机".equals(DeviceNamePolicy.display("合成手机")), "device name is preserved");
        System.out.println("DeviceNamePolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
