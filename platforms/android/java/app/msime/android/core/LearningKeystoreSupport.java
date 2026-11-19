package app.msime.android;

/** 显式声明独立学习密钥槽；JNI 不向只实现旧 SessionStore 的代理查询新方法。 */
public interface LearningKeystoreSupport {
    boolean supportsLearningKeystore();
}
