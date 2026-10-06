import { ActionButton } from "../core/action-button";
import { ErrorAlert } from "../core/error-alert";

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
    <ErrorAlert>
      {message}
      {signInRequired && onLogin && (
        <>
          {" "}
          <ActionButton action={onLogin} label="去登录" />
        </>
      )}
    </ErrorAlert>
  );
}
