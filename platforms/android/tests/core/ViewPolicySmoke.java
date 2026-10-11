import app.msime.android.ViewPolicy;

public final class ViewPolicySmoke {
    public static void main(String[] arguments) {
        checkRejectsUnsupported(ViewPolicy::setCentered, "centered");
        checkRejectsUnsupported(ViewPolicy::setCenteredVertically, "centered vertically");
        // 可见性判断允许传 null（视图还没建好），都返回 false；!isGone(null) 因此为真，调用方改可见性前必须自己判空。
        if (ViewPolicy.isVisible(null)) throw new AssertionError("isVisible(null) must be false");
        if (ViewPolicy.isGone(null)) throw new AssertionError("isGone(null) must be false");
        System.out.println("ViewPolicy smoke passed");
    }

    private static void checkRejectsUnsupported(java.util.function.Consumer<android.view.View> action,
            String name) {
        try {
            action.accept(null);
        } catch (IllegalArgumentException expected) {
            return;
        } catch (StackOverflowError error) {
            throw new AssertionError(name + " policy recurses instead of rejecting an unsupported view", error);
        }
        throw new AssertionError(name + " policy accepted an unsupported view");
    }
}
