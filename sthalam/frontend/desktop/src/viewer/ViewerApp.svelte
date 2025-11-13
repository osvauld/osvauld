<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { evaluateCEL as evaluateExpression } from '../lib/services/celEvaluator';
  import { parseHUML } from '../lib/services/humlParser';
  import ScreenRenderer from './ScreenRenderer.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';
  import NavigationPanel from '../components/NavigationPanel.svelte';
  import NavigationToggle from '../components/NavigationToggle.svelte';
  import { uiState } from '../state';

  // Viewer UI State - Read-only snapshot of CRDT state
  let viewerState = $state<Record<string, any>>({});
  let screens = $state<any[]>([]);
  let currentScreenId = $state<string>('');

  // Computed expressions from template
  let computedExpressions = $state<Record<string, string>>({});

  // Computed values - derived synchronously
  let computedValues = $derived(evaluateComputedValues());

  // CRDT subscriptions
  let templateUnsubscribe: (() => void) | null = null;
  let collaborativeUnsubscribe: (() => void) | null = null;

  // Template metadata for routing state updates
  let templateDefinition: any = null;

  onMount(() => {
    initializeViewer();
  });

  onDestroy(() => {
    if (templateUnsubscribe) templateUnsubscribe();
    if (collaborativeUnsubscribe) collaborativeUnsubscribe();
  });

  /**
   * Initialize Viewer Mode
   */
  function initializeViewer() {
    console.log('👁️ [ViewerApp] Initializing viewer mode...');

    // Get HUML source and parse it
    const templateMap = loroCoordinator.getTemplateMap();
    const humlSource = templateMap.get('huml_source');

    if (!humlSource || typeof humlSource !== 'string') {
      console.warn('⚠️ [ViewerApp] No HUML source found');
      return;
    }

    try {
      const template = parseHUML(humlSource);

      // Store template definition for state routing
      templateDefinition = template;

      // 1. Load viewer state definition from template (read-only)
      const stateDefinition = template.documents?.viewerState || {};

      // Extract initial values from state schema
      const initialState: Record<string, any> = {};
      for (const [key, schema] of Object.entries(stateDefinition)) {
        if (typeof schema === 'object' && schema !== null && 'initial' in schema) {
          initialState[key] = (schema as any).initial;
        } else {
          initialState[key] = schema;
        }
      }

      // Load persisted state from contentDoc if exists (read-only)
      const stateMap = loroCoordinator.getStateMap();
      const persistedState = stateMap.toJSON();

      // Also load collections from contentDoc
      const contentMap = loroCoordinator.getContentMap();
      const contentDocData = contentMap.toJSON();

      // Load collaborative state from collaborativeDoc (shared with owner/publisher)
      const collaborativeMap = loroCoordinator.getCollaborativeMap();
      const collaborativeState = collaborativeMap.toJSON();

      // Merge: initial state < persisted state < contentDoc collections < collaborative state
      viewerState = { ...initialState, ...persistedState, ...contentDocData, ...collaborativeState };

      console.log('📊 [ViewerApp] Initialized viewer state:', viewerState);

      // 2. Load computed expressions from template
      const expressions = template.documents?.viewerComputed || {};
      computedExpressions = expressions;
      console.log('🧮 [ViewerApp] Loaded computed expressions:', Object.keys(computedExpressions));
    } catch (error) {
      console.error('❌ [ViewerApp] Failed to parse HUML:', error);
    }

    // 3. Load viewer screens
    loadViewerScreens();

    // 4. Subscribe to template changes (when HUML source changes)
    const templateDoc = loroCoordinator.getDocuments().templateDoc;
    templateUnsubscribe = templateDoc.subscribe(() => {
      // Re-initialize when template changes
      loadViewerScreens();
    });

    // 5. Subscribe to collaborative doc changes (when owner/publisher updates collaborative state)
    const collaborativeDoc = loroCoordinator.getDocuments().collaborativeDoc;
    collaborativeUnsubscribe = collaborativeDoc.subscribe(() => {
      // Reload collaborative state when it changes from sync
      const collaborativeMap = loroCoordinator.getCollaborativeMap();
      const collaborativeState = collaborativeMap.toJSON();
      viewerState = { ...viewerState, ...collaborativeState };
      console.log('🔄 [ViewerApp] Updated collaborative state from sync:', collaborativeState);
    });
  }

  /**
   * Load viewer screens from template
   */
  function loadViewerScreens() {
    // Get raw HUML from templateDoc
    const templateMap = loroCoordinator.getTemplateMap();
    const humlSource = templateMap.get('huml_source');

    if (!humlSource || typeof humlSource !== 'string') {
      console.warn('⚠️ [ViewerApp] No HUML source found in template');
      screens = [];
      return;
    }

    try {
      // Parse HUML in memory
      const template = parseHUML(humlSource);
      console.log('✅ [ViewerApp] Parsed template:', template);

      // Extract viewer screens from ui.viewer
      const viewerScreens = template.ui?.viewer || [];
      screens = viewerScreens;

      // Set entry point as current screen
      if (screens.length > 0) {
        const entryScreen = screens.find(s => s.isEntryPoint) || screens[0];
        currentScreenId = entryScreen.id || entryScreen.name || 'home';
      }

      console.log('📺 [ViewerApp] Loaded viewer screens:', screens.length);
      if (screens.length > 0) {
        console.log('📋 [ViewerApp] First screen:', screens[0]);
      }
    } catch (error) {
      console.error('❌ [ViewerApp] Failed to parse HUML:', error);
      screens = [];
    }
  }

  /**
   * Evaluate computed expressions (with dependency resolution via multiple passes)
   * Returns computed values based on current viewerState
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
            ...viewerState,
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
   * Determine which Loro document a state key belongs to
   */
  function getDocumentTypeForKey(key: string): 'publisherState' | 'collaborativeState' | 'contentDoc' | 'unknown' {
    if (!templateDefinition?.documents) return 'unknown';

    // Check each document section
    if (templateDefinition.documents.publisherState?.[key]) return 'publisherState';
    if (templateDefinition.documents.collaborativeState?.[key]) return 'collaborativeState';
    if (templateDefinition.documents.contentDoc?.[key]) return 'contentDoc';

    return 'unknown';
  }

  /**
   * Handle viewer actions (limited: navigate and collaborative setState only)
   */
  function handleAction(action: string, params: any = {}) {
    console.log('🎯 [ViewerApp] Handling action:', action, params);

    switch (action) {
      case 'navigate':
        handleNavigate(params);
        break;

      case 'setState':
        handleSetState(params);
        break;

      default:
        console.warn('⚠️ [ViewerApp] Action not supported in viewer mode:', action);
    }
  }

  /**
   * Update collaborative state (viewers can only edit collaborative state)
   */
  function handleSetState(params: any) {
    const { stateUpdates } = params;

    if (!stateUpdates) return;

    // Determine which updates are allowed (only collaborative state)
    const collaborativeMap = loroCoordinator.getCollaborativeMap();
    let hasCollaborativeUpdates = false;

    for (const [key, value] of Object.entries(stateUpdates)) {
      const docType = getDocumentTypeForKey(key);

      if (docType === 'collaborativeState') {
        // Viewers CAN edit collaborative state
        collaborativeMap.set(key, value);
        viewerState = { ...viewerState, [key]: value };
        hasCollaborativeUpdates = true;
        console.log('✅ [ViewerApp] Updated collaborative state:', key, value);
      } else {
        // Viewers CANNOT edit other state types
        console.warn(`⚠️ [ViewerApp] Viewer cannot edit ${docType} state:`, key);
      }
    }

    if (hasCollaborativeUpdates) {
      loroCoordinator.getDocuments().collaborativeDoc.commit();
    }
  }

  /**
   * Navigate to a different screen
   */
  function handleNavigate(params: any) {
    const { targetScreen } = params;

    if (!targetScreen) return;

    currentScreenId = targetScreen;
    console.log('🗺️ [ViewerApp] Navigated to screen:', targetScreen);
  }

  /**
   * Get the current screen to render
   */
  let currentScreen = $derived(screens.find(s => s.id === currentScreenId) || screens[0]);

  /**
   * Prepare context for expression evaluation (read-only)
   */
  let expressionContext = $derived({
    ...viewerState,
    ...computedValues,
    mode: 'viewer'
  });
</script>

<div class="viewer-app">
  <!-- Navigation Panel -->
  <NavigationPanel />

  <!-- Main Content Area -->
  <div class="viewer-content">
    <div class="viewer-toolbar">
      <div class="flex items-center gap-2">
        {#if !uiState.showNavigationPanel}
          <NavigationToggle />
        {/if}
        <span class="toolbar-title">Viewer Mode</span>
      </div>
      <ModeSwitcher />
    </div>

    {#if currentScreen}
      <ScreenRenderer
        {currentScreen}
        context={expressionContext}
        onAction={handleAction}
      />
    {:else}
      <div class="no-screens">
        <h2>Viewer Mode</h2>
        <p>No screens defined for viewer mode.</p>
        <p>Please define viewer screens in your template.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .viewer-app {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }

  .viewer-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .viewer-toolbar {
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
