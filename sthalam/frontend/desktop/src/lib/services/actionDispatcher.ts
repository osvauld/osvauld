/**
 * Action Dispatcher Service
 *
 * Handles all action dispatching including built-in and custom actions.
 */

import type {
  BuiltInAction,
  NavigateParams,
  OpenModalParams,
  CloseModalParams,
  SetStateParams
} from '../types/huml';
import { navigateTo } from './navigationService.svelte';
import { openModal, closeModal } from './modalService.svelte';

/**
 * Custom action handler type
 */
export type CustomActionHandler = (action: string, params?: any) => void | Promise<void>;

/**
 * Registered custom action handlers
 */
const customHandlers = new Map<string, CustomActionHandler>();

/**
 * Global state change handler (for setState action)
 */
let stateChangeHandler: ((field: string, value: any) => void) | null = null;

/**
 * Register a custom action handler
 *
 * @param action Action name
 * @param handler Handler function
 *
 * @example
 * ```typescript
 * registerAction('publishPost', async (action, params) => {
 *   const { postId } = params;
 *   await api.publishPost(postId);
 *   alert('Post published!');
 * });
 * ```
 */
export function registerAction(action: string, handler: CustomActionHandler): void {
  customHandlers.set(action, handler);
}

/**
 * Unregister a custom action handler
 */
export function unregisterAction(action: string): void {
  customHandlers.delete(action);
}

/**
 * Set the global state change handler
 * This is called by setState action
 */
export function setStateChangeHandler(handler: (field: string, value: any) => void): void {
  stateChangeHandler = handler;
}

/**
 * Dispatch an action
 *
 * Handles both built-in actions (navigate, openModal, closeModal, setState)
 * and custom actions registered via registerAction().
 *
 * @param action Action name
 * @param params Action parameters
 *
 * @example
 * ```typescript
 * // Built-in navigation
 * dispatchAction('navigate', { screen: 'posts' });
 *
 * // Built-in modal
 * dispatchAction('openModal', { modal: 'confirm-delete', data: { postId: 123 } });
 *
 * // Custom action
 * dispatchAction('publishPost', { postId: 123 });
 * ```
 */
export async function dispatchAction(action: string, params?: any): Promise<void> {
  // Handle built-in actions
  switch (action as BuiltInAction) {
    case 'navigate': {
      const navParams = params as NavigateParams;
      if (!navParams?.screen) {
        console.error('[ActionDispatcher] navigate requires screen parameter');
        return;
      }
      navigateTo(navParams.screen, navParams);
      return;
    }

    case 'openModal': {
      const modalParams = params as OpenModalParams;
      if (!modalParams?.modal) {
        console.error('[ActionDispatcher] openModal requires modal parameter');
        return;
      }
      openModal(modalParams.modal, modalParams.data);
      return;
    }

    case 'closeModal': {
      const modalParams = params as CloseModalParams;
      if (!modalParams?.modal) {
        console.error('[ActionDispatcher] closeModal requires modal parameter');
        return;
      }
      closeModal(modalParams.modal);
      return;
    }

    case 'setState': {
      const stateParams = params as SetStateParams;
      if (!stateParams?.field) {
        console.error('[ActionDispatcher] setState requires field parameter');
        return;
      }
      if (!stateChangeHandler) {
        console.error('[ActionDispatcher] No state change handler registered');
        return;
      }
      stateChangeHandler(stateParams.field, stateParams.value);
      return;
    }
  }

  // Check for custom action handler
  const handler = customHandlers.get(action);
  if (handler) {
    try {
      await handler!(action, params);
    } catch (error) {
      console.error(`[ActionDispatcher] Error in custom action '${action}':`, error);
    }
    return;
  }

  // Unknown action
  console.warn(`[ActionDispatcher] Unknown action '${action}' with params:`, params);
}

/**
 * Check if an action is registered (built-in or custom)
 */
export function isActionRegistered(action: string): boolean {
  const builtInActions: BuiltInAction[] = ['navigate', 'openModal', 'closeModal', 'setState'];
  return builtInActions.includes(action as BuiltInAction) || customHandlers.has(action);
}

/**
 * Get all registered custom action names
 */
export function getRegisteredActions(): string[] {
  return Array.from(customHandlers.keys());
}

/**
 * Clear all custom action handlers
 */
export function clearActions(): void {
  customHandlers.clear();
}
