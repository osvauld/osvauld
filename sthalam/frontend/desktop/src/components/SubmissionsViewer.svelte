<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import type { Submission, SubmissionsStore } from "../lib/submissionsStore";
	import Dropdown from "./Dropdown.svelte";

	let allSubmissions = $state<Submission[]>([]);
	let selectedEvent = $state<string>("all");
	let uniqueEvents = $state<string[]>([]);
	let submissionsUnsubscribe: (() => void) | null = null;
	let submissionsStore: SubmissionsStore | null = null;

	// Filtered submissions based on selected event
	const filteredSubmissions = $derived(() => {
		if (selectedEvent === "all") {
			return allSubmissions;
		}
		return allSubmissions.filter((s) => s.eventName === selectedEvent);
	});

	// Setup subscription to submissions store
	function handleStoreReady(event: CustomEvent) {
		const storeInstance = event.detail.submissionsStore;
		setupSubmissionsSubscription(storeInstance);
	}

	function setupSubmissionsSubscription(storeInstance: SubmissionsStore) {
		// Clean up previous subscription
		if (submissionsUnsubscribe) {
			submissionsUnsubscribe();
			submissionsUnsubscribe = null;
		}

		// Store reference
		submissionsStore = storeInstance;

		// Subscribe to updates
		submissionsUnsubscribe = submissionsStore.subscribe(() => {
			console.log("📊 [SubmissionsViewer] Store notified of changes, calling loadSubmissions()");
			loadSubmissions();
		});

		console.log("📊 [SubmissionsViewer] Subscribed to SubmissionsStore");
		// Get initial submissions
		loadSubmissions();
	}

	// Load submissions from store
	function loadSubmissions() {
		console.log("📊 [SubmissionsViewer] loadSubmissions() called");

		if (!submissionsStore) {
			console.warn("📊 [SubmissionsViewer] No submissionsStore available");
			return;
		}

		// Get all submissions from store
		const submissions = submissionsStore.getAllSubmissions();
		console.log("📊 [SubmissionsViewer] Got submissions from store:", submissions.length);
		console.log("📊 [SubmissionsViewer] Submission IDs:", submissions.map(s => s.id));
		console.log("📊 [SubmissionsViewer] Submission timestamps:", submissions.map(s => new Date(s.timestamp).toISOString()));

		// Sort by timestamp (newest first)
		submissions.sort((a, b) => b.timestamp - a.timestamp);

		allSubmissions = submissions;
		uniqueEvents = submissionsStore.getUniqueEvents();

		console.log("📊 [SubmissionsViewer] Updated allSubmissions:", allSubmissions.length, "Events:", uniqueEvents);
		console.log("📊 [SubmissionsViewer] Filtered submissions for display:", filteredSubmissions().length);
	}

	// Format timestamp for display
	function formatTimestamp(timestamp: number): string {
		const date = new Date(timestamp);
		return date.toLocaleString();
	}

	// Format data fields for display
	function formatData(data: Record<string, any>): string {
		return Object.entries(data)
			.map(([key, value]) => `${key}: ${value}`)
			.join(", ");
	}

	// Get all field names from submissions
	const allFieldNames = $derived(() => {
		const fields = new Set<string>();
		filteredSubmissions().forEach((submission) => {
			Object.keys(submission.data).forEach((key) => fields.add(key));
		});
		return Array.from(fields).sort();
	});

	// Dropdown options for event filter
	const eventOptions = $derived(() => {
		const options = [
			{
				value: "all",
				label: "All Events",
				count: allSubmissions.length
			}
		];

		uniqueEvents.forEach(event => {
			const count = allSubmissions.filter((s) => s.eventName === event).length;
			options.push({
				value: event,
				label: event,
				count
			});
		});

		return options;
	});

	// Export submissions as CSV
	function exportToCSV() {
		const submissions = filteredSubmissions();
		if (submissions.length === 0) return;

		const fields = allFieldNames();
		const headers = ["Timestamp", "Event", ...fields];
		const rows = submissions.map((s) => [
			formatTimestamp(s.timestamp),
			s.eventName || "N/A",
			...fields.map((field) => s.data[field] || ""),
		]);

		const csv = [
			headers.join(","),
			...rows.map((row) =>
				row.map((cell) => `"${String(cell).replace(/"/g, '""')}"`).join(",")
			),
		].join("\n");

		const blob = new Blob([csv], { type: "text/csv" });
		const url = URL.createObjectURL(blob);
		const a = document.createElement("a");
		a.href = url;
		a.download = `submissions_${selectedEvent}_${Date.now()}.csv`;
		a.click();
		URL.revokeObjectURL(url);
	}

	// Export submissions as JSON
	function exportToJSON() {
		const submissions = filteredSubmissions();
		if (submissions.length === 0) return;

		const json = JSON.stringify(submissions, null, 2);
		const blob = new Blob([json], { type: "application/json" });
		const url = URL.createObjectURL(blob);
		const a = document.createElement("a");
		a.href = url;
		a.download = `submissions_${selectedEvent}_${Date.now()}.json`;
		a.click();
		URL.revokeObjectURL(url);
	}

	// Listen for store ready event (like livnote's pattern)
	onMount(() => {
		// Check if coordinator already exists (like NavButton does)
		import('../state').then(({ dataState }) => {
			const coordinator = dataState.getBlocksuiteCoordinator();
			if (coordinator) {
				const existingStore = coordinator.getSubmissionsStore();
				if (existingStore) {
					setupSubmissionsSubscription(existingStore);
				}
			}
		});

		document.addEventListener(
			"submissions-store-ready",
			handleStoreReady as EventListener
		);

		return () => {
			document.removeEventListener(
				"submissions-store-ready",
				handleStoreReady as EventListener
			);
		};
	});

	onDestroy(() => {
		if (submissionsUnsubscribe) {
			submissionsUnsubscribe();
		}
	});
