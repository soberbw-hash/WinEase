import * as Dialog from "@radix-ui/react-dialog";
import { useRef } from "react";
export function SupportModal({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const origin = useRef<HTMLElement | null>(null);
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-panel support-modal"
          onOpenAutoFocus={() => {
            origin.current = document.activeElement as HTMLElement;
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            origin.current?.focus();
          }}
        >
          <Dialog.Title>赞助</Dialog.Title>
          <Dialog.Description>感谢支持</Dialog.Description>
          <img
            src="/donate-qr.png"
            alt="赞助收款码"
            className="support-modal__qr"
          />
          <Dialog.Close asChild>
            <button className="secondary-button">关闭</button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
