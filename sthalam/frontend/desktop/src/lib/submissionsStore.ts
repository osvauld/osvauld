/**
 * Submissions Store - Manages form submissions in Loro submissionsDoc
 *
 * Submissions are stored as a Loro List (array) where each item is:
 * {id, timestamp, eventName, data, ...formFields}
 *
 * This matches the template definition: submissions:: type: "array"
 */

import type { LoroList } from 'loro-crdt';

export interface Submission {
  id: string;
  timestamp: number;
  eventName: string;
  data: Record<string, any>;
}

export class SubmissionsStore {
  private submissionsList: LoroList;
  private subscribers: Set<() => void> = new Set();

  constructor(submissionsList: LoroList) {
    this.submissionsList = submissionsList;

    // Subscribe to Loro document changes
    this.submissionsList.subscribe(() => {
      this.notifySubscribers();
    });

    console.log('📋 [SubmissionsStore] Initialized with List-based storage');
  }

  /**
   * Add a new form submission
   */
  addSubmission(eventName: string, data: Record<string, any>): string {
    const timestamp = Date.now();
    const id = `${timestamp}-${Math.random().toString(36).substr(2, 6)}`;

    // Store in Loro List as a map entry
    const submissionData = {
      id,
      timestamp,
      eventName,
      ...data  // Spread form data fields directly into the submission object
    };

    // Push to the end of the list (append-only)
    this.submissionsList.push(submissionData);

    console.log('📝 [SubmissionsStore] Added submission to list:', { id, eventName, data });

    // Notify subscribers
    this.notifySubscribers();

    return id;
  }

  /**
   * Get all submissions
   */
  getAllSubmissions(): Submission[] {
    const submissions: Submission[] = [];

    // Iterate over Loro List items
    const listData = this.submissionsList.toJSON() as any[];

    for (const item of listData) {
      if (item && typeof item === 'object') {
        submissions.push({
          id: item.id || '',
          timestamp: item.timestamp || 0,
          eventName: item.eventName || '',
          data: item  // The entire item IS the data (includes all form fields)
        });
      }
    }

    // Sort by timestamp (newest first)
    submissions.sort((a, b) => b.timestamp - a.timestamp);

    console.log(`📊 [SubmissionsStore] Retrieved ${submissions.length} submissions from list`);

    return submissions;
  }

  /**
   * Get unique event names from all submissions
   */
  getUniqueEvents(): string[] {
    const events = new Set<string>();

    const submissions = this.getAllSubmissions();
    submissions.forEach(s => {
      if (s.eventName) {
        events.add(s.eventName);
      }
    });

    return Array.from(events).sort();
  }

  /**
   * Get submissions for a specific event
   */
  getSubmissionsByEvent(eventName: string): Submission[] {
    return this.getAllSubmissions().filter(s => s.eventName === eventName);
  }

  /**
   * Subscribe to submission changes
   * Returns unsubscribe function
   */
  subscribe(callback: () => void): () => void {
    this.subscribers.add(callback);

    return () => {
      this.subscribers.delete(callback);
    };
  }

  /**
   * Notify all subscribers of changes
   */
  private notifySubscribers(): void {
    this.subscribers.forEach(callback => callback());
  }

  /**
   * Clear all submissions (for testing/admin)
   */
  clearAll(): void {
    // Clear the list by deleting from end to start (to avoid index shifting issues)
    const length = this.submissionsList.length;
    for (let i = length - 1; i >= 0; i--) {
      this.submissionsList.delete(i, 1);
    }

    console.log('🗑️  [SubmissionsStore] Cleared all submissions from list');
    this.notifySubscribers();
  }
}
