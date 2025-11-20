<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { initializeStateFromTemplate, reloadFieldsFromDocument, subscribeToDocument, getUniqueDocuments } from '../shared/loro/stateManager';
  import { evaluateCEL as evaluateExpression, initReactive, updateReactiveState, getAllReactiveValues } from '../lib/services/celEvaluator';
  import { parseHUML } from '../lib/services/humlParser';
  import ScreenRenderer from './ScreenRenderer.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';
  import NavigationPanel from '../components/NavigationPanel.svelte';
  import NavigationToggle from '../components/NavigationToggle.svelte';
  import { uiState } from '../state';
  import { dataState } from '../state/data.svelte';

  // Viewer UI State - Read-only snapshot of CRDT state
  let viewerState = $state<Record<string, any>>({});
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
    console.log('🧮 [ViewerApp] Computed values from reactive system:', values);
    return values;
  });

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
   * Re-initialize viewer when resource changes
   * This effect runs on mount and whenever currentResourceId changes
   */
  $effect(() => {
    const resourceId = dataState.currentResourceId;

    // Skip if resourceId hasn't actually changed
    if (resourceId === previousResourceId) {
      return;
    }

    console.log('🔄 [ViewerApp] Effect triggered for resourceId:', resourceId, '(previous:', previousResourceId, ')');
    previousResourceId = resourceId;

    if (!resourceId) {
      console.log('❌ [ViewerApp] No resource selected, clearing content');
      viewerState = {};
      screens = [];
      currentScreenId = '';
      return;
    }

    // Re-initialize viewer with new resource
    console.log('🔄 [ViewerApp] Resource changed, re-initializing viewer');

    // Clear old state and screens immediately to prevent old blocks from evaluating
    viewerState = {};
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
    initializeViewer();
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

      // 1. Load state from template documents using shared state manager
      // Pass user context so CEL expressions in initial values can access userId, etc.
      const userContext = {
        userId: dataState.userDetails?.userId ?? '',
        deviceId: dataState.userDetails?.deviceId ?? '',
        username: dataState.userDetails?.username ?? ''
      };
      viewerState = initializeStateFromTemplate(template, loroCoordinator, userContext);

      console.log('📊 [ViewerApp] Initialized viewer state:', viewerState);

      // 2. Load computed expressions from template
      const expressions = template.computed || {};
      computedExpressions = expressions;
      console.log('🧮 [ViewerApp] Loaded computed expressions:', Object.keys(computedExpressions));

      // 3. Initialize reactive system with template
      initReactive(template);
      console.log('⚛️ [ViewerApp] Reactive system initialized');

      // 4. Sync loaded state to reactive system
      // The reactive system initialized with template defaults, but we have actual data from Loro
      console.log('🔄 [ViewerApp] Syncing loaded state to reactive system...');
      const documentsDefinition = template.documents || {};
      for (const fieldName of Object.keys(documentsDefinition)) {
        if (viewerState[fieldName] !== undefined) {
          console.log(`  ↪️ Syncing ${fieldName}:`, viewerState[fieldName]);
          updateReactiveState(fieldName, viewerState[fieldName]);
        }
      }
      reactiveVersion++; // Force recompute
      console.log('✅ [ViewerApp] State synced to reactive system');
    } catch (error) {
      console.error('❌ [ViewerApp] Failed to parse HUML:', error);
    }

    // 5. Load viewer screens
    loadViewerScreens();

    // 6. Subscribe to template changes (when HUML source changes)
    const templateDoc = loroCoordinator.getDocuments().templateDoc;
    templateUnsubscribe = templateDoc.subscribe(() => {
      // Re-initialize when template changes
      loadViewerScreens();
    });

    // 7. Subscribe to all documents used in the template (derived from template metadata)
    const documentsToSubscribe = getUniqueDocuments(templateDefinition);
    console.log('📡 [ViewerApp] Subscribing to documents:', documentsToSubscribe);

    for (const docName of documentsToSubscribe) {
      const unsubscribe = subscribeToDocument(docName, loroCoordinator, () => {
        // Reload fields from this document when it changes from sync
        // Use untrack() to prevent any unwanted side effects
        untrack(() => {
          const updatedState = reloadFieldsFromDocument(docName, templateDefinition, loroCoordinator);
          viewerState = { ...viewerState, ...updatedState };
          console.log(`🔄 [ViewerApp] Updated state from ${docName}:`, updatedState);

          // Update reactive signals for each changed field (fixes computed values!)
          for (const [key, value] of Object.entries(updatedState)) {
            updateReactiveState(key, value);
            console.log(`⚛️ [ViewerApp] Updated reactive signal: ${key} =`, value);
          }

          // Increment version to trigger Svelte reactivity
          reactiveVersion++;
        });
      });
      documentUnsubscribes.push(unsubscribe);
    }
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
   * Get document type for a field from template
   */
  function getDocumentTypeForKey(key: string): string {
    if (!templateDefinition?.documents) return 'unknown';
    const fieldDef = templateDefinition.documents[key];
    return fieldDef?.document || 'unknown';
  }

  /**
   * Update state (viewers can edit: user_content_doc, collaborative_doc, submissions_doc)
   */
  function handleSetState(params: any) {
    const { stateUpdates } = params;

    if (!stateUpdates) return;

    // Track which documents need committing
    const docsToCommit = new Set<string>();

    for (const [key, value] of Object.entries(stateUpdates)) {
      const docType = getDocumentTypeForKey(key);

      // Viewers can edit these document types
      if (docType === 'user_content_doc') {
        const userContentMap = loroCoordinator.getUserContentMap();
        userContentMap.set(key, value);
        viewerState = { ...viewerState, [key]: value };
        docsToCommit.add('user_content_doc');
        console.log('✅ [ViewerApp] Updated user content:', key, value);

        // Update reactive system
        updateReactiveState(key, value);
        reactiveVersion++;
      } else if (docType === 'collaborative_doc') {
        const collaborativeMap = loroCoordinator.getCollaborativeMap();
        collaborativeMap.set(key, value);
        viewerState = { ...viewerState, [key]: value };
        docsToCommit.add('collaborative_doc');
        console.log('✅ [ViewerApp] Updated collaborative state:', key, value);

        // Update reactive system
        updateReactiveState(key, value);
        reactiveVersion++;
      } else if (docType === 'submissions_doc') {
        // Submissions are managed via LoroList (not a Map)
        // Extract the new submission (last item in the array) and push it to the list
        if (key === 'submissions' && Array.isArray(value) && value.length > 0) {
          const newSubmission = value[value.length - 1]; // Get the last item (newly added)

          console.log('📝 [ViewerApp] Adding new submission to LoroList:', newSubmission);

          // Get the submissions list directly and push the new submission
          const submissionsList = loroCoordinator.getSubmissions();
          submissionsList.push(newSubmission);

          // Commit the submissions doc
          docsToCommit.add('submissions_doc');

          console.log('✅ [ViewerApp] Submission added to list successfully');

          // Update viewerState to reflect the new submission count
          viewerState = { ...viewerState, [key]: value };

          // Update reactive system
          updateReactiveState(key, value);
          reactiveVersion++;
        } else {
          console.warn(`⚠️ [ViewerApp] Submissions update ignored - expected array with items`);
        }
      } else {
        // Viewers CANNOT edit content_doc, template_doc, static_assets
        console.warn(`⚠️ [ViewerApp] Viewer cannot edit ${docType}:`, key);
      }
    }

    // Commit all modified documents
    if (docsToCommit.has('user_content_doc')) {
      loroCoordinator.getDocuments().userContentDoc.commit();
    }
    if (docsToCommit.has('collaborative_doc')) {
      loroCoordinator.getDocuments().collaborativeDoc.commit();
    }
    if (docsToCommit.has('submissions_doc')) {
      loroCoordinator.getDocuments().submissionsDoc.commit();
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
    mode: 'viewer',
    // Inject user details for CEL access
    userId: dataState.userDetails?.userId ?? '',
    deviceId: dataState.userDetails?.deviceId ?? '',
    username: dataState.userDetails?.username ?? ''
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
