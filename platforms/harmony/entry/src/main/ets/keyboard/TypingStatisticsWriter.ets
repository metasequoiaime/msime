import client from 'libmsimeclient.so';

/**
 * The typing-statistics calls that run on a TaskPool thread rather than the keyboard's UI thread.
 *
 * Kept in a file of their own, importing nothing but the native module, so the worker that runs them loads that and nothing else of the keyboard.
 */

/** Sends one `record_keys` request. The store takes a file lock and rewrites the whole document, which is not the UI thread's to wait for. */
@Concurrent
export function writeKeyPresses(request: string): string {
  return client.typingStatistics(request);
}

/** Sends one `record` (a commit), `record_voice` or `flush` request. Run on the session's one SequenceRunner, so commits reach the store in the order they were typed, which the active-time gaps are measured by, and a `flush` lands after every record queued before it. */
@Concurrent
export function writeTypingStatistics(request: string): string {
  return client.typingStatistics(request);
}

/** The statistics master switch: 1 on, 0 off or never written, -1 for an unreadable document. */
@Concurrent
export function readTypingStatisticsEnabled(directory: string): number {
  return client.typingStatisticsEnabled(directory);
}
