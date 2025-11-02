<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { contentStore } from '../shared/loro/contentStore';
  import { evaluateExpression } from '../lib/humlEvaluator';
  import type { TreeNode } from 'loro-crdt';
  import BlockRenderer from '../shared/blocks/BlockRenderer.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';

  // Publisher UI State - Local snapshots of CRDT state for reactive UI
  let publisherUIState = $state<Record<string, any>>({});
  let computedValues = $state<Record<string, any>>({});
  let content = $state<Record<string, any>>({});
  let screens = $state<any[]>([]);
  let currentScreenId = $state<string>('');

  // Computed expressions from template
  let computedExpressions = $state<Record<string, string>>({});

  // CRDT subscriptions
  let contentUnsubscribe: (() => void) | null = null;
  let templateUnsubscribe: (() => void) | null = null;
  let animationFrameId: number | null = null;

  // FPS tracking
  let lastFrameTime = 0;
  let frameCount = 0;
  let fps = 0;
  let fpsHistory: number[] = [];

  onMount(() => {
    initializePublisher();

    // Start animation loop for time-based expressions
    function animate(timestamp: number) {
      // Calculate FPS more frequently (every 250ms)
      frameCount++;
      if (timestamp - lastFrameTime >= 250) {
        const currentFps = Math.round((frameCount * 1000) / (timestamp - lastFrameTime));
        fpsHistory.push(currentFps);
        if (fpsHistory.length > 4) fpsHistory.shift();

        // Average last 4 readings for smoother display
        fps = Math.round(fpsHistory.reduce((a, b) => a + b, 0) / fpsHistory.length);

        frameCount = 0;
        lastFrameTime = timestamp;
      }

      publisherUIState = {
        ...publisherUIState,
        time: (publisherUIState.time || 0) + 0.05, // Increment time each frame
        fps
      };
      animationFrameId = requestAnimationFrame(animate);
    }
    animationFrameId = requestAnimationFrame(animate);
  });

  onDestroy(() => {
    if (contentUnsubscribe) contentUnsubscribe();
    if (templateUnsubscribe) templateUnsubscribe();
    if (animationFrameId) cancelAnimationFrame(animationFrameId);
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

    // 2. Load computed expressions into state
    const publisherComputedMap = templateDoc.getMap('publisherComputed');
    const expressions: Record<string, string> = {};
    for (const [key, value] of publisherComputedMap.entries()) {
      expressions[key] = value as string;
    }
    computedExpressions = expressions;

    // 3. Load publisher screens
    loadPublisherScreens();

    // 4. Subscribe to content changes
    contentUnsubscribe = contentStore.subscribe(() => {
      content = contentStore.getContent();
      // Ensure posts and comments arrays exist
      if (!content.posts) {
        content.posts = [];
      }
      if (!content.comments) {
        content.comments = [];
      }
      evaluateComputed();
    });

    // 5. Subscribe to template changes
    templateUnsubscribe = templateDoc.subscribe(() => {
      loadPublisherScreens();
    });

    // 6. Initial content load
    content = contentStore.getContent();

    // 7. Initialize posts and comments arrays if they don't exist
    if (!content.posts) {
      content.posts = [];
      console.log('📦 [PublisherApp] Initialized empty posts array');
    }
    if (!content.comments) {
      content.comments = [];
      console.log('📦 [PublisherApp] Initialized empty comments array');
    }

    // 8. Initial computed evaluation
    evaluateComputed();
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
    if (screens.length > 0) {
      console.log('📋 [PublisherApp] First screen:', screens[0]);
      console.log('📦 [PublisherApp] First screen blocks:', screens[0]?.blocks?.length);
      if (screens[0]?.blocks?.[0]) {
        const firstBlock = screens[0].blocks[0];
        console.log('🔍 [PublisherApp] First block type:', firstBlock.type);
        console.log('🔍 [PublisherApp] First block has nested blocks:', firstBlock.blocks?.length || 0);
        if (firstBlock.blocks && firstBlock.blocks.length > 0) {
          console.log('🎯 [PublisherApp] Nested block types:', firstBlock.blocks.map((b: any) => b.type));
        }
      }
    }
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
    const children = node.children() || [];
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

    // Debug: Log button blocks
    if (block.type === 'nav-button' && block.action === 'deletePost') {
      console.log('🔍 [PublisherApp] Delete button properties:', Object.keys(block));
      console.log('🔍 [PublisherApp] Delete button full:', block);
    }

    // Process children
    const children = node.children() || [];
    if (children.length > 0) {
      block.blocks = children.map(child => nodeToBlock(child));
    }

    return block;
  }

  /**
   * Evaluate computed expressions (with dependency resolution via multiple passes)
   */
  function evaluateComputed() {
    console.log('🧮 [PublisherApp] Evaluating computed expressions:', computedExpressions);

    let newComputed: Record<string, any> = {};
    let prevComputed: Record<string, any> = {};
    let maxPasses = 5;  // Prevent infinite loops
    let pass = 0;

    // Keep evaluating until values stabilize (handles dependencies between computed values)
    do {
      prevComputed = { ...newComputed };
      pass++;

      for (const [key, expression] of Object.entries(computedExpressions)) {
        try {
          // Build context with previously computed values
          const context = {
            ...publisherUIState,
            content,
            ...newComputed,  // Include already-computed values!
            size: (arr: any[]) => arr?.length || 0,
            length: (str: string) => str?.length || 0
          };

          // Strip {{}} if present, then evaluate
          let cleanExpression = expression;
          if (typeof expression === 'string') {
            const trimmed = expression.trim();
            if (trimmed.startsWith('{{') && trimmed.endsWith('}}')) {
              cleanExpression = trimmed.slice(2, -2).trim();
            }
          }

          if (pass === 1) {
            console.log(`🧮 [PublisherApp] Computing ${key}:`, cleanExpression);
          }

          const result = evaluateExpression(cleanExpression, context);
          newComputed[key] = result;

          if (pass === 1) {
            console.log(`✅ [PublisherApp] ${key} =`, result);
          }
        } catch (error) {
          if (pass === 1) {
            console.error(`❌ [PublisherApp] Failed to evaluate computed.${key}:`, error);
          }
          newComputed[key] = undefined;
        }
      }

      // Check if values changed
      const changed = Object.keys(newComputed).some(key => newComputed[key] !== prevComputed[key]);
      if (!changed) break;

    } while (pass < maxPasses);

    console.log('📊 [PublisherApp] Final computed values:', newComputed, `(${pass} passes)`);
    computedValues = newComputed;
  }

  /**
   * Handle publisher actions
   */
  function handleAction(action: string, params: any = {}) {
    console.log('🎯 [PublisherApp] Handling action:', action, params);
    console.log('🔍 [PublisherApp] Current state:', publisherUIState);

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

      case 'addComment':
        handleAddComment(params);
        break;

      case 'deleteComment':
        handleDeleteComment(params);
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
  async function handlePublishPost(params: any) {
    console.log('📝 [PublisherApp] handlePublishPost called with:', params);

    const { formData } = params;
    const postContent = formData?.content || publisherUIState.newPostContent;

    console.log('📝 [PublisherApp] Post content:', postContent);

    if (!postContent) {
      console.warn('⚠️ [PublisherApp] No content to publish');
      return;
    }

    // Set publishing state
    publisherUIState = { ...publisherUIState, isPublishing: true };

    // Publish the post
    console.log('📝 [PublisherApp] Calling contentStore.publishPost...');
    const postId = contentStore.publishPost(postContent);
    console.log('✅ [PublisherApp] Published post:', postId);

    // Update content from store to get the new post
    content = contentStore.getContent();
    console.log('📦 [PublisherApp] Updated content, posts count:', content.posts?.length);

    // Save to backend (persist the change)
    await saveToBackend();

    // Clear form and reset state
    publisherUIState = {
      ...publisherUIState,
      newPostContent: '',
      isPublishing: false
    };

    // Trigger re-evaluation of computed values
    evaluateComputed();

    console.log('✅ [PublisherApp] Post published and state cleared');
  }

  /**
   * Save current state to backend
   */
  async function saveToBackend() {
    try {
      // First, check what's in the contentMap
      const contentMap = loroCoordinator.getContentMap();
      const postsInLoro = contentMap.get('posts');
      console.log('🔍 [PublisherApp] Posts in Loro before commit:', postsInLoro);

      // Commit all Loro documents
      const docs = loroCoordinator.getDocuments();
      docs.contentDoc.commit();
      docs.templateDoc.commit();
      docs.userContentDoc.commit();
      docs.uiStateDoc.commit();

      console.log('💾 [PublisherApp] Documents committed, saving to backend...');

      // Then save to backend
      const { dataState } = await import('../state/data.svelte');
      if (dataState.currentResourceId) {
        await dataState.saveCurrentResource(dataState.currentResourceId);
        console.log('✅ [PublisherApp] Saved to backend successfully');
      }
    } catch (error) {
      console.error('❌ [PublisherApp] Failed to save:', error);
    }
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
  async function handleDeletePost(params: any) {
    const { postId } = params;

    if (!postId) {
      console.warn('⚠️ [PublisherApp] No post ID provided');
      return;
    }

    const success = contentStore.deletePost(postId);

    if (success) {
      console.log('✅ [PublisherApp] Deleted post:', postId);

      // Update content and save
      content = contentStore.getContent();
      await saveToBackend();
      evaluateComputed();
    }
  }

  /**
   * Add a comment (top-level or reply)
   */
  async function handleAddComment(params: any) {
    console.log('💬 [PublisherApp] handleAddComment params:', params);

    // Extract parameters - content might come from formData or directly
    const postId = params.postId;
    const parentCommentId = params.parentCommentId;
    const commentContent = params.formData?.content || params.content;

    console.log('💬 [PublisherApp] Extracted:', { postId, parentCommentId, commentContent });

    if (!postId || !commentContent) {
      console.warn('⚠️ [PublisherApp] Missing postId or content', { postId, commentContent });
      return;
    }

    const commentId = contentStore.addComment(postId, commentContent, parentCommentId || null);
    console.log('✅ [PublisherApp] Added comment:', commentId);

    // Clear the appropriate form field
    if (parentCommentId) {
      // Was a reply
      publisherUIState = {
        ...publisherUIState,
        replyContent: '',
        replyingTo: ''
      };
    } else {
      // Was a top-level comment
      publisherUIState = {
        ...publisherUIState,
        commentContent: ''
      };
    }

    // Update content and save
    content = contentStore.getContent();
    await saveToBackend();
    evaluateComputed();
  }

  /**
   * Delete a comment (and all its replies)
   */
  async function handleDeleteComment(params: any) {
    const { commentId } = params;

    if (!commentId) {
      console.warn('⚠️ [PublisherApp] No comment ID provided');
      return;
    }

    const success = contentStore.deleteComment(commentId);

    if (success) {
      console.log('✅ [PublisherApp] Deleted comment:', commentId);

      // Update content and save
      content = contentStore.getContent();
      await saveToBackend();
      evaluateComputed();
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
          updates[key] = evaluateExpression(expression, context);
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
  let currentScreen = $derived(screens.find(s => s.id === currentScreenId) || screens[0]);

  /**
   * Prepare context for expression evaluation
   */
  let expressionContext = $derived((() => {
    // Pre-compute comments trees for all posts (CEL can't call JS functions)
    const commentsTrees: Record<string, any[]> = {};
    const posts = content.posts || [];

    for (const post of posts) {
      commentsTrees[post.id] = contentStore.getCommentsTree(post.id);
    }

    return {
      ...publisherUIState,
      content,
      ...computedValues,
      mode: 'publisher',
      commentsTrees
    };
  })());
</script>

<div class="publisher-app">
  <div class="publisher-toolbar">
    <span class="toolbar-title">Publisher Mode</span>
    <ModeSwitcher />
  </div>

  {#if currentScreen}
    <div class="screen-container">
      <div class="screen" style={currentScreen.css}>
        {#each currentScreen.blocks as block (block.id || Math.random())}
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
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .publisher-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    background: #1e1e1e;
    border-bottom: 1px solid #333;
  }

  .toolbar-title {
    font-size: 14px;
    font-weight: 500;
    color: #888;
  }

  .screen-container {
    flex: 1;
    overflow: auto;
  }

  .screen {
    width: 100%;
    min-height: 100%;
  }

  .no-screens {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    color: #666;
  }

  .no-screens h2 {
    margin-bottom: 1rem;
    color: #333;
  }
</style>