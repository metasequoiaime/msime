package app.msime.android;

import java.util.concurrent.ThreadFactory;

/** Android 宿主共用的线程创建策略。 */
public final class ThreadPolicy {
    private ThreadPolicy() {}

    private static Thread namedThread(String name, Runnable runnable) {
        return new Thread(runnable, name);
    }

    /** 创建并立即启动固定名称的线程，daemon 状态沿用调用线程。 */
    public static Thread startNamedThread(String name, Runnable runnable) {
        Thread thread = namedThread(name, runnable);
        thread.start();
        return thread;
    }

    /** 创建使用固定名称且不会阻止进程退出的未启动线程。 */
    public static Thread namedDaemonThread(String name, Runnable runnable) {
        Thread thread = namedThread(name, runnable);
        thread.setDaemon(true);
        return thread;
    }

    /** 创建使用固定名称且不会阻止进程退出的线程工厂。 */
    public static ThreadFactory namedDaemonFactory(String name) {
        return runnable -> namedDaemonThread(name, runnable);
    }
}
