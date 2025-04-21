import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { setBlockType } from "prosemirror-commands";

// Add CSS for dropdowns
export const dropdownStyle = `
  .dropdown-container {
    position: relative;
    display: inline-block;
  }
  
  .format-dropdown-button {
    display: flex;
    align-items: center;
    gap: 5px;
    color: #bfc0cc;
    border: none;
    padding: 6px 12px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
  }
  
  .format-dropdown-button:hover {
    background: #2a2b2f;
  }
  
  .dropdown-menu {
    position: absolute;
    top: 110%;
    left: 0px;
    min-width: 160px;
    background: #2a2b2f;
    border: 1px solid #3a3b44;
    border-radius: 4px;
    font-size: 14px;
    z-index: 100;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }
  
  .dropdown-item {
    padding: 8px 12px;
    cursor: pointer;
    color: #bfc0cc;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  .dropdown-item:hover {
    background: #3a3b44;
  }
  
  .has-submenu {
    position: relative;
  }
  
  .submenu {
    position: absolute;
    left: 100%;
    top: 0;
    min-width: 160px;
    background: #2a2b2f;
    border: 1px solid #3a3b44;
    border-radius: 4px;
    font-size: 14px;
    display: none;
    z-index: 101;
    box-shadow: 2px 2px 8px rgba(0, 0, 0, 0.3);
    overflow: hidden;
  }
  
  .submenu-item {
    padding: 8px 12px;
    cursor: pointer;
    color: #bfc0cc;
  }
  
  .submenu-item:hover {
    background: #3a3b44;
  }
`;

// Add CSS for active dropdown items
export const activeItemStyle = `
  .active-menuitem {
    background-color: #3a3b44;
    box-shadow: 2px 0 0 0 rgb(124 145 249 / 1) inset;
  }
`;

// Helper function to hide all dropdowns
export function hideDropdowns() {
  const dropdowns = document.querySelectorAll(".dropdown-menu, .submenu");
  dropdowns.forEach((dropdown) => {
    if (dropdown instanceof HTMLElement) {
      dropdown.style.display = "none";
    }
  });
}

// Helper function to get current heading level
export function getCurrentHeadingLevel(state, schema) {
  const { selection } = state;
  const { $from } = selection;

  if ($from.depth > 0) {
    const parent = $from.node($from.depth);
    if (parent.type === schema.nodes.heading) {
      return parent.attrs.level;
    }
  }
  return null;
}

// Function to create heading submenu
export function createHeadingSubmenu(schema: Schema, view: EditorView) {
  const headingsSubmenu = document.createElement("div");
  headingsSubmenu.className = "submenu";

  const headingLevels = [
    { level: 1, text: "Level 1" },
    { level: 2, text: "Level 2" },
    { level: 3, text: "Level 3" },
    { level: 4, text: "Level 4" },
    { level: 5, text: "Level 5" },
    { level: 6, text: "Level 6" },
  ];

  headingLevels.forEach((heading) => {
    const headingOption = document.createElement("div");
    headingOption.className = "submenu-item";
    headingOption.textContent = heading.text;
    headingOption.addEventListener("click", (e) => {
      e.stopPropagation();
      setBlockType(schema.nodes.heading, { level: heading.level })(
        view.state,
        view.dispatch
      );
      view.focus();
      hideDropdowns();
    });
    headingsSubmenu.appendChild(headingOption);
  });

  return headingsSubmenu;
} 