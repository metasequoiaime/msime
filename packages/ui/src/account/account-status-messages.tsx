export interface AccountStatusMessagesProps {
  error?: string;
  notice?: string;
  noticeClassName?: string;
}

import { ErrorAlert } from "../core/error-alert";
import { StatusMessage } from "../core/status-message";

/** Shared error and success messages used by account surfaces. */
export function AccountStatusMessages({
  error,
  notice,
  noticeClassName = "notice",
}: AccountStatusMessagesProps) {
  return (
    <>
      {error && <ErrorAlert>{error}</ErrorAlert>}
      {notice && (
        <StatusMessage role="status" className={noticeClassName}>
          {notice}
        </StatusMessage>
      )}
    </>
  );
}
