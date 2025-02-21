import { AffineEditorContainer } from '@blocksuite/presets';
import { Doc, Schema, Store, DocCollection } from '@blocksuite/store';
import { AffineSchemas } from '@blocksuite/blocks';

export function initEditor() {
  const schema = new Schema().register(AffineSchemas);
  const collection = new DocCollection({ schema });
  collection.meta.initialize();

  const doc = collection.createDoc({ id: 'page1' });

  doc.spaceDoc.on("update", (data) => {

    console.log(data);
  })
  // Initialize the doc first
  doc.load(() => {
    const pageBlockId = doc.addBlock('affine:page', {});
    doc.addBlock('affine:surface', {}, pageBlockId);
    const noteId = doc.addBlock('affine:note', {}, pageBlockId);
    doc.addBlock('affine:paragraph', {}, noteId);
  });
  collection.docSync.onStatusChange.subscribe(
    (state) => state,
    (syncState) => {
      console.log('Sync state changed:', JSON.stringify(
        syncState)
      );
    }
  );
  // Wait for doc to be ready then subscribe to sync

  // Listen to Y.js updates directly

  const editor = new AffineEditorContainer();
  editor.doc = doc;
  editor.slots.docLinkClicked.on(({ docId }) => {
    const target = <Doc>collection.getDoc(docId);
    editor.doc = target;
  });
  return { editor, collection };
}
