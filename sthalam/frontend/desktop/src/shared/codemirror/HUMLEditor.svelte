<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, ViewUpdate, keymap, Decoration } from '@codemirror/view';
  import type { DecorationSet } from '@codemirror/view';
  import { EditorState, StateField } from '@codemirror/state';
  import type { Range } from '@codemirror/state';
  import { indentUnit, foldGutter, foldKeymap } from '@codemirror/language';
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
  import { search, searchKeymap } from '@codemirror/search';
  import { huml } from './huml';
  import { humlTheme } from './humlTheme';

  // Props
  let { value = $bindable(''), onchange }: { value?: string; onchange?: (value: string) => void } = $props();

  let editorContainer: HTMLDivElement;
  let editorView: EditorView | null = null;

  /**
   * Build decorations for hiding quotes and list markers
   */
  const hideMarks = StateField.define<DecorationSet>({
    create(state) {
      return buildDecorations(state);
    },
    update(decorations, tr) {
      if (tr.docChanged) {
        return buildDecorations(tr.state);
      }
      return decorations.map(tr.changes);
    },
    provide: f => EditorView.decorations.from(f)
  });

  function buildDecorations(state: EditorState): DecorationSet {
    const builder: Range<Decoration>[] = [];

    for (let i = 1; i <= state.doc.lines; i++) {
      const line = state.doc.line(i);
      const text = line.text;

      // Hide `- ::` completely at start of list items
      const listMatch = /^(\s*)-\s+::/;
      const listResult = listMatch.exec(text);
      if (listResult) {
        const start = line.from + listResult[1].length;
        const end = start + 4; // "- ::" is 4 characters
        // Hide the entire "- ::" without replacement
        builder.push(Decoration.replace({}).range(start, end));
      }

      // Hide quotes
      let idx = 0;
      while (idx < text.length) {
        const char = text[idx];

        if (char === '"' || char === "'") {
          const quoteChar = char;
          const openPos = line.from + idx;

          // Find closing quote
          let closeIdx = idx + 1;
          let escaped = false;

          while (closeIdx < text.length) {
            if (text[closeIdx] === '\\' && !escaped) {
              escaped = true;
              closeIdx++;
              continue;
            }

            if (text[closeIdx] === quoteChar && !escaped) {
              const closePos = line.from + closeIdx;

              // Hide both quotes
              builder.push(Decoration.replace({}).range(openPos, openPos + 1));
              builder.push(Decoration.replace({}).range(closePos, closePos + 1));

              idx = closeIdx + 1;
              break;
            }

            escaped = false;
            closeIdx++;
          }

          if (closeIdx >= text.length) {
            idx++;
          }
        } else {
          idx++;
        }
      }
    }

    return Decoration.set(builder, true);
  }

  onMount(() => {
    const startState = EditorState.create({
      doc: value,
      extensions: [
        // Basic setup
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap, ...foldKeymap, ...searchKeymap]),

        // Search functionality
        search(),

        // Language and theme
        huml(),
        humlTheme,

        // Editor configuration
        indentUnit.of('  '),
        EditorView.lineWrapping,

        // Code folding
        foldGutter({
          openText: '▼',
          closedText: '▶',
        }),

        // Hide quotes and format CSS
        hideMarks,

        // Update handler
        EditorView.updateListener.of((update: ViewUpdate) => {
          if (update.docChanged) {
            const newValue = update.state.doc.toString();
            value = newValue;
            if (onchange) {
              onchange(newValue);
            }
          }
        }),

        // Styling
        EditorView.theme({
          '&': {
            height: '100%',
            fontSize: '14px',
          },
          '.cm-scroller': {
            overflow: 'auto',
            fontFamily: '"Fira Code", "JetBrains Mono", Consolas, monospace',
          },
          '.cm-gutters': {
            minWidth: '70px',
          },
          '.cm-foldGutter': {
            width: '20px',
            padding: '0 4px',
            cursor: 'pointer',
          },
        }),
      ],
    });

    editorView = new EditorView({
      state: startState,
      parent: editorContainer,
    });
  });

  onDestroy(() => {
    if (editorView) {
      editorView.destroy();
      editorView = null;
    }
  });

  // Watch for external value changes
  $effect(() => {
    if (editorView && value !== editorView.state.doc.toString()) {
      console.log('📝 [HUMLEditor] Updating editor:', {
        newValueLength: value.length,
        currentDocLength: editorView.state.doc.length,
        isDifferent: value !== editorView.state.doc.toString()
      });
      editorView.dispatch({
        changes: {
          from: 0,
          to: editorView.state.doc.length,
          insert: value,
        },
      });
    } else if (editorView) {
      console.log('📝 [HUMLEditor] Value unchanged, skipping update:', value.length);
    }
  });
</script>

<div bind:this={editorContainer} class="huml-editor"></div>

<style>
  .huml-editor {
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .huml-editor :global(.cm-editor) {
    height: 100%;
  }

  .huml-editor :global(.cm-scroller) {
    overflow: auto !important;
  }
</style>
