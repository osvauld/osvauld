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
    console.log("📊 [SubmissionsStore] Yjs change detected!");

    // Log what changed
    if (this.submissionsBlocks) {
      const allSubmissions = this.getAllSubmissions();
      console.log("📊 [SubmissionsStore] Total submissions after change:", allSubmissions.length);
      console.log("📊 [SubmissionsStore] Submissions:", allSubmissions);
    }

    console.log("📊 [SubmissionsStore] Notifying", this.observers.size, "observers");
    this.notifyObservers();
  };

  /**
   * Get all submissions from all forms
   */
  getAllSubmissions(): Submission[] {
    console.log("📊 [SubmissionsStore.getAllSubmissions] Called");

    if (!this.submissionsBlocks) {
      console.log("📊 [SubmissionsStore.getAllSubmissions] No submissionsBlocks!");
      return [];
    }

    const submissions: Submission[] = [];

    console.log("📊 [SubmissionsStore.getAllSubmissions] submissionsBlocks size:", this.submissionsBlocks.size);

    // Log all keys in the map
    const allKeys: string[] = [];
    this.submissionsBlocks.forEach((value, key) => {
      allKeys.push(key);
    });
    console.log("📊 [SubmissionsStore.getAllSubmissions] All keys in map:", allKeys);

    // Iterate through all keys in submissionsBlocks
    this.submissionsBlocks.forEach((value, key) => {
      console.log("📊 [SubmissionsStore.getAllSubmissions] Checking key:", key, "value type:", typeof value, "value:", value);

      // Keys are in format: ${formId}_submissions
      if (key.endsWith("_submissions")) {
        const items = value?.items || [];
        console.log("📊 [SubmissionsStore.getAllSubmissions] Found", items.length, "items for key:", key);
        if (items.length > 0) {
          console.log("📊 [SubmissionsStore.getAllSubmissions] First item:", items[0]);
        }
        items.forEach((item: Submission) => {
          submissions.push(item);
        });
      }
    });

    console.log("📊 [SubmissionsStore.getAllSubmissions] Returning", submissions.length, "submissions");
    if (submissions.length > 0) {
      console.log("📊 [SubmissionsStore.getAllSubmissions] First submission:", submissions[0]);
      console.log("📊 [SubmissionsStore.getAllSubmissions] Last submission:", submissions[submissions.length - 1]);
    }
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
    console.log("📊 [SubmissionsStore] Calling", this.observers.size, "observer callbacks");
    this.observers.forEach((callback, index) => {
      console.log("📊 [SubmissionsStore] Calling observer callback", index);
      callback();
    });
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
