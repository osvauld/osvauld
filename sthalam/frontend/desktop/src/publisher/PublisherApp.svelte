<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { evaluateCEL as evaluateExpression } from '../lib/services/celEvaluator';
  import { parseHUML } from '../lib/services/humlParser';
  import BlockRenderer from '../renderer/BlockRenderer.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';
  import { uploadVideo } from '../lib/services/videoService';

  // Publisher UI State - Local snapshots of CRDT state for reactive UI
  let publisherUIState = $state<Record<string, any>>({});
  let screens = $state<any[]>([]);
  let currentScreenId = $state<string>('');

  // Computed expressions from template
  let computedExpressions = $state<Record<string, string>>({});

  // Computed values - derived synchronously (no infinite loop since we removed time/fps/mouse)
  let computedValues = $derived(evaluateComputedValues());

  // CRDT subscriptions
  let templateUnsubscribe: (() => void) | null = null;

  onMount(() => {
    initializePublisher();
  });

  onDestroy(() => {
    if (templateUnsubscribe) templateUnsubscribe();
  });

  /**
   * Persist state changes to contentDoc automatically
   */
  $effect(() => {
    // Watch for changes to publisherUIState
    const stateToSave = publisherUIState;

    // Only save if we have state to persist
    if (Object.keys(stateToSave).length === 0) return;

    // Save to contentDoc (automatically debounced by Svelte)
    const stateMap = loroCoordinator.getStateMap();
    for (const [key, value] of Object.entries(stateToSave)) {
      // Only persist non-temporary fields (exclude _uploading, _error)
      if (!key.endsWith('_uploading') && !key.endsWith('_error')) {
        stateMap.set(key, value);
      }
    }
    loroCoordinator.getDocuments().contentDoc.commit();
  });

  /**
   * Initialize Publisher Mode
   */
  function initializePublisher() {
    console.log('🚀 [PublisherApp] Initializing publisher mode...');

    // Get HUML source and parse it
    const templateMap = loroCoordinator.getTemplateMap();
    const humlSource = templateMap.get('huml_source');

    if (!humlSource || typeof humlSource !== 'string') {
      console.warn('⚠️ [PublisherApp] No HUML source found');
      return;
    }

    try {
      const template = parseHUML(humlSource);

      // 1. Load publisher state definition from template
      const stateDefinition = template.documents?.publisherState || {};

      // Extract initial values from state schema
      const initialState: Record<string, any> = {};
      for (const [key, schema] of Object.entries(stateDefinition)) {
        if (typeof schema === 'object' && schema !== null && 'initial' in schema) {
          initialState[key] = (schema as any).initial;
        } else {
          initialState[key] = schema;
        }
      }

      // Load persisted state from contentDoc if exists
      const stateMap = loroCoordinator.getStateMap();
      const persistedState = stateMap.toJSON();

      // Merge persisted state with initial state (persisted takes precedence)
      publisherUIState = { ...initialState, ...persistedState };

      // Initialize videos as empty array if not defined (for list-based templates)
      if (!publisherUIState.videos) {
        publisherUIState.videos = [];
      }

      console.log('📊 [PublisherApp] Initialized state:', publisherUIState);
      console.log('💾 [PublisherApp] Loaded persisted state from contentDoc:', persistedState);

      // 2. Load computed expressions from template
      const expressions = template.documents?.publisherComputed || {};
      computedExpressions = expressions;
      console.log('🧮 [PublisherApp] Loaded computed expressions:', Object.keys(computedExpressions));
    } catch (error) {
      console.error('❌ [PublisherApp] Failed to parse HUML:', error);
    }

    // 3. Load publisher screens
    loadPublisherScreens();

    // 4. Subscribe to template changes (when HUML source changes)
    const templateDoc = loroCoordinator.getDocuments().templateDoc;
    templateUnsubscribe = templateDoc.subscribe(() => {
      // Re-initialize when template changes
      loadPublisherScreens();
    });
  }

  /**
   * Load publisher screens from template
   */
  function loadPublisherScreens() {
    // Get raw HUML from templateDoc
    const templateMap = loroCoordinator.getTemplateMap();
    const humlSource = templateMap.get('huml_source');

    if (!humlSource || typeof humlSource !== 'string') {
      console.warn('⚠️ [PublisherApp] No HUML source found in template');
      screens = [];
      return;
    }

    try {
      // Parse HUML in memory
      const template = parseHUML(humlSource);
      console.log('✅ [PublisherApp] Parsed template:', template);

      // Extract publisher screens from ui.publisher
      const publisherScreens = template.ui?.publisher || [];
      screens = publisherScreens;

      // Set entry point as current screen
      if (screens.length > 0) {
        const entryScreen = screens.find(s => s.isEntryPoint) || screens[0];
        currentScreenId = entryScreen.id || entryScreen.name || 'home';
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
    } catch (error) {
      console.error('❌ [PublisherApp] Failed to parse HUML:', error);
      screens = [];
    }
  }

  /**
   * Evaluate computed expressions (with dependency resolution via multiple passes)
   * Returns computed values based on current publisherUIState
   */
  function evaluateComputedValues(): Record<string, any> {
    if (Object.keys(computedExpressions).length === 0) {
      return {};
    }

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
            ...newComputed,  // Include already-computed values!
            size: (arr: any[]) => arr?.length || 0,
            length: (str: string) => str?.length || 0
          };

          // Expressions are pure CEL (no {{}} markers in template)
          const result = evaluateExpression(expression, context);
          newComputed[key] = result;
        } catch (error) {
          newComputed[key] = undefined;
        }
      }

      // Check if values changed
      const changed = Object.keys(newComputed).some(key => newComputed[key] !== prevComputed[key]);
      if (!changed) break;

    } while (pass < maxPasses);

    return newComputed;
  }

  /**
   * Handle publisher actions (generic)
   */
  async function handleAction(action: string, params: any = {}) {
    console.log('🎯 [PublisherApp] Handling action:', action, params);

    switch (action) {
      case 'setState':
        handleSetState(params);
        break;

      case 'navigate':
        handleNavigate(params);
        break;

      case 'uploadVideo':
        await handleUploadVideo(params);
        break;

      default:
        console.warn('⚠️ [PublisherApp] Unknown action:', action, 'Use setState for state updates');
    }
  }

  /**
   * Handle video upload action
   */
  async function handleUploadVideo(params: any) {
    const { stateField = 'uploadedVideo' } = params;

    try {
      console.log('🎬 [PublisherApp] Starting video upload...');

      // Set uploading state
      publisherUIState = {
        ...publisherUIState,
        [`${stateField}_uploading`]: true,
        [`${stateField}_error`]: null
      };

      const result = await uploadVideo();

      if (result) {
        // Store video ID in state
        publisherUIState = {
          ...publisherUIState,
          [stateField]: result.id,
          [`${stateField}_filename`]: result.filename,
          [`${stateField}_size`]: result.size,
          [`${stateField}_uploading`]: false
        };

        console.log('✅ [PublisherApp] Video uploaded and stored in contentDoc:', result);
        console.log('📦 [PublisherApp] Video is now in contentDoc.videos map with ID:', result.id);
      } else {
        // User cancelled
        publisherUIState = {
          ...publisherUIState,
          [`${stateField}_uploading`]: false
        };
        console.log('ℹ️ [PublisherApp] Video upload cancelled by user');
      }
    } catch (error) {
      console.error('❌ [PublisherApp] Video upload failed:', error);
      publisherUIState = {
        ...publisherUIState,
        [`${stateField}_uploading`]: false,
        [`${stateField}_error`]: String(error)
      };
      alert(`Video upload failed: ${error}`);
    }
  }


  /**
   * Update UI state (generic)
   */
  function handleSetState(params: any) {
    const { stateUpdates } = params;

    if (!stateUpdates) return;

    // Evaluate state updates (expressions are already evaluated by ButtonBlock)
    // stateUpdates contains the final values to set
    publisherUIState = {
      ...publisherUIState,
      ...stateUpdates
    };

    console.log('📝 [PublisherApp] State updated:', stateUpdates);
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
   * Prepare context for expression evaluation (generic)
   */
  let expressionContext = $derived({
    ...publisherUIState,
    ...computedValues,
    mode: 'publisher'
  });
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