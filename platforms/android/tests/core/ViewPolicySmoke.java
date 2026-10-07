import app.msime.android.ViewPolicy;

public final class ViewPolicySmoke {
    public static void main(String[] arguments) {
        checkRejectsUnsupported(ViewPolicy::setCentered, "centered");
        checkRejectsUnsupported(ViewPolicy::setCenteredVertically, "centered vertically");
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
