/**
 * Modal Service - Svelte 5 Runes
 *
 * Manages modal dialog state (open/close, data passing).
 */

import type { ModalState } from '../types/huml';

/**
 * Modal state (Svelte 5 rune)
 */
let modalState = $state<ModalState>({});

/**
 * Open a modal
 *
 * @param modalName Modal name (matches modal block name)
 * @param data Optional data to pass to modal
 */
export function openModal(modalName: string, data?: any): void {
  modalState[modalName] = {
    visible: true,
    data
  };

  console.log(`[Modal] Opened: ${modalName}`, data);
}

/**
 * Close a modal
 *
 * @param modalName Modal name
 * @param clearData If true, clear modal data (default: true)
 */
export function closeModal(modalName: string, clearData = true): void {
  if (modalState[modalName]) {
    modalState[modalName] = {
      visible: false,
      data: clearData ? undefined : modalState[modalName].data
    };
  }

  console.log(`[Modal] Closed: ${modalName}`);
}

/**
 * Toggle a modal
 */
export function toggleModal(modalName: string, data?: any): void {
  const isVisible = modalState[modalName]?.visible ?? false;

  if (isVisible) {
    closeModal(modalName);
  } else {
    openModal(modalName, data);
  }
}

/**
 * Close all modals
 */
export function closeAllModals(): void {
  for (const modalName in modalState) {
    modalState[modalName] = {
      visible: false,
      data: undefined
    };
  }

  console.log('[Modal] Closed all modals');
}

/**
 * Check if any modal is open
 */
export function isAnyModalOpen(): boolean {
  return Object.values(modalState).some((modal) => modal.visible);
}

/**
 * Export reactive modal state
 * Use in Svelte components like: modals.isVisible('confirm-delete')
 */
export const modals = {
  isVisible(modalName: string): boolean {
    return modalState[modalName]?.visible ?? false;
  },
  getData(modalName: string): any {
    return modalState[modalName]?.data;
  },
  get state() {
    return modalState;
  }
};
