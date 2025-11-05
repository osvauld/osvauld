<script lang="ts">
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import { dataState, uiState } from '../state';
  import HUMLEditor from '../shared/codemirror/HUMLEditor.svelte';
  import NavigationPanel from '../components/NavigationPanel.svelte';
  import NavigationToggle from '../components/NavigationToggle.svelte';
  import ModeSwitcher from '../components/ModeSwitcher.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { readTextFile } from '@tauri-apps/plugin-fs';
  import { parseHUML } from '../lib/services/humlParser';
  import { templateImporter } from '../shared/loro/templateImporter';

  let humlContent = $state<string>('');
  let originalContent = $state<string>('');
  let isSaving = $state<boolean>(false);
  let lastSavedTime = $state<Date | null>(null);
  let autoSaveInterval: number | null = null;
  let isImporting = $state<boolean>(false);

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

    console.log('📂 [BuilderApp] Loading HUML content:', {
      resourceId,
      contentLength: content.length,
      currentContentLength: humlContent.length,
      isDifferent: content !== humlContent
    });

    humlContent = content;
    originalContent = content;

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
   */
  async function saveContent() {
    if (!dataState.currentResourceId || !isDirty || isSaving) {
      return;
    }

    try {
      isSaving = true;

      // Parse and update template tree if HUML is valid
      try {
        const parsed = parseHUML(humlContent);
        console.log('✅ [BuilderApp] HUML parsed successfully');

        // Update template tree from HUML
        await templateImporter.importFromHUML(humlContent);
      } catch (parseError) {
        console.warn('⚠️ [BuilderApp] HUML parse failed, saving source only:', parseError);
        // Continue to save the source even if parsing fails
      }

      // Update template doc with raw HUML source
      const templateMap = loroCoordinator.getTemplateMap();
      templateMap.set('huml_source', humlContent);

      // Commit all documents
      loroCoordinator.getDocuments().contentDoc.commit();
      loroCoordinator.getDocuments().templateDoc.commit();
      loroCoordinator.getDocuments().userContentDoc.commit();
      loroCoordinator.getDocuments().uiStateDoc.commit();

      // Save to backend
      await dataState.saveCurrentResource(dataState.currentResourceId);

      originalContent = humlContent;
      lastSavedTime = new Date();

      console.log('💾 [BuilderApp] Saved HUML content');
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
   * Import HUML file
   */
  async function importHUML() {
    if (!dataState.currentResourceId) {
      alert('Please select a resource first');
      return;
    }

    try {
      // Pick file
      const selected = await open({
        filters: [{
          name: 'HUML Template',
          extensions: ['huml']
        }],
        multiple: false
      });

      if (!selected || typeof selected !== 'string') {
        return;
      }

      isImporting = true;

      // Read file content
      const fileContent = await readTextFile(selected);

      // Parse HUML to validate it
      const parsed = parseHUML(fileContent);
      console.log('📥 [BuilderApp] Parsed HUML:', parsed);

      // Import HUML template
      // This stores raw HUML in templateDoc and extracts state to contentDoc
      await templateImporter.importFromHUML(fileContent);

      // Commit all document changes
      loroCoordinator.getDocuments().contentDoc.commit();
      loroCoordinator.getDocuments().templateDoc.commit();
      loroCoordinator.getDocuments().userContentDoc.commit();
      loroCoordinator.getDocuments().uiStateDoc.commit();

      // Save to backend
      await dataState.saveCurrentResource(dataState.currentResourceId);

      // Update editor
      humlContent = fileContent;
      originalContent = fileContent;
      lastSavedTime = new Date();

      console.log('✅ [BuilderApp] Imported HUML template');
    } catch (error) {
      console.error('❌ [BuilderApp] Import failed:', error);
      alert(`Failed to import: ${error}`);
    } finally {
      isImporting = false;
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
      <p>Select a HUML template from the sidebar or create a new one to get started.</p>
    </div>
  </div>
{:else}
  <!-- Resource selected - show HUML editor -->
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
          onclick={importHUML}
          class="btn-import"
          disabled={isImporting}
          title="Import HUML template file"
        >
          {isImporting ? '📥 Importing...' : '📥 Import'}
        </button>

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

  .btn-import,
  .btn-save,
  .btn-reload {
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