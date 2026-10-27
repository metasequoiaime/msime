/**
 * 「我的词条」交换的文件与原生调用：设置应用和键盘都用它。规则在 Rust（`msime_client_user_words_mirror`），什么时候调在 `UserWordsMirrorPolicy`，这里只读文件身份、发请求、读回复。
 */
import fs from '@ohos.file.fs';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from './KeyboardLog';
import { FileIdentity, UserWordsMirrorPolicy, UserWordsReply } from './UserWordsMirrorPolicy';

export class UserWordsMirror {
  /** 一个普通文件此刻的身份；不存在、是符号链接或者读不到时为 null。 */
  static identity(path: string): FileIdentity | null {
    try {
      const stat: fs.Stat = fs.lstatSync(path);
      if (!stat.isFile()) return null;
      return { size: stat.size, mtime: stat.mtime, ino: String(stat.ino) };
    } catch {
      return null;
    }
  }

  /** 本方 Engine 日志（含 WAL）的标记，日志一变它就变。 */
  static journalMark(stateDirectory: string): string {
    return UserWordsMirrorPolicy.mark(UserWordsMirrorPolicy.journalFiles(stateDirectory)
      .map((path: string): FileIdentity | null => UserWordsMirror.identity(path)));
  }

  /** 一个发布文件的标记。 */
  static fileMark(path: string): string {
    return UserWordsMirrorPolicy.mark([UserWordsMirror.identity(path)]);
  }

  /** 同步调用：只给键盘在建会话之前用。 */
  static run(request: string): UserWordsReply {
    try {
      return UserWordsMirror.report(UserWordsMirrorPolicy.parse(client.userWordsMirror(request)));
    } catch (error) {
      KeyboardLog.error(`user words mirror unavailable: ${describeError(error)}`);
      return UserWordsMirrorPolicy.parse('');
    }
  }

  /** 工作线程上的调用。 */
  static async runAsync(request: string): Promise<UserWordsReply> {
    try {
      return UserWordsMirror.report(UserWordsMirrorPolicy.parse(await client.userWordsMirrorAsync(request)));
    } catch (error) {
      KeyboardLog.error(`user words mirror unavailable: ${describeError(error)}`);
      return UserWordsMirrorPolicy.parse('');
    }
  }

  /** 只记数量，不记词。 */
  private static report(reply: UserWordsReply): UserWordsReply {
    if (!reply.ok) {
      KeyboardLog.warn(`user words not exchanged: ${reply.error}`);
    } else if (reply.captured + reply.applied + reply.rejected + reply.deferred + reply.heldDeletions > 0) {
      KeyboardLog.info(`user words exchanged: captured=${reply.captured} applied=${reply.applied} `
        + `rejected=${reply.rejected} deferred=${reply.deferred} held=${reply.heldDeletions} `
        + `words=${reply.words} theirs=${reply.theirs} published=${reply.published}`);
    }
    return reply;
  }
}
