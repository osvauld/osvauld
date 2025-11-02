<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { contentStore } from '../shared/loro/contentStore';
  import { evaluateCEL } from '../utils/celEvaluator';
  import type { TreeNode } from 'loro-crdt';
  import BlockRenderer from '../lib/blocks/BlockRenderer.svelte';

  // Publisher UI State (reactive)
  let publisherUIState = $state<Record<string, any>>({});
  let computedValues = $state<Record<string, any>>({});
  let content = $state<Record<string, any>>({});
  let screens = $state<any[]>([]);
  let currentScreenId = $state<string>('');

  // Subscriptions
  let contentUnsubscribe: (() => void) | null = null;
  let templateUnsubscribe: (() => void) | null = null;

  onMount(() => {
    initializePublisher();
  });

  onDestroy(() => {
    if (contentUnsubscribe) contentUnsubscribe();
    if (templateUnsubscribe) templateUnsubscribe();
  });

  /**
   * Initialize Publisher Mode
   */
  function initializePublisher() {
    console.log('🚀 [PublisherApp] Initializing publisher mode...');

    const docs = loroCoordinator.getDocuments();
    const templateDoc = docs.templateDoc;

    // 1. Load publisher state definition
    const publisherStateMap = templateDoc.getMap('publisherState');
    const stateDefinition: Record<string, any> = {};
    for (const [key, value] of publisherStateMap.entries()) {
      stateDefinition[key] = value;
    }
    publisherUIState = { ...stateDefinition };
    console.log('📊 [PublisherApp] Initialized state:', publisherUIState);

    // 2. Load computed expressions
    const publisherComputedMap = templateDoc.getMap('publisherComputed');
    const computedExpressions: Record<string, string> = {};
    for (const [key, value] of publisherComputedMap.entries()) {
      computedExpressions[key] = value as string;
    }

    // 3. Load publisher screens
    loadPublisherScreens();

    // 4. Subscribe to content changes
    contentUnsubscribe = contentStore.subscribe(() => {
      content = contentStore.getContent();
      evaluateComputed(computedExpressions);
    });

    // 5. Subscribe to template changes
    templateUnsubscribe = templateDoc.subscribe(() => {
      loadPublisherScreens();
    });

    // 6. Initial content load
    content = contentStore.getContent();

    // 7. Initial computed evaluation
    evaluateComputed(computedExpressions);
  }

  /**
   * Load publisher screens from template
   */
  function loadPublisherScreens() {
    const docs = loroCoordinator.getDocuments();
    const publisherTree = docs.templateDoc.getTree('publisherScreens');

    if (!publisherTree) {
      console.warn('⚠️ [PublisherApp] No publisher screens found');
      return;
    }

    const screenNodes = publisherTree.roots();
    screens = [];

    for (const node of screenNodes) {
      const screen = nodeToScreen(node);
      screens.push(screen);

      // Set entry point as current screen
      if (screen.isEntryPoint || screens.length === 1) {
        currentScreenId = screen.id;
      }
    }

    console.log('📺 [PublisherApp] Loaded screens:', screens.length);
  }

  /**
   * Convert TreeNode to screen object
   */
  function nodeToScreen(node: TreeNode): any {
    const screen: any = {
      id: node.data.get('id') || `screen-${node.id}`,
      type: node.data.get('type'),
      name: node.data.get('name'),
      isEntryPoint: node.data.get('isEntryPoint'),
      css: node.data.get('css'),
      blocks: []
    };

    // Load child blocks
    const children = node.children();
    for (const child of children) {
      screen.blocks.push(nodeToBlock(child));
    }

    return screen;
  }

  /**
   * Convert TreeNode to block object
   */
  function nodeToBlock(node: TreeNode): any {
    const block: any = {};

    // Copy all properties from node data
    for (const [key, value] of node.data.entries()) {
      block[key] = value;
    }

    // Process children
    const children = node.children();
    if (children.length > 0) {
      block.blocks = children.map(child => nodeToBlock(child));
    }

    return block;
  }

  /**
   * Evaluate computed expressions
   */
  function evaluateComputed(expressions: Record<string, string>) {
    const context = {
      ...publisherUIState,
      content,
      size: (arr: any[]) => arr?.length || 0,
      length: (str: string) => str?.length || 0
    };

    const newComputed: Record<string, any> = {};

    for (const [key, expression] of Object.entries(expressions)) {
      try {
        newComputed[key] = evaluateCEL(expression, context);
      } catch (error) {
        console.error(`❌ [PublisherApp] Failed to evaluate computed.${key}:`, error);
        newComputed[key] = undefined;
      }
    }

    computedValues = newComputed;
  }

  /**
   * Handle publisher actions
   */
  function handleAction(action: string, params: any = {}) {
    console.log('🎯 [PublisherApp] Handling action:', action, params);

    switch (action) {
      case 'publishPost':
        handlePublishPost(params);
        break;

      case 'updatePost':
        handleUpdatePost(params);
        break;

      case 'deletePost':
        handleDeletePost(params);
        break;

      case 'setState':
        handleSetState(params);
        break;

      case 'navigate':
        handleNavigate(params);
        break;

      default:
        console.warn('⚠️ [PublisherApp] Unknown action:', action);
    }
  }

  /**
   * Publish a new post
   */
  function handlePublishPost(params: any) {
    const { formData } = params;
    const postContent = formData?.content || publisherUIState.newPostContent;

    if (!postContent) {
      console.warn('⚠️ [PublisherApp] No content to publish');
      return;
    }

    // Set publishing state
    publisherUIState = { ...publisherUIState, isPublishing: true };

    // Publish the post
    const postId = contentStore.publishPost(postContent);

    // Clear form and reset state
    publisherUIState = {
      ...publisherUIState,
      newPostContent: '',
      isPublishing: false
    };

    console.log('✅ [PublisherApp] Published post:', postId);
  }

  /**
   * Update an existing post
   */
  function handleUpdatePost(params: any) {
    const { postId, content } = params;

    if (!postId) {
      console.warn('⚠️ [PublisherApp] No post ID provided');
      return;
    }

    const success = contentStore.updatePost(postId, { content });

    if (success) {
      // Clear editing state
      publisherUIState = {
        ...publisherUIState,
        editingPostId: ''
      };
    }
  }

  /**
   * Delete a post
   */
  function handleDeletePost(params: any) {
    const { postId } = params;

    if (!postId) {
      console.warn('⚠️ [PublisherApp] No post ID provided');
      return;
    }

    const success = contentStore.deletePost(postId);

    if (success) {
      console.log('✅ [PublisherApp] Deleted post:', postId);
    }
  }

  /**
   * Update UI state
   */
  function handleSetState(params: any) {
    const { stateUpdates } = params;

    if (!stateUpdates) return;

    // Evaluate state updates (might contain expressions)
    const context = {
      ...publisherUIState,
      content,
      ...computedValues
    };

    const updates: Record<string, any> = {};

    for (const [key, value] of Object.entries(stateUpdates)) {
      if (typeof value === 'string' && value.startsWith('{{') && value.endsWith('}}')) {
        // Evaluate expression
        const expression = value.slice(2, -2);
        try {
          updates[key] = evaluateCEL(expression, context);
        } catch (error) {
          console.error(`❌ [PublisherApp] Failed to evaluate state update for ${key}:`, error);
          updates[key] = value;
        }
      } else {
        updates[key] = value;
      }
    }

    publisherUIState = {
      ...publisherUIState,
      ...updates
    };

    console.log('📝 [PublisherApp] State updated:', updates);
  }

  /**
   * Navigate to a different screen
   */
  function handleNavigate(params: any) {
    const { targetScreen } = params;

    if (!targetScreen) return;

    currentScreenId = targetScreen;
    console.log('🗺️ [PublisherApp] Navigated to screen:', targetScreen);
  }

  /**
   * Get the current screen to render
   */
  $: currentScreen = screens.find(s => s.id === currentScreenId) || screens[0];

  /**
   * Prepare context for expression evaluation
   */
  $: expressionContext = {
    ...publisherUIState,
    content,
    ...computedValues,
    mode: 'publisher'
  };
</script>

<div class="publisher-app">
  {#if currentScreen}
    <div class="screen" style={currentScreen.css}>
      {#each currentScreen.blocks as block}
        <BlockRenderer
          {block}
          context={expressionContext}
          onAction={handleAction}
          onStateChange={(key, value) => {
            publisherUIState = { ...publisherUIState, [key]: value };
          }}
        />
      {/each}
    </div>
  {:else}
    <div class="no-screens">
      <h2>Publisher Mode</h2>
      <p>No screens defined for publisher mode.</p>
      <p>Please define publisher screens in your template.</p>
    </div>
  {/if}
</div>

<style>
  .publisher-app {
    width: 100%;
    height: 100%;
    overflow: auto;
  }

  .screen {
    width: 100%;
    min-height: 100vh;
  }

  .no-screens {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    text-align: center;
    color: #666;
  }

  .no-screens h2 {
    margin-bottom: 1rem;
    color: #333;
  }
</style>