import * as Y from 'yjs';

/**
 * Merges a remote document update into a local document using Yjs CRDT capabilities
 * @param localResource The local version of the document
 * @param remoteResource The remote version of the document
 * @returns The merged document data without saving or updating current state
 */
export function mergeDocuments(localResource, remoteResource) {
  try {
    console.log("Merging document:", localResource);

    // Create a temporary Yjs document for merging
    const tempDoc = new Y.Doc();
    const tempType = tempDoc.getXmlFragment('prosemirror');

    // Convert states to Uint8Array if needed
    const localState = Array.isArray(localResource.data.yjs_state)
      ? new Uint8Array(localResource.data.yjs_state)
      : localResource.data.yjs_state;

    const remoteState = Array.isArray(remoteResource.data.yjs_state)
      ? new Uint8Array(remoteResource.data.yjs_state)
      : remoteResource.data.yjs_state;

    // Apply both states to our temporary document
    // Order matters here - Yjs will automatically resolve conflicts
    // based on the timestamp of each operation
    Y.applyUpdate(tempDoc, localState);
    Y.applyUpdate(tempDoc, remoteState);

    // Extract the merged content
    const mergedContent = tempType.toJSON();

    // Determine which resource is newer based on last_modified timestamp
    const useRemoteEditorState =
      remoteResource.data.last_modified > localResource.data.last_modified;

    // Use the editor state from the newer resource
    const editorState = useRemoteEditorState
      ? remoteResource.data.editor_state
      : localResource.data.editor_state;

    // Create the merged document data
    const mergedYjsState = Y.encodeStateAsUpdate(tempDoc);
    const mergedNoteData = {
      // Don't set client_id as it will be set when document is loaded normally
      content: mergedContent,
      editor_state: editorState,
      yjs_state: Array.from(mergedYjsState), // Convert to array for storage
      resource_id: localResource.id,
      last_modified: Date.now(),
      favourite: localResource.favourite, // Preserve favorite status
      folder_id: localResource.folder_id
    };

    // Clean up temp document
    tempDoc.destroy();

    // Return the merged data without saving or affecting current state
    return mergedNoteData;
  } catch (error) {
    console.error("Error merging documents:", error);
    throw error;
  }
}
