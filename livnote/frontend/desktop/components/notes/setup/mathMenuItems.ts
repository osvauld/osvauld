import { Schema } from "prosemirror-model";
import { EditorView } from "prosemirror-view";
import { insertInlineMath, insertDisplayMath } from "../mathPlugin";

/**
 * Add math insertion buttons to the menu
 */
export function addMathItems(
  menu: HTMLElement,
  schema: Schema,
  editorView: EditorView
): void {
  // Inline Math button
  if (schema.nodes.math_inline) {
    const inlineMathButton = document.createElement("button");
    inlineMathButton.className = "menu-button math-inline-button";
    inlineMathButton.title = "Insert inline math ($...$)";
    inlineMathButton.innerHTML = `
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" font-family="serif" font-style="italic" font-size="16" fill="#85889C">π</text>
      </svg>
    `;

    inlineMathButton.onclick = (e) => {
      e.preventDefault();
      const { state, dispatch } = editorView;
      insertInlineMath(schema)(state, dispatch);
      editorView.focus();
    };

    menu.appendChild(inlineMathButton);
  }

  // Display Math button
  if (schema.nodes.math_display) {
    const displayMathButton = document.createElement("button");
    displayMathButton.className = "menu-button math-display-button";
    displayMathButton.title = "Insert display math ($$...$$)";
    displayMathButton.innerHTML = `
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" font-family="serif" font-weight="bold" font-size="18" fill="#85889C">∑</text>
      </svg>
    `;

    displayMathButton.onclick = (e) => {
      e.preventDefault();
      const { state, dispatch } = editorView;
      insertDisplayMath(schema)(state, dispatch);
      editorView.focus();
    };

    menu.appendChild(displayMathButton);
  }
}
