#pragma once

#ifndef _WIN32
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/file.h>
#include <unistd.h>
#if defined(__linux__)
#include <sys/syscall.h>
#endif

// 先在扫描范围外取得文件锁，再发布文件名；读取端只能在写入关闭或进程退出后取得锁。
// 发布不覆盖已有记录；展开栈时再次故障仍留下已经写好的摘要，不等待任何其它进程。
static inline int msime_crash_file_create(const char *path, const char *temporary) {
  const int file = open(temporary, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (file < 0) return -1;
  int published = -1;
  if (flock(file, LOCK_EX | LOCK_NB) == 0) {
#if defined(__APPLE__)
    published = renameatx_np(AT_FDCWD, temporary, AT_FDCWD, path, RENAME_EXCL);
#elif defined(__linux__) && defined(SYS_renameat2)
    // Android 的 SELinux 禁止硬链接；优先使用与共享 Rust 存储相同的不覆盖重命名。
    published = (int)syscall(SYS_renameat2, AT_FDCWD, temporary, AT_FDCWD, path, 1);
    if (published < 0 && (errno == ENOSYS || errno == EINVAL)) {
      published = link(temporary, path);
    }
#else
    published = link(temporary, path);
#endif
  }
  const int error = errno;
  // rename 已移走临时名；硬链接回退和失败路径都只删除本次独占创建的临时名。
  (void)unlink(temporary);
  if (published == 0) return file;
  close(file);
  errno = error;
  return -1;
}
#endif