</script>

<div class="submissions-viewer">
	<div class="viewer-header">
		<h2>Form Submissions</h2>
		<div class="header-controls">
			<!-- Event Filter Dropdown -->
			<div class="filter-group">
				<label for="event-filter">Filter by Event:</label>
				<Dropdown
					bind:value={selectedEvent}
					options={eventOptions()}
					onChange={(value) => selectedEvent = value || "all"}
					class="event-dropdown"
				/>
			</div>

			<!-- Export Buttons -->
			{#if filteredSubmissions().length > 0}
				<div class="export-buttons">
					<button class="export-btn" onclick={exportToCSV} title="Export as CSV">
						<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
							<path
								d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z"
							/>
						</svg>
						CSV
					</button>
					<button class="export-btn" onclick={exportToJSON} title="Export as JSON">
						<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
							<path
								d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z"
							/>
						</svg>
						JSON
					</button>
				</div>
			{/if}
		</div>
	</div>

	<div class="submissions-content">
		{#if filteredSubmissions().length === 0}
			<div class="empty-state">
				<div class="empty-icon">📝</div>
				<h3>No submissions yet</h3>
				<p>
					{#if selectedEvent === "all"}
						Form submissions will appear here once users submit forms.
					{:else}
						No submissions found for event "{selectedEvent}".
					{/if}
				</p>
			</div>
		{:else}
			<div class="table-container">
				<table class="submissions-table">
					<thead>
						<tr>
							<th>Timestamp</th>
							<th>Event</th>
							{#each allFieldNames() as field}
								<th>{field}</th>
							{/each}
						</tr>
					</thead>
					<tbody>
						{#each filteredSubmissions() as submission (submission.id)}
							<tr>
								<td class="timestamp-cell">{formatTimestamp(submission.timestamp)}</td>
								<td class="event-cell">{submission.eventName || "N/A"}</td>
								{#each allFieldNames() as field}
									<td class="data-cell">
										{submission.data[field] !== undefined
											? String(submission.data[field])
											: "-"}
									</td>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>

			<div class="submissions-count">
				Showing {filteredSubmissions().length} of {allSubmissions.length} submission{allSubmissions.length === 1 ? "" : "s"}
			</div>
		{/if}
	</div>
</div>

<style>
	.submissions-viewer {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		background: #010409;
		color: #c9d1d9;
	}

	.viewer-header {
		padding: 1.5rem;
		border-bottom: 1px solid #292a36;
		display: flex;
		justify-content: space-between;
		align-items: center;
		flex-wrap: wrap;
		gap: 1rem;
	}

	.viewer-header h2 {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0;
		color: #c9d1d9;
	}

	.header-controls {
		display: flex;
		align-items: center;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.filter-group {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.filter-group label {
		font-size: 0.875rem;
		color: #8b949e;
		white-space: nowrap;
	}

	.filter-group :global(.event-dropdown) {
		min-width: 200px;
	}

	.export-buttons {
		display: flex;
		gap: 0.5rem;
	}

	.export-btn {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		background: #16171f;
		color: #8a86e5;
		border: 1px solid #8a86e5;
		border-radius: 6px;
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.2s;
	}

	.export-btn:hover {
		background: #8a86e5;
		color: #0d0e13;
	}

	.export-btn svg {
		width: 1rem;
		height: 1rem;
	}

	.submissions-content {
		flex: 1;
		overflow: auto;
		padding: 1.5rem;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		text-align: center;
		padding: 2rem;
	}

	.empty-icon {
		font-size: 3rem;
		margin-bottom: 1rem;
		opacity: 0.3;
	}

	.empty-state h3 {
		font-size: 1.25rem;
		margin-bottom: 0.5rem;
		color: #c9d1d9;
	}

	.empty-state p {
		color: #8b949e;
		max-width: 400px;
	}

	.table-container {
		overflow: auto;
		border: 1px solid #292a36;
		border-radius: 8px;
	}

	.submissions-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.875rem;
	}

	.submissions-table thead {
		background: #16171f;
		position: sticky;
		top: 0;
		z-index: 10;
	}

	.submissions-table th {
		padding: 0.75rem 1rem;
		text-align: left;
		font-weight: 600;
		color: #c9d1d9;
		border-bottom: 1px solid #292a36;
		white-space: nowrap;
	}

	.submissions-table tbody tr {
		border-bottom: 1px solid #292a36;
		transition: background 0.2s;
	}

	.submissions-table tbody tr:hover {
		background: #16171f;
	}

	.submissions-table tbody tr:last-child {
		border-bottom: none;
	}

	.submissions-table td {
		padding: 0.75rem 1rem;
		color: #8b949e;
	}

	.timestamp-cell {
		white-space: nowrap;
		color: #c9d1d9;
		font-family: monospace;
		font-size: 0.8125rem;
	}

	.event-cell {
		color: #8a86e5;
		font-weight: 500;
	}

	.data-cell {
		max-width: 300px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.submissions-count {
		margin-top: 1rem;
		padding: 0.75rem 1rem;
		text-align: center;
		color: #8b949e;
		font-size: 0.875rem;
		background: #16171f;
		border-radius: 6px;
	}
</style>
