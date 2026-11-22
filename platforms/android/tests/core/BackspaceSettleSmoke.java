import app.msime.android.BackspaceSettle;

public final class BackspaceSettleSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        long timeout = BackspaceSettle.TIMEOUT_MS;

        // 空闲、没有待确认的写入：立刻发；发出后到回声之前按住新触摸，回声到了放开。
        BackspaceSettle settle = new BackspaceSettle();
        check(!settle.holding(0));
        check(settle.deadline() == -1);
        check(settle.request(1000, false, 0));
        settle.sent(1000);
        check(settle.holding(1010));
        check(settle.deadline() == 1000 + timeout);
        // 连删的下一次排在未确认的这一次后面。
        check(!settle.request(1020, false, 0));
        check(settle.echo());
        check(!settle.holding(1021));
        check(!settle.echo());
        check(settle.request(1030, false, 0));

        // 编辑器不回报（空输入框）：超时放行，之后不再按住。
        settle.sent(2000);
        check(!settle.expire(2000 + timeout - 1));
        check(settle.holding(2000 + timeout - 1));
        check(settle.expire(2000 + timeout));
        check(!settle.holding(2000 + timeout));
        check(!settle.expire(2000 + timeout + 1));
        // 超时之后迟到的回报不再算确认。
        check(!settle.echo());

        // 先上屏、再按退格：上屏的回声没到，退格先等；回声到了才发。
        BackspaceSettle commitFirst = new BackspaceSettle();
        check(!commitFirst.request(3000, true, 5));
        check(commitFirst.holding(3001));
        check(commitFirst.deadline() == 3000 + timeout);
        check(!commitFirst.ready(3010, true));
        check(commitFirst.ready(3012, false));
        check(!commitFirst.holding(3012));
        commitFirst.sent(3012);
        check(commitFirst.echo());

        // 编辑器没为某次写入回报，预期一直挂着：最多等一次超时，同一批预期不再让后面的退格等。
        BackspaceSettle stale = new BackspaceSettle();
        check(!stale.request(4000, true, 7));
        check(!stale.ready(4000 + timeout - 1, true));
        check(stale.ready(4000 + timeout, true));
        stale.sent(4000 + timeout);
        check(stale.echo());
        check(stale.request(4200, true, 7));
        // 又有新的写入（预期总数变了）：重新等它的回声。
        check(!stale.request(4300, true, 8));
        check(stale.ready(4300 + timeout, true));

        // 换输入框时放弃在路上的退格，不再按住。
        BackspaceSettle restarted = new BackspaceSettle();
        restarted.sent(5000);
        restarted.reset();
        check(!restarted.holding(5001));
        check(!restarted.echo());
        check(!restarted.request(5002, true, 1));
        restarted.reset();
        check(!restarted.holding(5003));
        check(!restarted.ready(5003, false));

        System.out.println("BackspaceSettle smoke passed");
    }
}
