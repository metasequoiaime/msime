package app.msime.android;

import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

/** Android 宿主共用的线程创建策略。 */
public final class ThreadPolicy {
    private ThreadPolicy() {}

    /** 创建使用固定名称且不立即启动的线程，daemon 状态沿用调用线程。 */
    public static Thread namedThread(String name, Runnable runnable) {
        return new Thread(runnable, name);
    }

    /** 创建使用固定名称并保留平台默认线程属性的线程工厂。 */
    public static ThreadFactory namedFactory(String name) {
        ThreadFactory factory = Executors.defaultThreadFactory();
        return runnable -> {
            Thread thread = factory.newThread(runnable);
            thread.setName(name);
            return thread;
        };
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
