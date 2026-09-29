import app.msime.android.SpaceCursorMovement;

public final class SpaceCursorMovementSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        Object first = new Object();
        Object second = new Object();
        SpaceCursorMovement movement = new SpaceCursorMovement();
        movement.begin(10, first);
        check(movement.isActive());
        check(movement.advance(15, first, 12) == 0);
        check(movement.advance(34, first, 12) == 2);
        check(movement.advance(30, first, 12) == 0);
        check(movement.advance(22, first, 12) == -1);
        check(movement.advance(46, second, 12) == 0);
        check(!movement.isActive());
        check(movement.advance(58, first, 12) == 0);

        movement.begin(-20, second);
        check(movement.advance(-31, second, 12) == 0);
        check(movement.advance(-44, second, 12) == -2);
        movement.cancel();
        check(!movement.isActive());
        check(movement.advance(24, null, 12) == 0);

        movement.begin(Float.NaN, first);
        check(!movement.isActive());
        movement.begin(0, first);
        check(movement.advance(Float.POSITIVE_INFINITY, first, 12) == 0);
        check(!movement.isActive());
        movement.begin(0, first);
        check(movement.advance(4097, first, 12) == 0);
        check(!movement.isActive());
        movement.begin(0, first);
        check(movement.advance(12, first, 0) == 0);
        check(!movement.isActive());
        System.out.println("Android space cursor: accumulation, reversal and editor guards passed");
    }
}
