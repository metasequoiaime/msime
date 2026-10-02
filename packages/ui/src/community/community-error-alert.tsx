import { ActionButton } from "../core/action-button";

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
          <ActionButton action={onLogin} label="去登录" />
        </>
      )}
    </p>
  );
}
