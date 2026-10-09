/** One action sent to `msime_client_vocabulary_review`. */
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
  /** Absolute resource root containing the bundled wordbooks. */
  resources: string;
  day: string;
  action: VocabularyAction;
  plugins?: string;
}

export class VocabularyReviewRequest {
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
