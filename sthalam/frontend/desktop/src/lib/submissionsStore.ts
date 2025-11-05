/**
 * Submissions Store - Manages form submissions in Loro submissionsDoc
 *
 * Submissions are stored as a Loro Map where:
 * - Key: submission ID (timestamp-random)
 * - Value: {timestamp, eventName, data}
 */

import type { LoroMap } from 'loro-crdt';

export interface Submission {
  id: string;
  timestamp: number;
  eventName: string;
  data: Record<string, any>;
}

export class SubmissionsStore {
  private submissionsMap: LoroMap;
  private subscribers: Set<() => void> = new Set();

  constructor(submissionsMap: LoroMap) {
    this.submissionsMap = submissionsMap;

    // Subscribe to Loro document changes
    this.submissionsMap.subscribe(() => {
      this.notifySubscribers();
    });

    console.log('📋 [SubmissionsStore] Initialized');
  }

  /**
   * Add a new form submission
   */
  addSubmission(eventName: string, data: Record<string, any>): string {
    const timestamp = Date.now();
    const id = `${timestamp}-${Math.random().toString(36).substr(2, 6)}`;

    // Store in Loro Map
    const submissionData = {
      timestamp,
      eventName,
      data
    };

    this.submissionsMap.set(id, submissionData);

    console.log('📝 [SubmissionsStore] Added submission:', { id, eventName, data });

    // Notify subscribers
    this.notifySubscribers();

    return id;
  }

  /**
   * Get all submissions
   */
  getAllSubmissions(): Submission[] {
    const submissions: Submission[] = [];

    // Iterate over Loro Map entries
    const entries = this.submissionsMap.toJSON() as Record<string, any>;

    for (const [id, value] of Object.entries(entries)) {
      if (value && typeof value === 'object') {
        submissions.push({
          id,
          timestamp: value.timestamp || 0,
          eventName: value.eventName || '',
          data: value.data || {}
        });
      }
    }

    // Sort by timestamp (newest first)
    submissions.sort((a, b) => b.timestamp - a.timestamp);

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
    const entries = this.submissionsMap.toJSON() as Record<string, any>;
    Object.keys(entries).forEach(key => {
      this.submissionsMap.delete(key);
    });

    console.log('🗑️  [SubmissionsStore] Cleared all submissions');
    this.notifySubscribers();
  }
}
