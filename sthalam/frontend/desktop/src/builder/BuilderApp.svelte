<script lang="ts">
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { dataState, uiState } from '../state';
  import HUMLEditor from '../shared/codemirror/HUMLEditor.svelte';
  import NavigationPanel from '../components/NavigationPanel.svelte';
  import NavigationToggle from '../components/NavigationToggle.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { readFile, readTextFile } from '@tauri-apps/plugin-fs';
  import { invoke } from '@tauri-apps/api/core';

  let humlContent = $state<string>('');
  let originalContent = $state<string>('');
  let isSaving = $state<boolean>(false);
  let lastSavedTime = $state<Date | null>(null);
  let autoSaveInterval: number | null = null;
  let isImporting = $state<boolean>(false);
  let isOpeningPreview = $state<boolean>(false);
  let isRunningTests = $state<boolean>(false);
  let testReport = $state<TestReport | null>(null);
  let showTestResults = $state<boolean>(false);
  let hasWasmApp = $state<boolean>(false);

  // Test report types matching Rust TestReport
  interface TestResult {
    name: string;
    passed: boolean;
    error: string | null;
    duration_ms: number;
  }

  interface TestReport {
    total: number;
    passed: number;
    failed: number;
    results: TestResult[];
    duration_ms: number;
  }

  // Track if content has unsaved changes
  const isDirty = $derived(humlContent !== originalContent);

  // Track if we have a resource selected
  const hasResource = $derived(!!dataState.currentResourceId);

  // Load HUML content from Loro contentDoc
  $effect(() => {
    const resourceId = dataState.currentResourceId;

    console.log('🔄 [BuilderApp] Effect triggered for resourceId:', resourceId);

    // Clear autosave interval when resource changes
    if (autoSaveInterval !== null) {
      clearInterval(autoSaveInterval);
      autoSaveInterval = null;
    }

    if (!resourceId) {
      console.log('❌ [BuilderApp] No resource selected, clearing content');
      humlContent = '';
      originalContent = '';
      return;
    }

    // Get HUML source from template doc
    const templateMap = loroCoordinator.getTemplateMap();
    const humlSource = templateMap.get('huml_source');

    const content = typeof humlSource === 'string' ? humlSource : '';

    console.log('📂 [BuilderApp] Loading content:', {
      resourceId,
      contentLength: content.length,
      currentContentLength: humlContent.length,
      isDifferent: content !== humlContent
    });

    humlContent = content;
    originalContent = content;
    // Reset hasWasmApp - will be set when import succeeds
    // TODO: Check page's has_wasm from handle_open_page response
    hasWasmApp = false;

    // Start autosave timer (60 second interval)
    autoSaveInterval = window.setInterval(async () => {
      if (isDirty) {
        await saveContent();
      }
    }, 60000);

    // Cleanup
    return () => {
      if (autoSaveInterval !== null) {
        clearInterval(autoSaveInterval);
        autoSaveInterval = null;
      }
    };
  });

  /**
   * Save content to Loro contentDoc
   * Note: SyncManager handles sending changes to backend automatically
   * HUML parsing is done in Rust when opening preview, not here
   */
  async function saveContent() {
    if (!dataState.currentResourceId || !isDirty || isSaving) {
      return;
    }

    try {
      isSaving = true;

      // Update template doc with raw HUML source
      // HUML parsing happens in Rust when opening native preview
      const templateMap = loroCoordinator.getTemplateMap();
      templateMap.set('huml_source', humlContent);

      // Commit all documents - SyncManager will auto-sync to backend
      loroCoordinator.getDocuments().contentDoc.commit();
      loroCoordinator.getDocuments().templateDoc.commit();
      loroCoordinator.getDocuments().userContentDoc.commit();
      loroCoordinator.getDocuments().uiStateDoc.commit();

      originalContent = humlContent;
      lastSavedTime = new Date();

      console.log('💾 [BuilderApp] Content committed, auto-sync will send to backend');
    } catch (error) {
      console.error('❌ [BuilderApp] Failed to save:', error);
      alert(`Failed to save: ${error}`);
    } finally {
      isSaving = false;
    }
  }

  /**
   * Reload content from Loro
   */
  function reloadContent() {
    if (isDirty) {
      const confirmed = confirm(
        'You have unsaved changes. Reloading will discard them. Continue?'
      );
      if (!confirmed) return;
    }

    const templateMap = loroCoordinator.getTemplateMap();
    const humlSource = templateMap.get('huml_source');
    const content = typeof humlSource === 'string' ? humlSource : '';

    humlContent = content;
    originalContent = content;

    console.log('🔄 [BuilderApp] Reloaded HUML content');
  }

  /**
   * Import WASM app to page
   * Saves to page's wasm_module layer in redb (encrypted)
   */
  async function importWasmApp() {
    const pageId = dataState.currentPageId;
    if (!pageId) {
      alert('Please select a page first');
      return;
    }

    try {
      const selected = await open({
        filters: [{
          name: 'WASM App',
          extensions: ['wasm']
        }],
        multiple: false
      });

      if (!selected || typeof selected !== 'string') {
        return;
      }

      isImporting = true;

      console.log('📥 [BuilderApp] Selected WASM app:', selected);

      // Read the WASM file bytes
      const wasmBytes = await readFile(selected);

      // Convert to base64
      const base64 = btoa(
        Array.from(wasmBytes)
          .map(b => String.fromCharCode(b))
          .join('')
      );

      console.log('📦 [BuilderApp] WASM bytes:', wasmBytes.length, 'base64 length:', base64.length);

      // Save WASM to page
      await invoke('handle_save_page_wasm', {
        input: {
          pageId: pageId,
          wasmBase64: base64
        }
      });

      console.log('✅ [BuilderApp] WASM saved to page');

      hasWasmApp = true;
      lastSavedTime = new Date();
    } catch (error) {
      console.error('❌ [BuilderApp] Import failed:', error);
      alert(`Failed to import: ${error}`);
    } finally {
      isImporting = false;
    }
  }

  /**
   * Open preview for existing WASM app
   * Loads from page's wasm_module layer and opens native window
   */
  async function openPreview() {
    const pageId = dataState.currentPageId;
    if (!pageId) {
      alert('Please select a page first');
      return;
    }

    try {
      isOpeningPreview = true;

      console.log('🔗 [BuilderApp] Opening existing WASM preview for page:', pageId);

      const windowLabel = await invoke<string>('handle_open_page_preview', {
        input: {
          pageId: pageId
        }
      });

      console.log('✅ [BuilderApp] Preview window opened:', windowLabel);
    } catch (error) {
      console.error('❌ [BuilderApp] Failed to open preview:', error);
      alert(`Failed to open preview: ${error}`);
    } finally {
      isOpeningPreview = false;
    }
  }

  /**
   * Run embedded template tests
   *
   * If a page_id is available (via dataState.currentPageId), tests run with
   * real Scribe integration - fixtures seed to Loro, actions execute real
   * CRDT operations, and assert_state/assert_query verify actual document state.
   */
  async function runTests() {
    if (!humlContent.trim()) {
      alert('No HUML content to test');
      return;
    }

    try {
      isRunningTests = true;
      testReport = null;

      // Pass page_id for Scribe integration if available
      const pageId = dataState.currentPageId;
      console.log('🧪 [BuilderApp] Running template tests...', pageId ? `(with Scribe: ${pageId})` : '(intent only)');

      const report = await invoke<TestReport>('handle_run_template_tests', {
        input: {
          template_source: humlContent,
          page_id: pageId || undefined  // Only pass if available
        }
      });

      testReport = report;
      showTestResults = true;

      console.log('✅ [BuilderApp] Test results:', report);
    } catch (error) {
      console.error('❌ [BuilderApp] Failed to run tests:', error);
      alert(`Failed to run tests: ${error}`);
    } finally {
      isRunningTests = false;
    }
  }

  /**
   * Handle keyboard shortcuts
   */
  function handleKeyDown(e: KeyboardEvent) {
    // Ctrl+S or Cmd+S to save
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault();
      saveContent();
    }
  }

  /**
   * Format time for display
   */
  function formatTime(date: Date): string {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const seconds = Math.floor(diff / 1000);

    if (seconds < 60) return 'just now';
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;

    return date.toLocaleTimeString();
  }

