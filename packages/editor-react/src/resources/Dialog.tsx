import { Dialog } from "@base-ui/react/dialog";
import type { ReactNode } from "react";
import { usePortalContainer } from "../chrome/portal";
import { Icon } from "../icons/Icon";
import "./resources.css";
export function ResourceDialog({
  title,
  description,
  width = 660,
  children,
  onClose,
  className = "",
}: {
  title: string;
  description?: string;
  width?: number;
  children: ReactNode;
  onClose(): void;
  className?: string;
}) {
  const container = usePortalContainer();
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Dialog.Portal container={container}>
        <Dialog.Backdrop className="vc-resource-backdrop" />
        <Dialog.Popup className={`vc-resource-dialog ${className}`} style={{ width }}>
          <header className="vc-resource-header">
            <Dialog.Title>{title}</Dialog.Title>
            <Dialog.Close className="vc-iconbutton" aria-label="Close dialog">
              <Icon name="x" />
            </Dialog.Close>
          </header>
          {description ? (
            <Dialog.Description className="vc-resource-description">
              {description}
            </Dialog.Description>
          ) : null}
          {children}
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
export function ResourceButton({
  children,
  onClick,
  disabled = false,
  primary = false,
}: {
  children: ReactNode;
  onClick(): void;
  disabled?: boolean;
  primary?: boolean;
}) {
  return (
    <button
      type="button"
      className="vc-resource-button"
      data-primary={primary ? "" : undefined}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
