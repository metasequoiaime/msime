//! 单工作线程复用权重；最多一个执行任务和一个等待任务，空闲后释放模型。

use crate::{language_name, validate_text, Error};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, Weak};
use std::time::{Duration, Instant};

pub(crate) trait Model {
    fn translate(
        &self,
        text: &str,
        language: &str,
        deadline: Instant,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<String, Error>;
}

type Reply = mpsc::Sender<Result<String, Error>>;
struct Job {
    path: PathBuf,
    text: String,
    language: String,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    reply: Reply,
    epoch: u64,
}

enum Command {
    Translate(Job),
    Unload,
}

/// 一个进程只创建一个工作队列，宿主在后台线程等待结果。
/// 取消标记可由宿主的候选代次、失焦或关闭操作设置。
pub struct Worker {
    sender: mpsc::SyncSender<Command>,
    tokens: Mutex<Vec<Weak<AtomicBool>>>,
    epoch: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// 丢弃等待句柄会取消任务；超时也会取消底层加载和生成。
pub struct Pending {
    receiver: mpsc::Receiver<Result<String, Error>>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    finished: bool,
}

impl Pending {
    pub fn wait(mut self) -> Result<String, Error> {
        match self
            .receiver
            .recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
        {
            Ok(result) => {
                self.finished = true;
                result
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Err(Error::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Unavailable),
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.finished {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
}

impl Worker {
    pub(crate) fn with_loader<M: Model + 'static>(
        idle: Duration,
        mut load: impl FnMut(&Path, Instant, Arc<AtomicBool>) -> Result<M, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let epoch = Arc::new(AtomicU64::new(0));
        let worker_epoch = epoch.clone();
        let closed = Arc::new(AtomicBool::new(false));
        let worker_closed = closed.clone();
        let thread = std::thread::Builder::new()
            .name("local-translation".into())
            .spawn(move || {
                let mut cached: Option<(PathBuf, M)> = None;
                let mut loaded_epoch = 0;
                loop {
                    if worker_closed.load(Ordering::Acquire) {
                        break;
                    }
                    let command = match receiver.recv_timeout(idle) {
                        Ok(command) => command,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            cached = None;
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let current_epoch = worker_epoch.load(Ordering::Acquire);
                    if current_epoch != loaded_epoch {
                        cached = None;
                        loaded_epoch = current_epoch;
                    }
                    let Command::Translate(job) = command else {
                        cached = None;
                        continue;
                    };
                    let result = (|| {
                        check(&job, &worker_epoch)?;
                        if cached.as_ref().is_none_or(|(path, _)| path != &job.path) {
                            cached = None;
                            let model = load(&job.path, job.deadline, job.cancelled.clone())?;
                            check(&job, &worker_epoch)?;
                            cached = Some((job.path.clone(), model));
                        }
                        let result = cached.as_ref().ok_or(Error::Unavailable)?.1.translate(
                            &job.text,
                            &job.language,
                            job.deadline,
                            &|| {
                                job.cancelled.load(Ordering::Relaxed)
                                    || worker_epoch.load(Ordering::Acquire) != job.epoch
                            },
                        )?;
                        check(&job, &worker_epoch)?;
                        Ok(result)
                    })();
                    if worker_epoch.load(Ordering::Acquire) != job.epoch {
                        cached = None;
                    }
                    let _ = job.reply.send(result);
                }
            })
            .map_err(|_| Error::Unavailable)?;
        Ok(Self {
            sender,
            tokens: Mutex::new(Vec::new()),
            epoch,
            closed,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn submit(
        &self,
        path: PathBuf,
        text: String,
        language: String,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Pending, Error> {
        validate_text(&text)?;
        if language_name(&language).is_none() || !path.is_absolute() {
            return Err(Error::InvalidInput);
        }
        let (reply, receiver) = mpsc::channel();
        let mut tokens = self.tokens.lock().map_err(|_| Error::Unavailable)?;
        if self.closed.load(Ordering::Acquire) {
            return Err(Error::Unavailable);
        }
        tokens.retain(|token| token.strong_count() > 0);
        let job = Job {
            path,
            text,
            language,
            deadline,
            cancelled: cancelled.clone(),
            reply,
            epoch: self.epoch.load(Ordering::Acquire),
        };
        check(&job, &self.epoch)?;
        self.sender
            .try_send(Command::Translate(job))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => Error::Busy,
                mpsc::TrySendError::Disconnected(_) => Error::Unavailable,
            })?;
        tokens.push(Arc::downgrade(&cancelled));
        Ok(Pending {
            receiver,
            cancelled,
            deadline,
            finished: false,
        })
    }

    /// 进程退出前取消全部任务，等待工作线程释放模型。只在退出线程调用。
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        self.unload();
        if let Ok(mut thread) = self.thread.lock() {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// 关闭功能、切换模型和删除权重前使所有在途任务失效并释放缓存。
    pub fn unload(&self) {
        if let Ok(tokens) = self.tokens.lock() {
            self.epoch.fetch_add(1, Ordering::AcqRel);
            for token in tokens.iter().filter_map(Weak::upgrade) {
                token.store(true, Ordering::Relaxed);
            }
            // 队列满时执行中的任务结束也会观察到代次变化并卸载。
            let _ = self.sender.try_send(Command::Unload);
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn check(job: &Job, epoch: &AtomicU64) -> Result<(), Error> {
    if job.cancelled.load(Ordering::Relaxed) || epoch.load(Ordering::Acquire) != job.epoch {
        Err(Error::Cancelled)
    } else if Instant::now() >= job.deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}

#[cfg(feature = "runtime")]
impl Model for crate::Translator {
    fn translate(
        &self,
        text: &str,
        language: &str,
        deadline: Instant,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<String, Error> {
        self.translate(text, language, deadline, cancelled)
    }
}

#[cfg(feature = "runtime")]
impl Worker {
    pub fn new() -> Result<Self, Error> {
        Self::with_loader(Duration::from_secs(60), crate::Translator::load_cancellable)
    }

    /// 宿主在每次冷加载时解析并校验模型；解析器只运行于推理工作线程。
    pub fn with_model_resolver(
        mut resolve: impl FnMut(&Path, Instant, Arc<AtomicBool>) -> Result<PathBuf, Error>
            + Send
            + 'static,
    ) -> Result<Self, Error> {
        Self::with_loader(Duration::from_secs(60), move |root, deadline, cancelled| {
            let path = resolve(root, deadline, cancelled.clone())?;
            crate::Translator::load_cancellable(&path, deadline, cancelled)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    struct Fake {
        dropped: Arc<AtomicUsize>,
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl Model for Fake {
        fn translate(
            &self,
            text: &str,
            _: &str,
            _: Instant,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<String, Error> {
            if cancelled() {
                Err(Error::Cancelled)
            } else {
                Ok(text.to_owned())
            }
        }
    }
    fn submit(worker: &Worker) -> Pending {
        worker
            .submit(
                std::env::temp_dir().join("synthetic.gguf"),
                "合成文本".into(),
                "en".into(),
                Instant::now() + Duration::from_secs(2),
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap()
    }

    #[test]
    fn cached_model_is_reused_and_released_after_idle() {
        let loads = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let (l, d) = (loads.clone(), dropped.clone());
        let worker = Worker::with_loader(Duration::from_millis(200), move |_, _, _| {
            l.fetch_add(1, Ordering::SeqCst);
            Ok(Fake { dropped: d.clone() })
        })
        .unwrap();
        let pending = submit(&worker);
        let cancel = pending.cancelled.clone();
        assert_eq!(pending.wait().unwrap(), "合成文本");
        assert!(!cancel.load(Ordering::Relaxed));
        assert_eq!(submit(&worker).wait().unwrap(), "合成文本");
        assert_eq!(loads.load(Ordering::SeqCst), 1);
        let deadline = Instant::now() + Duration::from_secs(2);
        while dropped.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn queue_is_bounded_and_unload_cancels_loading_and_waiting_work() {
        let (started, began) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = Worker::with_loader(Duration::from_secs(60), move |_, _, cancel| {
            started.send(()).unwrap();
            released.recv().unwrap();
            if cancel.load(Ordering::Relaxed) {
                Err(Error::Cancelled)
            } else {
                Ok(Fake {
                    dropped: Arc::new(AtomicUsize::new(0)),
                })
            }
        })
        .unwrap();
        let first = submit(&worker);
        began.recv_timeout(Duration::from_secs(2)).unwrap();
        let second = submit(&worker);
        let overflow = worker.submit(
            std::env::temp_dir().join("synthetic.gguf"),
            "合成文本".into(),
            "en".into(),
            Instant::now() + Duration::from_secs(2),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(matches!(overflow, Err(Error::Busy)));
        worker.unload();
        release.send(()).unwrap();
        assert_eq!(first.wait(), Err(Error::Cancelled));
        assert_eq!(second.wait(), Err(Error::Cancelled));
        let next = submit(&worker);
        began.recv_timeout(Duration::from_secs(2)).unwrap();
        release.send(()).unwrap();
        assert_eq!(next.wait().unwrap(), "合成文本");
        worker.shutdown();
        assert!(worker.closed.load(Ordering::Acquire));
    }
}