</script>

<svelte:window onkeydown={handleKeyDown} />

{#if !hasResource}
  <!-- No resource selected - show empty state -->
  <div class="empty-state">
    <NavigationPanel />
    {#if !uiState.showNavigationPanel}
      <div class="floating-toggle">
        <NavigationToggle />
      </div>
    {/if}
    <div class="empty-message">
      <h2>No resource selected</h2>
      <p>Select a resource from the sidebar or create a new one to get started.</p>
    </div>
  </div>
{:else}
  <!-- Resource selected - show editor -->
  <div class="builder-app">
    <NavigationPanel />
    {#if !uiState.showNavigationPanel}
      <div class="floating-toggle">
        <NavigationToggle />
      </div>
    {/if}

    <div class="editor-workspace">
      <!-- Toolbar -->
      <div class="toolbar">
        <button
          onclick={importWasmApp}
          class="btn-import"
          disabled={isImporting}
          title="Import WASM app to page"
        >
          {isImporting ? '📥 Importing...' : '📥 Import WASM'}
        </button>

        {#if hasWasmApp}
          <span class="wasm-badge">
            🔧 WASM loaded
          </span>
        {/if}

        <button
          onclick={saveContent}
          class="btn-save"
          class:dirty={isDirty}
          disabled={!isDirty || isSaving}
          title={isDirty ? 'Save (Ctrl+S)' : 'No changes to save'}
        >
          {isSaving ? '💾 Saving...' : isDirty ? '💾 Save *' : '✓ Saved'}
        </button>

        <button
          onclick={reloadContent}
          class="btn-reload"
          title="Reload from document (discards unsaved changes)"
        >
          🔄 Reload
        </button>

        <button
          onclick={openPreview}
          class="btn-preview"
          disabled={isOpeningPreview}
          title="Open WASM app preview (Vello GPU rendering)"
        >
          {isOpeningPreview ? '🖼️ Opening...' : '🖼️ Preview'}
        </button>

        <button
          onclick={runTests}
          class="btn-test"
          class:has-results={testReport !== null}
          class:all-passed={testReport?.failed === 0 && testReport?.total > 0}
          class:has-failures={testReport?.failed !== undefined && testReport.failed > 0}
          disabled={isRunningTests || !humlContent.trim()}
          title="Run embedded template tests"
        >
          {#if isRunningTests}
            🧪 Running...
          {:else if testReport}
            🧪 {testReport.passed}/{testReport.total}
          {:else}
            🧪 Run Tests
          {/if}
        </button>

        <span class="status" class:modified={isDirty}>
          {#if lastSavedTime}
            Saved {formatTime(lastSavedTime)}
          {:else if isDirty}
            Unsaved changes
          {:else}
            No changes
          {/if}
        </span>

        <div class="toolbar-right">
          <ModeSwitcher />
        </div>
      </div>

      <!-- Editor -->
      <div class="editor-container">
        <HUMLEditor bind:value={humlContent} />
      </div>

      <!-- Test Results Panel -->
      {#if showTestResults && testReport}
        <div class="test-results-panel">
          <div class="test-results-header">
            <span class="test-results-title">
              {#if testReport.failed === 0}
                ✅ All Tests Passed
              {:else}
                ❌ {testReport.failed} Test{testReport.failed > 1 ? 's' : ''} Failed
              {/if}
              <span class="test-summary">
                ({testReport.passed}/{testReport.total} passed, {testReport.duration_ms}ms)
              </span>
            </span>
            <button class="btn-close-results" onclick={() => showTestResults = false}>✕</button>
          </div>
          <div class="test-results-list">
            {#each testReport.results as result}
              <div class="test-result" class:passed={result.passed} class:failed={!result.passed}>
                <span class="test-icon">{result.passed ? '✓' : '✗'}</span>
                <span class="test-name">{result.name}</span>
                <span class="test-duration">{result.duration_ms}ms</span>
                {#if result.error}
                  <div class="test-error">{result.error}</div>
                {/if}
              </div>
            {/each}
            {#if testReport.total === 0}
              <div class="no-tests">No tests found in template. Add a <code>tests {'{'} {'}'}</code> block.</div>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .builder-app {
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: #010409;
    display: flex;
  }

  .empty-state {
    width: 100%;
    height: 100%;
    display: flex;
    background: #010409;
  }

  .empty-message {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #8b949e;
  }

  .empty-message h2 {
    font-size: 1.5rem;
    margin-bottom: 0.5rem;
    color: #c9d1d9;
  }

  .empty-message p {
    font-size: 1rem;
  }

  .floating-toggle {
    position: absolute;
    top: 1rem;
    left: 1rem;
    z-index: 1000;
  }

  .editor-workspace {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .toolbar {
    display: flex;
    gap: 12px;
    padding: 12px;
    background: #1e1e1e;
    border-bottom: 1px solid #333;
    align-items: center;
  }

  .status {
    flex: 1;
    font-size: 13px;
    color: #888;
  }

  .toolbar-right {
    margin-left: auto;
  }

  .status.modified {
    color: #f59e0b;
  }

  .wasm-badge {
    background: #10b981;
    color: white;
    padding: 4px 10px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 500;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-import,
  .btn-save,
  .btn-reload,
  .btn-preview,
  .btn-test {
    padding: 8px 16px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-weight: 500;
    transition: all 0.2s;
  }

  .btn-import {
    background: #3b82f6;
    color: white;
  }

  .btn-import:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-import:disabled {
    background: #374151;
    color: #6b7280;
    cursor: not-allowed;
  }

  .btn-save {
    background: #10b981;
    color: white;
  }

  .btn-save.dirty {
    background: #f59e0b;
    animation: pulse 2s ease-in-out infinite;
  }

  .btn-save:hover:not(:disabled) {
    background: #059669;
  }

  .btn-save.dirty:hover:not(:disabled) {
    background: #d97706;
  }

  .btn-save:disabled {
    background: #374151;
    color: #6b7280;
    cursor: not-allowed;
  }

  .btn-reload {
    background: #6366f1;
    color: white;
  }

  .btn-reload:hover {
    background: #4f46e5;
  }

  .btn-preview {
    background: #8b5cf6;
    color: white;
  }

  .btn-preview:hover:not(:disabled) {
    background: #7c3aed;
  }

  .btn-preview:disabled {
    background: #374151;
    color: #6b7280;
    cursor: not-allowed;
  }

  .btn-test {
    background: #6366f1;
    color: white;
  }

  .btn-test:hover:not(:disabled) {
    background: #4f46e5;
  }

  .btn-test:disabled {
    background: #374151;
    color: #6b7280;
    cursor: not-allowed;
  }

  .btn-test.has-results.all-passed {
    background: #10b981;
  }

  .btn-test.has-results.has-failures {
    background: #ef4444;
  }

  /* Test Results Panel */
  .test-results-panel {
    background: #1e1e1e;
    border-top: 1px solid #333;
    max-height: 200px;
    overflow-y: auto;
  }

  .test-results-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    background: #252526;
    border-bottom: 1px solid #333;
    position: sticky;
    top: 0;
  }

  .test-results-title {
    font-weight: 500;
    color: #e1e4e8;
  }

  .test-summary {
    font-weight: normal;
    color: #8b949e;
    margin-left: 8px;
    font-size: 13px;
  }

  .btn-close-results {
    background: transparent;
    border: none;
    color: #8b949e;
    cursor: pointer;
    padding: 4px 8px;
    font-size: 14px;
  }

  .btn-close-results:hover {
    color: #e1e4e8;
  }

  .test-results-list {
    padding: 8px 12px;
  }

  .test-result {
    display: flex;
    align-items: flex-start;
    flex-wrap: wrap;
    padding: 6px 8px;
    border-radius: 4px;
    margin-bottom: 4px;
    font-size: 13px;
  }

  .test-result.passed {
    background: rgba(16, 185, 129, 0.1);
  }

  .test-result.failed {
    background: rgba(239, 68, 68, 0.1);
  }

  .test-icon {
    width: 20px;
    font-weight: bold;
  }

  .test-result.passed .test-icon {
    color: #10b981;
  }

  .test-result.failed .test-icon {
    color: #ef4444;
  }

  .test-name {
    flex: 1;
    color: #c9d1d9;
  }

  .test-duration {
    color: #6b7280;
    font-size: 12px;
    margin-left: 8px;
  }

  .test-error {
    width: 100%;
    margin-top: 4px;
    margin-left: 20px;
    padding: 8px;
    background: rgba(239, 68, 68, 0.15);
    border-radius: 4px;
    color: #fca5a5;
    font-family: monospace;
    font-size: 12px;
    white-space: pre-wrap;
  }

  .no-tests {
    color: #8b949e;
    font-style: italic;
    padding: 8px;
  }

  .no-tests code {
    background: #374151;
    padding: 2px 6px;
    border-radius: 4px;
  }

  @keyframes pulse {
    0%, 100% {
      box-shadow: 0 0 0 0 rgba(245, 158, 11, 0.7);
    }
    50% {
      box-shadow: 0 0 0 6px rgba(245, 158, 11, 0);
    }
  }

  .editor-container {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .editor-container :global(.huml-editor) {
    height: 100%;
  }
</style>
