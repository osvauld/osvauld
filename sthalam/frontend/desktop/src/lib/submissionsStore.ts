import * as Y from "yjs";

export interface Submission {
  id: string;
  formId: string;
  eventName: string;
  data: Record<string, any>;
  timestamp: number;
}

/**
 * Manages form submissions using YJS
 * Follows livnote's CommentsStore pattern
 */
export class SubmissionsStore {
  private submissionsBlocks: Y.Map<any> | null = null;
  private submissionsDoc: Y.Doc | null = null;
  private observers: Set<() => void> = new Set();

  /**
   * Set the YJS map and doc for submissions
   */
  setSubmissionsMap(submissionsBlocks: Y.Map<any>, submissionsDoc: Y.Doc): void {
    // Clean up old observer if any
    if (this.submissionsBlocks) {
      this.submissionsBlocks.unobserveDeep(this.handleSubmissionsChange);
    }

    this.submissionsBlocks = submissionsBlocks;
    this.submissionsDoc = submissionsDoc;

    // Set up observer
    this.submissionsBlocks.observeDeep(this.handleSubmissionsChange);

    console.log("📊 [SubmissionsStore] Yjs map set and observer attached");
  }

  /**
   * Handle changes to submissions
   */
  private handleSubmissionsChange = (): void => {
    console.log("📊 [SubmissionsStore] Yjs change detected, notifying observers");
    this.notifyObservers();
  };

  /**
   * Get all submissions from all forms
   */
  getAllSubmissions(): Submission[] {
    if (!this.submissionsBlocks) {
      return [];
    }

    const submissions: Submission[] = [];

    // Iterate through all keys in submissionsBlocks
    this.submissionsBlocks.forEach((value, key) => {
      // Keys are in format: ${formId}_submissions
      if (key.endsWith("_submissions")) {
        const items = value?.items || [];
        items.forEach((item: Submission) => {
          submissions.push(item);
        });
      }
    });

    return submissions;
  }

  /**
   * Get submissions for a specific form
   */
  getFormSubmissions(formId: string): Submission[] {
    if (!this.submissionsBlocks) {
      return [];
    }

    const key = `${formId}_submissions`;
    const value = this.submissionsBlocks.get(key);
    return value?.items || [];
  }

  /**
   * Get unique event names from all submissions
   */
  getUniqueEvents(): string[] {
    const submissions = this.getAllSubmissions();
    const events = new Set<string>();

    submissions.forEach((submission) => {
      if (submission.eventName) {
        events.add(submission.eventName);
      }
    });

    return Array.from(events).sort();
  }

  /**
   * Add a submission to a form
   */
  addSubmission(formId: string, submission: Omit<Submission, 'id' | 'timestamp'>): void {
    if (!this.submissionsDoc || !this.submissionsBlocks) {
      throw new Error("SubmissionsStore not properly initialized");
    }

    const newSubmission: Submission = {
      ...submission,
      id: crypto.randomUUID(),
      timestamp: Date.now()
    };

    this.submissionsDoc.transact(() => {
      const key = `${formId}_submissions`;
      const current = this.submissionsBlocks!.get(key) || { items: [] };

      this.submissionsBlocks!.set(key, {
        items: [...current.items, newSubmission]
      });
    });
  }

  /**
   * Subscribe to submission changes
   */
  subscribe(callback: () => void): () => void {
    this.observers.add(callback);
    return () => {
      this.observers.delete(callback);
    };
  }

  /**
   * Notify all observers
   */
  private notifyObservers(): void {
    this.observers.forEach((callback) => callback());
  }

  /**
   * Clean up
   */
  destroy(): void {
    if (this.submissionsBlocks) {
      this.submissionsBlocks.unobserveDeep(this.handleSubmissionsChange);
    }

    this.observers.clear();
    this.submissionsBlocks = null;
  }
}
