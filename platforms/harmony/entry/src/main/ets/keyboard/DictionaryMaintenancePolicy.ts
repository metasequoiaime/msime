/** 需要 Engine 独占维护窗口的词库操作。 */
const MUTATING_OPERATIONS: string[] = ["edit", "import", "retry", "dismiss_failure", "reset"];

/**
 * The operations that go to the queue instead of the Engine.
 *
 * Importing a file and downloading an explicitly selected cloud entry are requests whose timing
 * the user chooses freely: they are as likely to do it with the keyboard up as with it down, and
 * "dictionary maintenance busy" is not an answer to "add these words". The queued route writes a
 * file the keyboard drains in bounded idle turns between Engine sessions.
 *
 * Ordinary settings edits still go directly to the Engine: the user is watching the list beside
 * that form, and sending those through a queue would leave it unchanged until an idle turn.
 */
const QUEUED_OPERATIONS: string[] = ["import_personal", "queue_edit"];

export interface DictionaryMaintenanceDecision {
  maintenance: boolean;
  allowed: boolean;
  /** The request goes to the personal-dictionary queue rather than to the Engine. */
  queued: boolean;
  error: string;
}

/**
 * Keeps the session-level safety rule independent from NAPI and ArkUI.
 *
 * Reads can share a live Engine session. Mutations may recreate an idle session, but must be
 * rejected while the user is composing so dictionary management never discards in-progress input.
 */
export class DictionaryMaintenancePolicy {
  static decide(operation: string, composing: boolean): DictionaryMaintenanceDecision {
    const queued: boolean = QUEUED_OPERATIONS.indexOf(operation) >= 0;
    // A queued request touches its own file and never the Engine, so it neither needs the
    // maintenance window nor has any reason to be refused mid-composition.
    const maintenance: boolean = !queued && MUTATING_OPERATIONS.indexOf(operation) >= 0;
    return {
      maintenance,
      queued,
      allowed: !maintenance || !composing,
      error: maintenance && composing ? "dictionary maintenance busy" : "",
    };
  }
}
