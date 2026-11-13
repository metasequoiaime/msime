import fs from '@ohos.file.fs';

/** 每次读写的块大小。资源里最大的词库有一百多 MB，块太小系统调用次数太多，太大又白占内存。 */
const CHUNK_BYTES: number = 1024 * 1024;

/**
 * 只用 read 和 write 拷贝文件。键盘进程里的文件拷贝一律走这里，不用 `@ohos.file.fs` 的拷贝接口。
 *
 * 输入法处于基础访问模式时，系统给键盘进程加了 seccomp 系统调用白名单，里面没有 `sendfile`。`fs.copyFileSync` 底层是 libc++ 的 `filesystem::copy_file`，用的正是 `sendfile`，白名单外的调用不会返回错误，而是让内核以 SIGSYS 直接杀掉进程。键盘在 `onCreate` 暂存资源时就会这样被杀，系统反复拉起、反复被杀，表现为键盘不弹出、日志里没有任何报错（2026-11-13 在 JUY-AL50 上抓到的 cppcrash：`SIGSYS(SYS_SECCOMP) syscall number is 71`）。设置应用跑在主进程，不受这个限制，但 `StagedResources` 两边共用，所以同样走这里。
 */
export class FileCopy {
  /** 把 `from` 的内容写进 `to`，`to` 已存在时截断重写。 */
  static copy(from: string, to: string): void {
    const input = fs.openSync(from, fs.OpenMode.READ_ONLY);
    try {
      const output = fs.openSync(to, fs.OpenMode.CREATE | fs.OpenMode.WRITE_ONLY | fs.OpenMode.TRUNC);
      try {
        const buffer: ArrayBuffer = new ArrayBuffer(CHUNK_BYTES);
        for (;;) {
          const read: number = fs.readSync(input.fd, buffer);
          if (read <= 0) {
            break;
          }
          let written: number = 0;
          while (written < read) {
            const chunk: ArrayBuffer = written === 0 ? buffer : buffer.slice(written, read);
            const wrote: number = fs.writeSync(output.fd, chunk, { length: read - written });
            if (wrote <= 0) {
              throw new Error(`short write copying ${from}`);
            }
            written += wrote;
          }
        }
      } finally {
        fs.closeSync(output);
      }
    } finally {
      fs.closeSync(input);
    }
  }
}
