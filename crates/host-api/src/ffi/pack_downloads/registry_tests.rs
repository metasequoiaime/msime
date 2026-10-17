//! 测试里资源包的登记表按线程分开：并行跑的 `one_task_per_pack_and_cancel_reaches_the_running_one` 和 `install_from_files_validates_the_list_and_names_a_missing_file` 都会登记 `voice-runtime`，前者还发「全部取消」，共用一张表时两者谁先谁后决定了谁失败。

use super::*;
use std::sync::mpsc;

#[test]
fn another_threads_registration_and_cancel_all_do_not_reach_this_one() {
    let mine = Task::register(ResourcePack::VoiceRuntime).unwrap();
    let (done, finished) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let other = std::thread::spawn(move || {
        // 另一个测试同时登记同一个资源包、再发「全部取消」。
        let theirs = Task::register(ResourcePack::VoiceRuntime);
        // SAFETY: null 和 0 表示取消本进程（这里是本线程的登记表）里的全部安装。
        unsafe { msime_client_resource_pack_cancel(std::ptr::null(), 0) };
        done.send(theirs.is_ok()).unwrap();
        released.recv().unwrap();
        drop(theirs);
    });
    assert!(
        finished.recv().unwrap(),
        "the other thread found this thread's voice-runtime registered"
    );
    assert!(
        !mine.cancel.load(Ordering::Relaxed),
        "the other thread's cancel-all reached this thread's install"
    );
    release.send(()).unwrap();
    other.join().unwrap();
    drop(mine);
    assert!(!resource_pack_tasks()
        .lock()
        .unwrap()
        .contains_key("voice-runtime"));
}
