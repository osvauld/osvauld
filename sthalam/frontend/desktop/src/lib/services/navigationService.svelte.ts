/**
 * Navigation Service - Svelte 5 Runes
 *
 * Manages SPA navigation between screens with history support.
 */

import type { NavigationState } from '../types/huml';

/**
 * Navigation state (Svelte 5 rune)
 */
let navigationState = $state<NavigationState>({
  currentScreen: 'home',  // Default screen
  params: {},
  history: []
});

/**
 * Navigate to a screen
 *
 * @param screen Screen name
 * @param params Route parameters
 * @param replaceHistory If true, replace current entry instead of pushing
 */
export function navigateTo(
  screen: string,
  params: Record<string, any> = {},
  replaceHistory = false
): void {
  // Save current location to history (unless replacing)
  if (!replaceHistory) {
    navigationState.history = [
      ...navigationState.history,
      {
        screen: navigationState.currentScreen,
        params: navigationState.params
      }
    ];
  }

  // Update current screen and params
  navigationState.currentScreen = screen;
  navigationState.params = params;

  console.log(`[Navigation] → ${screen}`, params);
}

/**
 * Go back to previous screen
 */
export function goBack(): boolean {
  if (navigationState.history.length === 0) {
    console.warn('[Navigation] No history to go back to');
    return false;
  }

  const previous = navigationState.history[navigationState.history.length - 1];
  navigationState.history = navigationState.history.slice(0, -1);
  navigationState.currentScreen = previous.screen;
  navigationState.params = previous.params;

  console.log('[Navigation] ← Back');
  return true;
}

/**
 * Clear navigation history
 */
export function clearHistory(): void {
  navigationState.history = [];
  console.log('[Navigation] History cleared');
}

/**
 * Set initial screen
 */
export function setInitialScreen(screen: string, params: Record<string, any> = {}): void {
  navigationState.currentScreen = screen;
  navigationState.params = params;
  navigationState.history = [];
  console.log(`[Navigation] Initial screen: ${screen}`);
}

/**
 * Export reactive navigation state
 * Use in Svelte components like: navigation.currentScreen
 */
export const navigation = {
  get currentScreen() {
    return navigationState.currentScreen;
  },
  get params() {
    return navigationState.params;
  },
  get history() {
    return navigationState.history;
  },
  get canGoBack() {
    return navigationState.history.length > 0;
  }
};
