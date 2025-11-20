<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { getMapForField, getDocumentNameForField } from '../shared/loro/documentRouter';
  import { initializeStateFromTemplate, reloadFieldsFromDocument, subscribeToDocument, shouldTriggerReactiveUpdate, getUniqueDocuments } from '../shared/loro/stateManager';
  import { evaluateCEL as evaluateExpression, initReactive, updateReactiveState, getAllReactiveValues } from '../lib/services/celEvaluator';
  import { parseHUML } from '../lib/services/humlParser';
  import BlockRenderer from '../renderer/BlockRenderer.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';
  import NavigationPanel from '../components/NavigationPanel.svelte';
  import NavigationToggle from '../components/NavigationToggle.svelte';
  import { uploadAsset, setAllowedFileTypes } from '../lib/services/assetService';
  import { uiState } from '../state';
  import { dataState } from '../state/data.svelte';

  // Publisher UI State - Local snapshots of CRDT state for reactive UI
  let publisherUIState = $state<Record<string, any>>({});
  let screens = $state<any[]>([]);
  let currentScreenId = $state<string>('');

  // Computed expressions from template (kept for reference, but now handled by reactive system)
  let computedExpressions = $state<Record<string, string>>({});

  // Reactive version counter - increments on each state update to trigger Svelte reactivity
  let reactiveVersion = $state(0);

  // Computed values - now powered by OCaml React FRP!
  // This returns ALL reactive values (state + computed) from the signal graph
  // The reactiveVersion dependency forces this to re-run when state updates
  let computedValues = $derived.by(() => {
    reactiveVersion; // Read to create dependency
    const values = getAllReactiveValues();
    console.log('🧮 [PublisherApp] Computed values from reactive system:', values);
    console.log('🧮 [PublisherApp] videoCount:', values.videoCount);
    console.log('🧮 [PublisherApp] imageCount:', values.imageCount);
    console.log('🧮 [PublisherApp] hasVideos:', values.hasVideos);
    return values;
  });

  // Asset type configuration for unified upload handler
  const ASSET_CONFIG = {
    image: { emoji: '🖼️', label: 'Image', defaultField: 'uploadedImage' },
    video: { emoji: '🎬', label: 'Video', defaultField: 'uploadedVideo' },
    file: { emoji: '📄', label: 'File', defaultField: 'uploadedFile' },
    audio: { emoji: '🎵', label: 'Audio', defaultField: 'uploadedAudio' }
  };

  // CRDT subscriptions
  let templateUnsubscribe: (() => void) | null = null;
  let documentUnsubscribes: (() => void)[] = [];

  // Template metadata for routing state updates
  let templateDefinition: any = null;

  // Track previous resource ID to prevent double initialization
  let previousResourceId: string | null = null;

  onDestroy(() => {
    if (templateUnsubscribe) templateUnsubscribe();
    documentUnsubscribes.forEach(unsub => unsub());
  });

  /**
   * Re-initialize publisher when resource changes
   * This effect runs on mount and whenever currentResourceId changes
   */
  $effect(() => {
    const resourceId = dataState.currentResourceId;

    // Skip if resourceId hasn't actually changed
    if (resourceId === previousResourceId) {
      return;
    }

    console.log('🔄 [PublisherApp] Effect triggered for resourceId:', resourceId, '(previous:', previousResourceId, ')');
    previousResourceId = resourceId;

    if (!resourceId) {
      console.log('❌ [PublisherApp] No resource selected, clearing content');
      publisherUIState = {};
      screens = [];
      currentScreenId = '';
      return;
    }

    // Re-initialize publisher with new resource
    console.log('🔄 [PublisherApp] Resource changed, re-initializing publisher');

    // Clear old state and screens immediately to prevent old blocks from evaluating
    publisherUIState = {};
    screens = [];
    currentScreenId = '';

    // Clean up old subscriptions
    if (templateUnsubscribe) {
      templateUnsubscribe();
      templateUnsubscribe = null;
    }
    documentUnsubscribes.forEach(unsub => unsub());
    documentUnsubscribes = [];

    // Initialize with new resource
    initializePublisher();
  });

  /**
   * Persist state changes to appropriate Loro documents automatically
   */
  $effect(() => {
    // Watch for changes to publisherUIState
    const stateToSave = publisherUIState;

    // Only save if we have state to persist
    if (Object.keys(stateToSave).length === 0) return;
    if (!templateDefinition?.documents) return;

    // Track which documents were modified so we can commit them
    const modifiedDocs = new Set<string>();

    for (const [key, value] of Object.entries(stateToSave)) {
      // Only persist non-temporary fields (exclude _uploading, _error)
      if (key.endsWith('_uploading') || key.endsWith('_error')) continue;

      // Skip submissions - they're managed by SubmissionsStore
      if (key === 'submissions') continue;

      try {
        // Get the UCAN document name from template metadata
        const docName = getDocumentNameForField(key, templateDefinition);

        // Get the Loro map for this document and set the value
        const loroMap = getMapForField(key, templateDefinition, loroCoordinator);
        loroMap.set(key, value);

        // Track that this document was modified
        modifiedDocs.add(docName);
      } catch (error) {
        console.warn(`[PublisherApp] Failed to persist field '${key}':`, error);
      }
    }

    // Commit all modified documents
    const docs = loroCoordinator.getDocuments();
    if (modifiedDocs.has('content_doc')) docs.contentDoc.commit();
    if (modifiedDocs.has('collaborative_doc')) docs.collaborativeDoc.commit();
    if (modifiedDocs.has('user_content_doc')) docs.userContentDoc.commit();
    if (modifiedDocs.has('ui_state_doc')) docs.uiStateDoc.commit();
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

      // Store template definition for state routing
      templateDefinition = template;

      // 1. Load state from template documents using shared state manager
      // Pass user context so CEL expressions in initial values can access userId, etc.
      const userContext = {
        userId: dataState.userDetails?.userId ?? '',
        deviceId: dataState.userDetails?.deviceId ?? '',
        username: dataState.userDetails?.username ?? ''
      };
      publisherUIState = initializeStateFromTemplate(template, loroCoordinator, userContext);

      // Initialize arrays as empty if not defined (for list-based templates)
      if (!publisherUIState.videos) {
        publisherUIState.videos = [];
      }

      if (!publisherUIState.images) {
        publisherUIState.images = [];
      }

      if (!publisherUIState.files) {
        publisherUIState.files = [];
      }

      if (!publisherUIState.audios) {
        publisherUIState.audios = [];
      }

      console.log('📊 [PublisherApp] Initialized state:', publisherUIState);
      console.log('📊 [PublisherApp] Videos array:', publisherUIState.videos);
      console.log('📊 [PublisherApp] Images array:', publisherUIState.images);
      console.log('📊 [PublisherApp] Audios array:', publisherUIState.audios);

      // 2. Load computed expressions from template
      const expressions = template.computed || {};
      computedExpressions = expressions;
      console.log('🧮 [PublisherApp] Loaded computed expressions:', Object.keys(computedExpressions));

      // 3. Initialize reactive system with template
      console.log('🔍 [PublisherApp] Template structure before initReactive:', {
        hasDocuments: !!template.documents,
        hasPublisherState: !!template.documents?.publisherState,
        hasPublisherComputed: !!template.documents?.publisherComputed,
        publisherStateKeys: template.documents?.publisherState ? Object.keys(template.documents.publisherState) : [],
        publisherComputedKeys: template.documents?.publisherComputed ? Object.keys(template.documents.publisherComputed) : [],
        sampleState: template.documents?.publisherState?.counter,
        sampleComputed: template.documents?.publisherComputed?.counterDouble
      });
      initReactive(template);
      console.log('⚛️ [PublisherApp] Reactive system initialized');

      // 4. Sync loaded state to reactive system
      // The reactive system initialized with template defaults, but we have actual data from Loro
      console.log('🔄 [PublisherApp] Syncing loaded state to reactive system...');
      const documentsDefinition = template.documents || {};
      for (const fieldName of Object.keys(documentsDefinition)) {
        if (publisherUIState[fieldName] !== undefined) {
          console.log(`  ↪️ Syncing ${fieldName}:`, publisherUIState[fieldName]);
          updateReactiveState(fieldName, publisherUIState[fieldName]);
        }
      }
      reactiveVersion++; // Force recompute
      console.log('✅ [PublisherApp] State synced to reactive system');
    } catch (error) {
      console.error('❌ [PublisherApp] Failed to parse HUML:', error);
    }

    // 5. Load publisher screens
    loadPublisherScreens();

    // 6. Subscribe to template changes (when HUML source changes)
    const templateDoc = loroCoordinator.getDocuments().templateDoc;
    templateUnsubscribe = templateDoc.subscribe(() => {
      // Re-initialize when template changes
      loadPublisherScreens();
    });

    // 5. Subscribe to all documents used in the template (derived from template metadata)
    const documentsToSubscribe = getUniqueDocuments(templateDefinition);
    console.log('📡 [PublisherApp] Subscribing to documents:', documentsToSubscribe);

    for (const docName of documentsToSubscribe) {
      const unsubscribe = subscribeToDocument(docName, loroCoordinator, () => {
        // Reload fields from this document when it changes from sync
        // Use untrack() to prevent triggering the $effect that saves to Loro
        untrack(() => {
          const updatedState = reloadFieldsFromDocument(docName, templateDefinition, loroCoordinator);
          publisherUIState = { ...publisherUIState, ...updatedState };
          console.log(`🔄 [PublisherApp] Updated state from ${docName}:`, updatedState);

          // Update reactive signals for each changed field (fixes computed values!)
          for (const [key, value] of Object.entries(updatedState)) {
            updateReactiveState(key, value);
            console.log(`⚛️ [PublisherApp] Updated reactive signal: ${key} =`, value);
          }

          // Increment version to trigger Svelte reactivity
          reactiveVersion++;
        });
      });
      documentUnsubscribes.push(unsubscribe);
    }
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
   * OLD: Evaluate computed expressions (REPLACED BY OCaml React FRP)
   * The reactive system handles this automatically now - no multi-pass evaluation needed!
   * Dependencies are explicit and React FRP handles propagation efficiently.
   */
  // REMOVED: Multi-pass evaluation logic - replaced by reactive system

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
        await handleUploadAsset({ assetType: 'video', ...params });
        break;

      case 'uploadImage':
        await handleUploadAsset({ assetType: 'image', ...params });
        break;

      case 'uploadFile':
        await handleUploadAsset({ assetType: 'file', ...params });
        break;

      case 'uploadAudio':
        await handleUploadAsset({ assetType: 'audio', ...params });
        break;

      case 'setFileTypes':
        handleSetFileTypes(params);
        break;

      default:
        console.warn('⚠️ [PublisherApp] Unknown action:', action, 'Use setState for state updates');
    }
  }

  /**
   * Unified asset upload handler (images, videos, files, etc.)
   * Extensible: add new asset types to ASSET_CONFIG
   */
  async function handleUploadAsset(params: any) {
    const { assetType, stateField } = params;

    // Validate asset type
    if (!assetType || !ASSET_CONFIG[assetType]) {
      console.error('❌ [PublisherApp] Invalid asset type:', assetType);
      return;
    }

    const config = ASSET_CONFIG[assetType];
    const field = stateField || config.defaultField;

    try {
      console.log(`${config.emoji} [PublisherApp] Starting ${config.label.toLowerCase()} upload...`);

      // Set uploading state
      publisherUIState = {
        ...publisherUIState,
        [`${field}_uploading`]: true,
        [`${field}_error`]: null
      };

      const result = await uploadAsset(assetType);

      if (result) {
        // Store asset ID in state
        publisherUIState = {
          ...publisherUIState,
          [field]: result.id,
          [`${field}_filename`]: result.filename,
          [`${field}_size`]: result.size,
          [`${field}_uploading`]: false
        };

        console.log(`✅ [PublisherApp] ${config.label} uploaded:`, result);
      } else {
        // User cancelled
        publisherUIState = {
          ...publisherUIState,
          [`${field}_uploading`]: false
        };
        console.log(`ℹ️ [PublisherApp] ${config.label} upload cancelled by user`);
      }
    } catch (error) {
      console.error(`❌ [PublisherApp] ${config.label} upload failed:`, error);
      publisherUIState = {
        ...publisherUIState,
        [`${field}_uploading`]: false,
        [`${field}_error`]: String(error)
      };
      alert(`${config.label} upload failed: ${error}`);
    }
  }

  /**
   * Handle set file types action
   */
  function handleSetFileTypes(params: any) {
    const { types = [] } = params;

    if (!Array.isArray(types)) {
      console.error('❌ [PublisherApp] setFileTypes requires an array of file extensions');
      return;
    }

    try {
      setAllowedFileTypes(types);
      console.log('✅ [PublisherApp] Allowed file types configured:', types);
    } catch (error) {
      console.error('❌ [PublisherApp] Failed to set file types:', error);
    }
  }


  /**
   * Update UI state (generic)
   */
  function handleSetState(params: any) {
    const { stateUpdates } = params;

    if (!stateUpdates) return;

    // Update reactive state (triggers automatic reactive propagation!)
    for (const [key, value] of Object.entries(stateUpdates)) {
      updateReactiveState(key, value);
      console.log(`⚛️ [PublisherApp] Reactive update: ${key} =`, value);
    }

    // Also update local UI state for non-reactive fields (like UI state, uploads, etc.)
    publisherUIState = {
      ...publisherUIState,
      ...stateUpdates
    };

    // Increment version to trigger Svelte reactivity
    reactiveVersion++;

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
    ...computedValues,
    ...publisherUIState,  // Spread last so local state overwrites reactive signals
    mode: 'publisher',
    // Inject user details for CEL access
    userId: dataState.userDetails?.userId ?? '',
    deviceId: dataState.userDetails?.deviceId ?? '',
    username: dataState.userDetails?.username ?? ''
  });
</script>

<div class="publisher-app">
  <!-- Navigation Panel -->
  <NavigationPanel />

  <!-- Main Content Area -->
  <div class="publisher-content">
    <div class="publisher-toolbar">
      <div class="flex items-center gap-2">
        {#if !uiState.showNavigationPanel}
          <NavigationToggle />
        {/if}
        <span class="toolbar-title">Publisher Mode</span>
      </div>
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
                // Update local state - use spread to trigger $derived reactivity
                publisherUIState = { ...publisherUIState, [key]: value };

                // Update reactive system for collaborative/synced documents
                const docName = getDocumentNameForField(key, templateDefinition);
                if (shouldTriggerReactiveUpdate(docName)) {
                  updateReactiveState(key, value);
                  reactiveVersion++; // Trigger reactivity for collaborative changes
                }
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
</div>

<style>
  .publisher-app {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }

  .publisher-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
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