<script lang="ts">
  import BlockRenderer from '../renderer/BlockRenderer.svelte';
  import type { Context, ActionHandler } from '../lib/types/huml';

  interface Props {
    currentScreen: any;
    context: Context;
    onAction?: ActionHandler;
  }

  let { currentScreen, context, onAction }: Props = $props();

  // Handle actions from blocks
  function handleAction(action: string, params?: any): void {
    if (onAction) {
      onAction(action, params);
    }
  }

  // Handle state changes - route to ViewerApp for permission checking
  function handleStateChange(key: string, value: any): void {
    // Viewers can change: user_content_doc, collaborative_doc, submissions_doc
    // ViewerApp.handleSetState will validate document permissions
    if (onAction) {
      onAction('setState', { stateUpdates: { [key]: value } });
    }
  }
</script>

<div class="screen-renderer">
  {#if currentScreen}
    <div class="screen" style={currentScreen.css}>
      {#if currentScreen.blocks && currentScreen.blocks.length > 0}
        {#each currentScreen.blocks as block (block.id || Math.random())}
          <BlockRenderer
            {block}
            {context}
            onAction={handleAction}
            onStateChange={handleStateChange}
          />
        {/each}
      {:else}
        <div class="empty-screen">
          <p>No content to display</p>
        </div>
      {/if}
    </div>
  {:else}
    <div class="no-screen">
      <p>No screen selected</p>
    </div>
  {/if}
</div>

<style>
  .screen-renderer {
    width: 100%;
    height: 100%;
    overflow: auto;
  }

  .screen {
    width: 100%;
    min-height: 100%;
  }

  .empty-screen,
  .no-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    color: #666;
    font-size: 14px;
  }
</style>
