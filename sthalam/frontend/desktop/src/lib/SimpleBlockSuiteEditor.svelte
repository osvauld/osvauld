<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { PageEditor } from '@blocksuite/presets';
  import { Schema, DocCollection } from '@blocksuite/store';
  import { AffineSchemas } from '@blocksuite/blocks/schemas';
  import type { Doc } from '@blocksuite/store';

  let editorContainer = $state<HTMLDivElement>();
  let editor = $state<PageEditor | null>(null);
  let doc = $state<Doc | null>(null);

  onMount(async () => {
    console.log('🚀 [SimpleBlockSuiteEditor] Starting initialization...');

    try {
      // Create schema with default AFFiNE block schemas
      const schema = new Schema().register(AffineSchemas);
      console.log('✅ Schema created');

      // Create collection
      const collection = new DocCollection({ schema });
      collection.meta.initialize();
      console.log('✅ Collection initialized');

      // Create document
      doc = collection.createDoc({ id: 'doc:home' });
      console.log('✅ Document created:', doc.id);

      // Load document with initial block tree
      await doc.load(() => {
        // Create root page block
        const rootId = doc!.addBlock('affine:page', {});

        // Add surface block for graphics
        doc!.addBlock('affine:surface', {}, rootId);

        // Add note block (container for content)
        const noteId = doc!.addBlock('affine:note', {}, rootId);

        // Add some initial content
        doc!.addBlock('affine:paragraph', {}, noteId);
      });
      console.log('✅ Document loaded with block tree');

      // Reset history after initial load
      doc.resetHistory();
      console.log('✅ History reset');

      // Create PageEditor
      editor = new PageEditor();
      editor.doc = doc;
      console.log('✅ Editor created');

      // Append editor to DOM
      if (editorContainer) {
        editorContainer.appendChild(editor);
        console.log('✅ Editor appended to DOM');
      }

      // Expose to window for debugging
      if (typeof window !== 'undefined') {
        (window as any).blocksuiteEditor = editor;
        (window as any).blocksuiteDoc = doc;
        (window as any).blocksuiteCollection = collection;
        console.log('✅ BlockSuite exposed to window for debugging');
      }

      console.log('✅ BlockSuite editor ready!');

    } catch (error) {
      console.error('❌ Failed to initialize BlockSuite:', error);
      if (error instanceof Error) {
        console.error('Error details:', error.message, error.stack);
      }
    }
  });

  onDestroy(() => {
    console.log('🧹 Cleaning up BlockSuite editor...');
    if (editor && editorContainer?.contains(editor)) {
      editorContainer.removeChild(editor);
    }
    if (doc) {
      doc.collection.remove(doc.id);
    }
  });
</script>

<div class="blocksuite-editor-wrapper">
  <div bind:this={editorContainer} class="blocksuite-container"></div>
</div>

<style>
  .blocksuite-editor-wrapper {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .blocksuite-container {
    flex: 1;
    overflow: auto;
    background: var(--affine-background-primary-color, #fff);
  }

  :global(affine-editor-container) {
    display: block;
    height: 100%;
    width: 100%;
  }

  :global(editor-host) {
    display: block;
    height: 100%;
    width: 100%;
    font-family: var(--affine-font-family, sans-serif);
  }

  :global(editor-host > *) {
    height: 100%;
  }
</style>
