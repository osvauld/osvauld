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
    justify-content: center;
    gap: 5px;
    color: #85889C;
    background: transparent;
    border: 1px solid #3a3b44;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
    min-width: 120px;
  }
  
  .format-dropdown-button:hover {
    background: #2a2b2f;
  }
  
  .dropdown-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0px;
    min-width: 200px;
    background: #16171f;
    border: 1px solid #2a2b2f;
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
    gap: 8px;
  }
  
  .dropdown-item:hover {
    background: #2a2b2f;
  }
  
  .has-submenu {
    position: relative;
  }
  
  .submenu {
    position: absolute;
    left: 100%;
    top: 0;
    min-width: 200px;
    background: #16171f;
    border: 1px solid #2a2b2f;
    border-radius: 4px;
    font-size: 14px;
    display: none;
    z-index: 101;
    box-shadow: 2px 2px 8px rgba(0, 0, 0, 0.3);
  }
  
  .submenu-item {
    padding: 8px 12px;
    cursor: pointer;
    color: #bfc0cc;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  
  .submenu-item:hover {
    background: #2a2b2f;
  }

  .font-size-controls {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    background: transparent;
    border: 1px solid #3a3b44;
    border-radius: 4px;
  }

  .font-size-input {
    width: 40px;
    color: rgb(133, 136, 156);
    font-size: 14px;
    text-align: center;
  }

  .font-size-input:focus {
    outline: none;
    border-color: #4d4f60;
  }

  .size-adjust-button {
    background: transparent;
    border: none;
    color: #85889C;
    cursor: pointer;
    padding: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }

  .size-adjust-button:hover {
    color: #bfc0cc;
    background: #2a2b2f;
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