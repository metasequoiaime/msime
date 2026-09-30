export interface CommunityErrorAlertProps {
  message: string;
  signInRequired?: boolean;
  onLogin?: () => void;
}

export function CommunityErrorAlert({
  message,
  signInRequired = false,
  onLogin,
}: CommunityErrorAlertProps) {
  return (
    <p role="alert" className="error">
      {message}
      {signInRequired && onLogin && (
        <>
          {" "}
          <button type="button" className="secondary" onClick={onLogin}>
            去登录
          </button>
        </>
      )}
    </p>
  );
}
