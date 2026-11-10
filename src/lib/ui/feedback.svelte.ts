import { toast as sonnerToast } from "svelte-sonner";
export type ToastOptions = { actionLabel: string; onAction: () => void };
const feedbackId = "mia-feedback";
export function toast(message: string, action?: ToastOptions) {
  return sonnerToast(message, {
    id: feedbackId,
    duration: Infinity,
    closeButton: true,
    action: action
      ? { label: action.actionLabel, onClick: () => action.onAction() }
      : undefined,
  });
}
export function dismissToast() {
  sonnerToast.dismiss(feedbackId);
}
