<script lang="ts">
/**
 * LinkBlock - Hyperlink with navigation support
 * SECURITY: External links ALWAYS open in system browser, never in-app
 */

import type { LinkBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';
import { interpolateCEL } from '../../lib/services/celEvaluator';
import { open } from '@tauri-apps/plugin-shell';

interface Props {
  block: LinkBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context, onAction }: Props = $props();

// Interpolate content
const content = $derived(interpolateCEL(block.content, context));

// Interpolate href if regular link
const href = $derived(block.href ? interpolateCEL(block.href, context) : undefined);

// Evaluate CSS
const styles = $derived(block.css ? interpolateCEL(block.css, context) : undefined);

// Handle click
async function handleClick(event: Event) {
  event.preventDefault(); // ALWAYS prevent default to avoid in-app navigation

  // Internal SPA navigation
  if (block.action === 'navigate' && block.params?.screen) {
    if (onAction) {
      onAction('navigate', {
        targetScreen: block.params.screen,
        ...block.params
      });
    }
    return;
  }

  // External links - ALWAYS open in system browser
  if (href) {
    try {
      await open(href);
    } catch (error) {
      console.error('[LinkBlock] Failed to open external link:', error);
    }
  }
}
</script>

<!-- SECURITY: No href attribute to prevent in-app navigation -->
<a
  style={styles}
  class={block.class}
  onclick={handleClick}
  role="button"
  tabindex="0"
>
  {@html content}
</a>
