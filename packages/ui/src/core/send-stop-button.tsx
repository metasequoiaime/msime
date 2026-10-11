export interface SendStopButtonProps {
  className?: string;
  sending: boolean;
  sendDisabled?: boolean;
  onSend: () => void;
  onStop: () => void;
}

/** Composer action that keeps input focus while switching between send and stop. */
export function SendStopButton({
  className,
  sending,
  sendDisabled = false,
  onSend,
  onStop,
}: SendStopButtonProps) {
  return (
    <button
      type="button"
      className={className}
      aria-label={sending ? "停止" : "发送"}
      disabled={!sending && sendDisabled}
      onPointerDown={(event) => event.preventDefault()}
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => (sending ? onStop() : onSend())}
    >
      {sending ? (
        <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
          <rect width="14" height="14" rx="2" fill="currentColor" />
        </svg>
      ) : (
        <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true">
          <path
            d="M11 20V7.83l-5.59 5.59L4 12l8-8 8 8-1.41 1.41L13 7.83V20h-2Z"
            fill="currentColor"
          />
        </svg>
      )}
    </button>
  );
}
