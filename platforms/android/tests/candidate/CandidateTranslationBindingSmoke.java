import app.msime.android.CandidateTranslationBinding;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.Queue;

/** 账号译文缓存的绑定在后台核对：用假的 worker、主线程、时钟和缓存驱动。 */
public final class CandidateTranslationBindingSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class Fakes {
        final Queue<Runnable> worker = new ArrayDeque<>();
        final Queue<Runnable> main = new ArrayDeque<>();
        final List<String> bound = new ArrayList<>();
        final List<Boolean> checked = new ArrayList<>();
        String account = "1:a";
        String cacheKey;
        boolean entries;
        long now = 1_000;
        boolean reject;
        int reads;

        CandidateTranslationBinding binding() {
            return new CandidateTranslationBinding(
                () -> { reads++; return account; },
                task -> {
                    if (reject) return false;
                    worker.add(task);
                    return true;
                },
                main::add, () -> now,
                new CandidateTranslationBinding.Cache() {
                    @Override public boolean bindTo(String key) {
                        bound.add(key);
                        boolean same = java.util.Objects.equals(cacheKey, key);
                        if (same) return key != null;
                        cacheKey = key;
                        entries = false;
                        return false;
                    }
                    @Override public boolean hasEntries() { return entries; }
                },
                checked::add);
        }

        void runWorker() {
            Runnable task;
            while ((task = worker.poll()) != null) task.run();
        }

        void runMain() {
            Runnable task;
            while ((task = main.poll()) != null) task.run();
        }
    }

    public static void main(String[] args) {
        Fakes f = new Fakes();
        CandidateTranslationBinding binding = f.binding();
        check(!binding.usable(), "进程刚起来时还没核对，不用缓存");

        binding.ensureFresh();
        check(f.worker.size() == 1 && f.reads == 0, "核对排到 worker 上，主线程不读绑定");
        binding.ensureFresh();
        check(f.worker.size() == 1, "核对还在路上时不重复排");
        f.runWorker();
        check(f.reads == 1 && !binding.usable(), "回到主线程之前仍不可用");
        f.runMain();
        check(binding.usable() && f.bound.equals(List.of("1:a")), "核对完成：缓存绑到这个账号，可以用");
        check(f.checked.equals(List.of(false)), "第一次核对不需要去掉已画的译文");

        f.entries = true;
        f.now += CandidateTranslationBinding.MAX_AGE_MILLIS - 1;
        binding.ensureFresh();
        check(f.worker.isEmpty(), "核对过的绑定在有效期内不再读");
        f.now += 2;
        binding.ensureFresh();
        check(f.worker.size() == 1 && binding.usable(), "过了有效期在后台再核对，期间照常用缓存");
        f.runWorker();
        f.runMain();
        check(binding.usable() && f.entries && f.checked.get(1) == Boolean.FALSE, "账号没变：缓存留着");

        f.account = "2:b";
        f.now += CandidateTranslationBinding.MAX_AGE_MILLIS + 1;
        binding.ensureFresh();
        f.runWorker();
        f.runMain();
        check(f.bound.get(f.bound.size() - 1).equals("2:b") && !f.entries, "换了账号：缓存换绑定并清空");
        check(f.checked.get(f.checked.size() - 1) == Boolean.TRUE, "清空时通知主线程去掉画面上的旧译文");
        check(binding.usable(), "新账号的缓存（空的）可以用、可以发请求");

        binding.reset();
        check(!binding.usable(), "键盘显示、换输入框时先停用缓存");
        binding.ensureFresh();
        f.runWorker();
        binding.reset();
        f.runMain();
        check(!binding.usable(), "reset 之前发出的核对回来时作废");
        binding.ensureFresh();
        f.runWorker();
        f.runMain();
        check(binding.usable(), "reset 之后的核对照常生效");

        f.account = null;
        binding.invalidate();
        check(!binding.usable() && f.worker.size() == 1, "请求前后对不上：停用并马上重新核对");
        f.runWorker();
        f.runMain();
        check(!binding.usable() && f.bound.get(f.bound.size() - 1) == null,
            "provider 不可达时证明不了缓存属于谁：解绑，不用缓存");

        f.account = "2:b";
        binding.ensureFresh();
        f.worker.clear();
        f.now += 500;
        binding.ensureFresh();
        check(f.worker.isEmpty(), "核对可能还在排队");
        f.now += 600;
        binding.ensureFresh();
        check(f.worker.size() == 1, "worker 悄悄丢掉的核对一秒后重发");
        f.runWorker();
        f.runMain();
        check(binding.usable(), "重发的核对生效");

        f.reject = true;
        binding.reset();
        binding.ensureFresh();
        f.reject = false;
        binding.ensureFresh();
        check(f.worker.size() == 1, "worker 拒收后下一张列表马上重试");
        f.runWorker();
        f.runMain();

        check(binding.holds("2:b") && !binding.holds("1:a") && !binding.holds(null),
            "worker 上的请求核对只认当前账号");
        System.out.println("Android candidate translation binding passed");
    }
}
