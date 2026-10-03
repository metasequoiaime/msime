export interface AccountStatusMessagesProps {
  error?: string;
  notice?: string;
  noticeClassName?: string;
}

import { ErrorAlert } from "../core/error-alert";

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
        <p role="status" className={noticeClassName}>
          {notice}
        </p>
      )}
    </>
  );
}
