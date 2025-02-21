<!-- EditorToolbar.svelte -->
<script>
  import { setBlockType, toggleMark } from "prosemirror-commands";
  import { wrapInList } from "prosemirror-schema-list";

  // The editor view is passed as a prop from the parent component
  export let editorView;
  export let isDarkMode = false;

  // These reactive statements track the active state of each formatting option
  // They update automatically whenever the editor's state changes
  $: isH1 = isNodeActive("heading", { level: 1 });
  $: isH2 = isNodeActive("heading", { level: 2 });
  $: isH3 = isNodeActive("heading", { level: 3 });
  $: isParagraph = isNodeActive("paragraph");
  $: isBulletList = isNodeActive("bullet_list");
  $: isOrderedList = isNodeActive("ordered_list");
  $: isCodeBlock = isNodeActive("code_block");

  // This function checks if a particular node type is active at the current cursor position
  function isNodeActive(typeName, attrs = {}) {
    if (!editorView) return false;
    const state = editorView.state;
    const { from, to, empty } = state.selection;
    const node = empty
      ? state.selection.$from.parent
      : state.doc.cut(from, to).content.firstChild;
    return node.hasMarkup(state.schema.nodes[typeName], attrs);
  }
  $: isBold = isMarkActive("strong");
  $: isItalic = isMarkActive("em");
  function isMarkActive(markName) {
    if (!editorView) return false;
    const { from, $from, to, empty } = editorView.state.selection;
    const mark = editorView.state.schema.marks[markName];

    if (empty) {
      return !!mark.isInSet(editorView.state.storedMarks || $from.marks());
    }
    return editorView.state.doc.rangeHasMark(from, to, mark);
  }
  // Helper function to execute ProseMirror commands
  function runCommand(command) {
    return () => {
      if (!editorView) return;
      command(editorView.state, editorView.dispatch, editorView);
      editorView.focus();
    };
  }

  // Define all our formatting commands
  const commands = {
    // Convert the current block to a paragraph
    paragraph: () =>
      runCommand(setBlockType(editorView.state.schema.nodes.paragraph))(),

    // Convert the current block to different heading levels
    h1: () =>
      runCommand(
        setBlockType(editorView.state.schema.nodes.heading, { level: 1 })
      )(),
    h2: () =>
      runCommand(
        setBlockType(editorView.state.schema.nodes.heading, { level: 2 })
      )(),
    h3: () =>
      runCommand(
        setBlockType(editorView.state.schema.nodes.heading, { level: 3 })
      )(),

    // Convert the current block to a list item within a list
    bulletList: () =>
      runCommand(wrapInList(editorView.state.schema.nodes.bullet_list))(),
    orderedList: () =>
      runCommand(wrapInList(editorView.state.schema.nodes.ordered_list))(),

    // Convert the current block to a code block
    codeBlock: () =>
      runCommand(setBlockType(editorView.state.schema.nodes.code_block))(),
    toggleBold: () =>
      runCommand(toggleMark(editorView.state.schema.marks.strong))(),

    toggleItalic: () =>
      runCommand(toggleMark(editorView.state.schema.marks.em))(),

    setTextColor: (color) =>
      runCommand(
        toggleMark(editorView.state.schema.marks.textColor, { color })
      )(),
  };
</script>

<div class="toolbar" class:dark={isDarkMode}>
  <div class="toolbar-group">
    <button
      class:active={isParagraph}
      on:click={commands.paragraph}
      title="Normal text"
    >
      ¶
    </button>
    <button class:active={isH1} on:click={commands.h1} title="Heading 1">
      H1
    </button>
    <button class:active={isH2} on:click={commands.h2} title="Heading 2">
      H2
    </button>
    <button class:active={isH3} on:click={commands.h3} title="Heading 3">
      H3
    </button>
  </div>

  <div class="toolbar-group">
    <button
      class:active={isBulletList}
      on:click={commands.bulletList}
      title="Bullet list"
    >
      • List
    </button>
    <button
      class:active={isOrderedList}
      on:click={commands.orderedList}
      title="Numbered list"
    >
      1. List
    </button>
  </div>

  <div class="toolbar-group">
    <button
      class:active={isCodeBlock}
      on:click={commands.codeBlock}
      title="Code block"
    >
      &lt;/&gt;
    </button>
  </div>
  <div class="toolbar-group">
    <button class:active={isBold} on:click={commands.toggleBold} title="Bold">
      B
    </button>
    <button
      class:active={isItalic}
      on:click={commands.toggleItalic}
      title="Italic"
    >
      I
    </button>
    <select
      on:change={(e) => commands.setTextColor(e.target.value)}
      title="Text color"
    >
      <option value="">Color</option>
      <option value="#000000">Black</option>
      <option value="#FF0000">Red</option>
      <option value="#00FF00">Green</option>
      <option value="#0000FF">Blue</option>
      <option value="#FF00FF">Purple</option>
    </select>
  </div>

  <button
    class="theme-swither"
    class:dark={isDarkMode}
    on:click={() => (isDarkMode = !isDarkMode)}
  >
    {isDarkMode ? "☀️" : "🌙"}
  </button>
</div>

<style>
  .toolbar {
    display: flex;
    gap: 1rem;
    padding: 0.75rem;
    background: white;
    border: 1px solid #e2e8f0;
    border-top: none;
    z-index: 10;
    position: relative;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  }

  .toolbar.dark {
    background-color: #212121;
  }

  .toolbar-group {
    display: flex;
    gap: 0.25rem;
    align-items: center;
    padding: 0 0.5rem;
    border-right: 1px solid #e2e8f0;
  }

  .toolbar-group:last-child {
    border-right: none;
  }

  button {
    padding: 0.5rem 0.75rem;
    border: 1px solid #e2e8f0;
    background: white;
    border-radius: 0.25rem;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 2.5rem;
    color: #1a1a1a;
    line-height: 1;
  }

  button:hover {
    background: #f8fafc;
    border-color: #cbd5e1;
  }

  button.active {
    background: #e2e8f0;
    border-color: #94a3b8;
    color: #1e293b;
  }

  button:focus {
    outline: 2px solid #60a5fa;
    outline-offset: 2px;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  select {
    padding: 0.5rem;
    border: 1px solid #e2e8f0;
    border-radius: 0.25rem;
    background: white;
    font-size: 0.875rem;
    cursor: pointer;
    color: black;
  }

  select:hover {
    background: #f8fafc;
    border-color: #cbd5e1;
  }

  select:focus {
    outline: 2px solid #60a5fa;
    outline-offset: 2px;
  }

  button.theme-swither {
    background-color: white;
    margin-left: auto;
  }

  button.theme-swither:active {
    transform: scale(0.95);
  }

  button.theme-swither:focus {
    outline: none;
  }

  button.dark {
    background-color: #212121;
  }
</style>
