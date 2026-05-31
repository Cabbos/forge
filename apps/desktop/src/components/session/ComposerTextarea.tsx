import * as React from "react";

interface ComposerTextareaProps {
  disabled: boolean;
  hasChips: boolean;
  placeholder: string;
  value: string;
  onChange: React.ChangeEventHandler<HTMLTextAreaElement>;
  onCompositionEnd: React.CompositionEventHandler<HTMLTextAreaElement>;
  onCompositionStart: React.CompositionEventHandler<HTMLTextAreaElement>;
  onKeyDown: React.KeyboardEventHandler<HTMLTextAreaElement>;
}

const ComposerTextarea = React.forwardRef<HTMLTextAreaElement, ComposerTextareaProps>(function ComposerTextarea({
  disabled,
  hasChips,
  onChange,
  onCompositionEnd,
  onCompositionStart,
  onKeyDown,
  placeholder,
  value,
}, ref) {
  return (
    <div data-testid="composer-textarea-wrap" className="forge-composer-textarea-wrap">
      <textarea
        ref={ref}
        value={value}
        onChange={onChange}
        onKeyDown={onKeyDown}
        onCompositionStart={onCompositionStart}
        onCompositionEnd={onCompositionEnd}
        placeholder={placeholder}
        rows={1}
        disabled={disabled}
        className="forge-composer-textarea"
        style={{ paddingTop: hasChips ? "4px" : undefined }}
      />
    </div>
  );
});

ComposerTextarea.displayName = "ComposerTextarea";

export { ComposerTextarea };
