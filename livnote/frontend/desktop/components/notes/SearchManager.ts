import { Plugin } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import {
  search,
  SearchQuery,
  findNext,
  findPrev,
  replaceNext,
  replaceAll,
  replaceCurrent,
  getSearchState,
  setSearchState,
  type SearchResult
} from "prosemirror-search";

export interface SearchOptions {
  search: string;
  replace?: string;
  caseSensitive?: boolean;
  wholeWord?: boolean;
  regexp?: boolean;
}

export interface SearchState {
  query: SearchQuery | null;
  isActive: boolean;
  currentMatch: number;
  totalMatches: number;
  isReplaceMode: boolean;
}

export class SearchManager {
  private plugin: Plugin;
  private currentQuery: SearchQuery | null = null;
  private state: SearchState = {
    query: null,
    isActive: false,
    currentMatch: 0,
    totalMatches: 0,
    isReplaceMode: false
  };

  private listeners: Array<(state: SearchState) => void> = [];

  constructor() {
    this.plugin = search({});
  }

  /**
   * Get the search plugin to add to ProseMirror
   */
  getPlugin(): Plugin {
    return this.plugin;
  }

  /**
   * Subscribe to search state changes
   */
  subscribe(listener: (state: SearchState) => void): () => void {
    this.listeners.push(listener);
    return () => {
      const index = this.listeners.indexOf(listener);
      if (index > -1) {
        this.listeners.splice(index, 1);
      }
    };
  }

  /**
   * Notify all listeners of state changes
   */
  private notifyListeners(): void {
    this.listeners.forEach(listener => listener({ ...this.state }));
  }

  /**
   * Start a new search
   */
  startSearch(view: EditorView, options: SearchOptions): void {
    const query = new SearchQuery({
      search: options.search,
      replace: options.replace || "",
      caseSensitive: options.caseSensitive || false,
      wholeWord: options.wholeWord || false,
      regexp: options.regexp || false
    });

    if (query.valid) {
      const tr = setSearchState(view.state.tr, query);
      view.dispatch(tr);

      this.currentQuery = query;
      this.state = {
        query,
        isActive: true,
        currentMatch: 0,
        totalMatches: this.countMatches(view, query),
        isReplaceMode: Boolean(options.replace)
      };

      this.notifyListeners();
    }
  }

  /**
   * Update search options
   */
  updateSearch(view: EditorView, options: Partial<SearchOptions>): void {
    if (!this.currentQuery) return;

    const currentOptions = {
      search: this.currentQuery.search,
      replace: this.currentQuery.replace,
      caseSensitive: this.currentQuery.caseSensitive,
      wholeWord: this.currentQuery.wholeWord,
      regexp: this.currentQuery.regexp
    };

    const newOptions = { ...currentOptions, ...options };
    this.startSearch(view, newOptions);
  }

  /**
   * Clear search
   */
  clearSearch(view: EditorView): void {
    const emptyQuery = new SearchQuery({ search: "" });
    const tr = setSearchState(view.state.tr, emptyQuery);
    view.dispatch(tr);

    this.currentQuery = null;
    this.state = {
      query: null,
      isActive: false,
      currentMatch: 0,
      totalMatches: 0,
      isReplaceMode: false
    };

    this.notifyListeners();
  }

  /**
   * Find next match
   */
  findNext(view: EditorView): boolean {
    if (!this.currentQuery || !this.currentQuery.valid) return false;

    const result = findNext(view.state, view.dispatch, view);
    if (result) {
      this.updateMatchPosition(view);
      this.scrollToCurrentMatch(view);
    }
    return result;
  }

  /**
   * Find previous match
   */
  findPrevious(view: EditorView): boolean {
    if (!this.currentQuery || !this.currentQuery.valid) return false;

    const result = findPrev(view.state, view.dispatch, view);
    if (result) {
      this.updateMatchPosition(view);
      this.scrollToCurrentMatch(view);
    }
    return result;
  }

  /**
   * Scroll to the current highlighted match
   */
  private scrollToCurrentMatch(view: EditorView): void {
    // The prosemirror-search plugin automatically highlights the current match
    // but we can add extra scrolling behavior if needed
    const activeMatch = view.dom.querySelector('.ProseMirror-active-search-match');
    if (activeMatch) {
      activeMatch.scrollIntoView({
        behavior: 'smooth',
        block: 'center',
        inline: 'nearest'
      });
    }
  }

  /**
   * Replace current match and find next
   */
  replaceNext(view: EditorView): boolean {
    if (!this.currentQuery || !this.currentQuery.valid) return false;

    const result = replaceNext(view.state, view.dispatch, view);
    if (result) {
      this.state.totalMatches = this.countMatches(view, this.currentQuery);
      this.notifyListeners();
    }
    return result;
  }

  /**
   * Replace current match only
   */
  replaceCurrent(view: EditorView): boolean {
    if (!this.currentQuery || !this.currentQuery.valid) return false;

    const result = replaceCurrent(view.state, view.dispatch, view);
    if (result) {
      this.state.totalMatches = this.countMatches(view, this.currentQuery);
      this.notifyListeners();
    }
    return result;
  }

  /**
   * Replace all matches
   */
  replaceAll(view: EditorView): boolean {
    if (!this.currentQuery || !this.currentQuery.valid) return false;

    const result = replaceAll(view.state, view.dispatch, view);
    if (result) {
      this.state.totalMatches = 0;
      this.state.currentMatch = 0;
      this.notifyListeners();
    }
    return result;
  }

  /**
   * Get current search state
   */
  getState(): SearchState {
    return { ...this.state };
  }

  /**
   * Count total matches for the current query
   */
  private countMatches(view: EditorView, query: SearchQuery): number {
    let count = 0;
    let pos = 0;
    const doc = view.state.doc;

    while (pos < doc.content.size) {
      const result = query.findNext(view.state, pos);
      if (!result) break;
      count++;
      pos = result.to;
    }

    return count;
  }

  /**
   * Update current match position
   */
  private updateMatchPosition(view: EditorView): void {
    if (!this.currentQuery) return;

    const selection = view.state.selection;
    let currentMatch = 0;
    let pos = 0;

    while (pos < selection.from) {
      const result = this.currentQuery.findNext(view.state, pos);
      if (!result) break;
      if (result.from < selection.from) {
        currentMatch++;
      }
      pos = result.to;
    }

    this.state.currentMatch = Math.max(1, currentMatch + 1);
    this.notifyListeners();
  }

  /**
   * Toggle replace mode
   */
  toggleReplaceMode(): void {
    this.state.isReplaceMode = !this.state.isReplaceMode;
    this.notifyListeners();
  }

  // Static command methods for use with keyboard shortcuts
  static commands = {
    findNext: findNext,
    findPrev: findPrev,
    replaceNext: replaceNext,
    replaceAll: replaceAll,
    replaceCurrent: replaceCurrent
  };
}
