use super::*;

pub fn duplicate(lock: &ScopedFileLock) -> File {
    lock._file.try_clone().unwrap()
}

pub fn assert_finished_scope_releases_lock<T>(held: T, inherited: File, path: &Path) {
    let contender = open_private_lock_file(path).unwrap();
    assert!(!try_exclusive(&contender).unwrap());
    drop(held);
    assert!(
        try_exclusive(&contender).unwrap(),
        "持锁作用域结束后不能由旧描述符副本继续占锁"
    );
    drop(inherited);
    let next = open_private_lock_file(path).unwrap();
    assert!(!try_exclusive(&next).unwrap());
    unlock(&contender).unwrap();
    assert!(try_exclusive(&next).unwrap());
    unlock(&next).unwrap();
}
