/**
 * 背单词 request, as `msime_client_vocabulary_review` reads it.
 *
 * The native side parses this with `deny_unknown_fields` and no default for `directory`, `resources`, `day` or `action`, and answers a document missing any of them with `invalid vocabulary review request` before it looks at the action. `resources` was made required when 背单词 reached every host (#632) and this host never sent it, so every load, answer and import failed and no book — bundled, imported or from a wordbook pack — ever reached the page. The shape is built here, apart from the page, so a test can hold it to the fields the native side requires.
 */

/** One 背单词 action from the shared page, forwarded unchanged. */
export interface VocabularyAction {
  operation: string;
  word?: string;
  known?: boolean;
  wordbook?: string;
  new_per_day?: number;
  session_limit?: number;
  name?: string;
  text?: string;
}

export interface VocabularyRequest {
  directory: string;
  /** 内置词书所在的资源根目录：`wordbooks/` 与 `engine/` 同级，引擎资源目录本身要通过锁文件校验，不能多放东西。必须是绝对路径；没有随包词书时只列导入的书。 */
  resources: string;
  day: string;
  action: VocabularyAction;
  /** 插件目录：其中的单词本插件列进书目。只在声明了 `wordbook_packs` 的形态（2in1）上传，手机不传。 */
  plugins?: string;
}

export class VocabularyReviewRequest {
  /**
   * The request for `action` on `day`.
   *
   * `stateDirectory` holds the review progress and the imported books, and its `plugins/` the installed packs; `resources` is the root the bundled books would sit under. `wordbookPacks` is the form factor's `wordbook_packs` capability, the same answer the page is given: a form factor without it lists no wordbook packs.
   */
  static build(
    stateDirectory: string,
    resources: string,
    day: string,
    action: VocabularyAction,
    wordbookPacks: boolean,
  ): VocabularyRequest {
    const request: VocabularyRequest = {
      directory: stateDirectory,
      resources: resources,
      day: day,
      action: action,
    };
    if (wordbookPacks) {
      request.plugins = `${stateDirectory}/plugins`;
    }
    return request;
  }
}
