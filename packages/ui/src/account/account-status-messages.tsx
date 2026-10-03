export interface AccountStatusMessagesProps {
  error?: string;
  notice?: string;
  noticeClassName?: string;
}

/** Shared error and success messages used by account surfaces. */
export function AccountStatusMessages({
  error,
  notice,
  noticeClassName = "notice",
}: AccountStatusMessagesProps) {
  return (
    <>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {notice && (
        <p role="status" className={noticeClassName}>
          {notice}
        </p>
      )}
    </>
  );
}
