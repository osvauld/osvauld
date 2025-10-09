/**
 * UI State Management for Sthalam
 * Handles UI-related state like expanded folders, modals, etc.
 */
class UIState {
  // Navigation panel
  showNavigationPanel = $state(true);

  // Folder expansion state
  private expandedFolders = $state<Set<string>>(new Set());

  // Resource view state
  showResourceView = $state(false);

  // Welcome screen
  showWelcome = $state(false);

  // Folder manager modal
  showFolderManager = $state(false);

  /**
   * Toggle navigation panel visibility
   */
  toggleNavigationPanel(show?: boolean) {
    this.showNavigationPanel = show ?? !this.showNavigationPanel;
  }

  /**
   * Toggle folder manager modal
   */
  toggleFolderManager() {
    this.showFolderManager = !this.showFolderManager;
  }

  /**
   * Check if a folder is expanded
   */
  isFolderExpanded(folderId: string): boolean {
    return this.expandedFolders.has(folderId);
  }

  /**
   * Toggle folder expansion
   */
  toggleFolderExpansion(folderId: string) {
    if (this.expandedFolders.has(folderId)) {
      this.expandedFolders.delete(folderId);
    } else {
      this.expandedFolders.add(folderId);
    }
    // Trigger reactivity
    this.expandedFolders = new Set(this.expandedFolders);
  }

  /**
   * Expand a folder
   */
  expandFolder(folderId: string) {
    this.expandedFolders.add(folderId);
    this.expandedFolders = new Set(this.expandedFolders);
  }

  /**
   * Collapse a folder
   */
  collapseFolder(folderId: string) {
    this.expandedFolders.delete(folderId);
    this.expandedFolders = new Set(this.expandedFolders);
  }

  /**
   * Toggle resource view layout
   */
  toggleResourceView(show: boolean) {
    this.showResourceView = show;
  }

  /**
   * Set welcome screen visibility
   */
  setWelcomeScreen(show: boolean) {
    this.showWelcome = show;
  }
}

export const uiState = new UIState();
