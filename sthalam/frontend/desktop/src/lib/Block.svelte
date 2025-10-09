<script lang="ts">
	interface Props {
		block: {
			id: string;
			type: string;
			x: number;
			y: number;
			width: number;
			height: number;
			zIndex: number;
			content: string;
			styles: Record<string, string>;
		};
		isSelected: boolean;
		onUpdate: (blockId: string, updates: any) => void;
		onSelect: (blockId: string) => void;
	}

	let { block, isSelected, onUpdate, onSelect }: Props = $props();

	let isDragging = $state(false);
	let isResizing = $state(false);
	let resizeHandle = $state<string | null>(null);
	let dragStart = $state({ x: 0, y: 0 });
	let blockStart = $state({ x: 0, y: 0, width: 0, height: 0 });
	let dragModeEnabled = $state(false); // Ctrl+click to enable
	let hoverEdge = $state<string | null>(null); // Track which edge is hovered

	// Notice board state
	let newMessageText = $state("");

	// Parse messages from block content
	const messages = $derived(() => {
		if (block.type !== "notice-board" || !block.content) return [];
		try {
			return JSON.parse(block.content);
		} catch {
			return [];
		}
	});

	// Form state
	let formData = $state<Record<string, any>>({});
	let submitMessage = $state<{type: string, text: string} | null>(null);

	// Parse form config from block content
	const formConfigData = $derived(
		block.type === "form" && block.content
			? (() => {
				try {
					return JSON.parse(block.content);
				} catch {
					return { fields: [], submitButtonText: "Submit" };
				}
			})()
			: { fields: [], submitButtonText: "Submit" }
	);

	function postMessage() {
		if (!newMessageText.trim()) return;

		const newMessage = {
			id: `msg-${Date.now()}`,
			username: "User", // TODO: Get from user context
			timestamp: Date.now(),
			content: newMessageText.trim()
		};

		const updatedMessages = [...messages(), newMessage];
		onUpdate(block.id, { content: JSON.stringify(updatedMessages) });
		newMessageText = "";
	}

	function formatTimestamp(timestamp: number): string {
		const date = new Date(timestamp);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / 60000);

		if (diffMins < 1) return "just now";
		if (diffMins < 60) return `${diffMins}m ago`;
		if (diffMins < 1440) return `${Math.floor(diffMins / 60)}h ago`;
		return date.toLocaleDateString();
	}

	function handleFormSubmit() {
		// TODO: Create subdoc and send to kunki (sovereign node)
		console.log("Form submitted:", formData);

		// Show success message
		submitMessage = {
			type: "success",
			text: "Form submitted successfully!"
		};

		// Clear form
		formData = {};

		// Hide message after 3 seconds
		setTimeout(() => {
			submitMessage = null;
		}, 3000);
	}

	function handleMouseDown(e: MouseEvent) {
		// Don't start drag if clicking on resize handle
		const target = e.target as HTMLElement;
		if (target.classList.contains("resize-handle")) {
			return;
		}

		// Always select the block
		onSelect(block.id);

		// Check if clicking near border for resize (no Shift required)
		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const x = e.clientX - rect.left;
		const y = e.clientY - rect.top;
		const edgeThreshold = 10; // 10px from edge
		const nearEdge = y < edgeThreshold || y > rect.height - edgeThreshold ||
		                 x < edgeThreshold || x > rect.width - edgeThreshold;

		if (nearEdge) {
			handleBorderResize(e);
			return;
		}

		// Check if Ctrl (or Cmd on Mac) is held for drag mode
		if (e.ctrlKey || e.metaKey) {
			// Ctrl+click enables drag mode
			isDragging = true;
			dragModeEnabled = true;
			dragStart = { x: e.clientX, y: e.clientY };
			blockStart = { x: block.x, y: block.y, width: block.width, height: block.height };
			e.stopPropagation(); // Prevent canvas panning
			e.preventDefault(); // Prevent text selection
		}
		// Otherwise, allow normal text editing (don't start dragging)
	}

	function handleBorderResize(e: MouseEvent) {
		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const x = e.clientX - rect.left;
		const y = e.clientY - rect.top;

		const edgeThreshold = 10; // 10px from edge
		const nearTop = y < edgeThreshold;
		const nearBottom = y > rect.height - edgeThreshold;
		const nearLeft = x < edgeThreshold;
		const nearRight = x > rect.width - edgeThreshold;

		let handle = "";

		// Determine which edge/corner
		if (nearTop && nearLeft) handle = "nw";
		else if (nearTop && nearRight) handle = "ne";
		else if (nearBottom && nearLeft) handle = "sw";
		else if (nearBottom && nearRight) handle = "se";
		else if (nearTop) handle = "n";
		else if (nearBottom) handle = "s";
		else if (nearLeft) handle = "w";
		else if (nearRight) handle = "e";
		else return; // Not near an edge

		// Start resizing
		isResizing = true;
		resizeHandle = handle;
		dragStart = { x: e.clientX, y: e.clientY };
		blockStart = { x: block.x, y: block.y, width: block.width, height: block.height };
		e.stopPropagation();
		e.preventDefault();
	}

	function handleResizeStart(e: MouseEvent, handle: string) {
		e.stopPropagation();
		e.preventDefault();

		// Shift+click for incremental resize
		if (e.shiftKey) {
			handleIncrementalResize(handle, e.ctrlKey || e.altKey);
			return;
		}

		// Normal drag resize
		isResizing = true;
		resizeHandle = handle;
		dragStart = { x: e.clientX, y: e.clientY };
		blockStart = { x: block.x, y: block.y, width: block.width, height: block.height };
	}

	function handleIncrementalResize(handle: string, decrease: boolean) {
		const increment = 20; // 20px per click
		const amount = decrease ? -increment : increment;
		let updates: any = {};

		switch (handle) {
			case "n": // Top edge
				updates = {
					y: block.y - amount,
					height: Math.max(30, block.height + amount),
				};
				break;
			case "s": // Bottom edge
				updates = {
					height: Math.max(30, block.height + amount),
				};
				break;
			case "e": // Right edge
				updates = {
					width: Math.max(50, block.width + amount),
				};
				break;
			case "w": // Left edge
				updates = {
					x: block.x - amount,
					width: Math.max(50, block.width + amount),
				};
				break;
			case "se": // Bottom-right
				updates = {
					width: Math.max(50, block.width + amount),
					height: Math.max(30, block.height + amount),
				};
				break;
			case "sw": // Bottom-left
				updates = {
					x: block.x - amount,
					width: Math.max(50, block.width + amount),
					height: Math.max(30, block.height + amount),
				};
				break;
			case "ne": // Top-right
				updates = {
					y: block.y - amount,
					width: Math.max(50, block.width + amount),
					height: Math.max(30, block.height + amount),
				};
				break;
			case "nw": // Top-left
				updates = {
					x: block.x - amount,
					y: block.y - amount,
					width: Math.max(50, block.width + amount),
					height: Math.max(30, block.height + amount),
				};
				break;
		}

		onUpdate(block.id, updates);
	}

	function handleMouseMove(e: MouseEvent) {
		if (isDragging) {
			const dx = e.clientX - dragStart.x;
			const dy = e.clientY - dragStart.y;

			onUpdate(block.id, {
				x: blockStart.x + dx,
				y: blockStart.y + dy,
			});
		} else if (isResizing && resizeHandle) {
			const dx = e.clientX - dragStart.x;
			const dy = e.clientY - dragStart.y;

			let updates: any = {};

			switch (resizeHandle) {
				case "nw": // Top-left
					updates = {
						x: blockStart.x + dx,
						y: blockStart.y + dy,
						width: Math.max(50, blockStart.width - dx),
						height: Math.max(30, blockStart.height - dy),
					};
					break;
				case "n": // Top
					updates = {
						y: blockStart.y + dy,
						height: Math.max(30, blockStart.height - dy),
					};
					break;
				case "ne": // Top-right
					updates = {
						y: blockStart.y + dy,
						width: Math.max(50, blockStart.width + dx),
						height: Math.max(30, blockStart.height - dy),
					};
					break;
				case "e": // Right
					updates = {
						width: Math.max(50, blockStart.width + dx),
					};
					break;
				case "se": // Bottom-right
					updates = {
						width: Math.max(50, blockStart.width + dx),
						height: Math.max(30, blockStart.height + dy),
					};
					break;
				case "s": // Bottom
					updates = {
						height: Math.max(30, blockStart.height + dy),
					};
					break;
				case "sw": // Bottom-left
					updates = {
						x: blockStart.x + dx,
						width: Math.max(50, blockStart.width - dx),
						height: Math.max(30, blockStart.height + dy),
					};
					break;
				case "w": // Left
					updates = {
						x: blockStart.x + dx,
						width: Math.max(50, blockStart.width - dx),
					};
					break;
			}

			onUpdate(block.id, updates);
		}
	}

	function handleMouseUp() {
		isDragging = false;
		isResizing = false;
		resizeHandle = null;
		dragModeEnabled = false; // Reset drag mode
	}

	function handleBlockMouseMove(e: MouseEvent) {
		// Only track hover edge when not dragging/resizing
		if (isDragging || isResizing) return;

		const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const x = e.clientX - rect.left;
		const y = e.clientY - rect.top;

		const edgeThreshold = 10;
		const nearTop = y < edgeThreshold;
		const nearBottom = y > rect.height - edgeThreshold;
		const nearLeft = x < edgeThreshold;
		const nearRight = x > rect.width - edgeThreshold;

		// Update hover edge for cursor styling
		if (nearTop && nearLeft) hoverEdge = "nw";
		else if (nearTop && nearRight) hoverEdge = "ne";
		else if (nearBottom && nearLeft) hoverEdge = "sw";
		else if (nearBottom && nearRight) hoverEdge = "se";
		else if (nearTop) hoverEdge = "n";
		else if (nearBottom) hoverEdge = "s";
		else if (nearLeft) hoverEdge = "w";
		else if (nearRight) hoverEdge = "e";
		else hoverEdge = null;
	}

	function handleBlockMouseLeave() {
		hoverEdge = null;
	}

	// Get cursor based on hover edge
	const blockCursor = $derived(() => {
		if (isResizing || isDragging) return "grabbing";
		if (!hoverEdge) return "default";

		// Show resize cursor when near edges
		const cursors: Record<string, string> = {
			n: "ns-resize",
			s: "ns-resize",
			e: "ew-resize",
			w: "ew-resize",
			ne: "nesw-resize",
			sw: "nesw-resize",
			nw: "nwse-resize",
			se: "nwse-resize",
		};

		return cursors[hoverEdge] || "default";
	});

	// Compute dynamic styles
	const computedStyles = $derived(() => {
		const baseStyles = {
			"font-size": block.styles.fontSize || "16px",
			"font-weight": block.styles.fontWeight || "400",
			"color": block.styles.color || "#333",
			"background-color": block.styles.backgroundColor || "transparent",
			"padding": block.styles.padding || "12px",
			"border": block.styles.border || "2px solid #ddd",
			"border-radius": block.styles.borderRadius || "4px",
		};

		return Object.entries(baseStyles)
			.map(([key, value]) => `${key}: ${value}`)
			.join("; ");
	});

	// Sanitize HTML to prevent script execution (no JS, only HTML/CSS)
	function sanitizeHTML(html: string): string {
		// Remove script tags and their content
		let sanitized = html.replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '');

		// Remove all event handler attributes (onclick, onload, etc.)
		sanitized = sanitized.replace(/\s*on\w+\s*=\s*["'][^"']*["']/gi, '');
		sanitized = sanitized.replace(/\s*on\w+\s*=\s*[^\s>]*/gi, '');

		// Remove javascript: URLs
		sanitized = sanitized.replace(/javascript:/gi, '');

		return sanitized;
	}
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<div
	class="block block-{block.type}"
	style:left="{block.x}px"
	style:top="{block.y}px"
	style:width="{block.width}px"
	style:height="{block.height}px"
	style:z-index={block.zIndex}
	style:cursor={blockCursor()}
	style={block.type === "html" ? "" : computedStyles()}
	onmousedown={handleMouseDown}
	onmousemove={handleBlockMouseMove}
	onmouseleave={handleBlockMouseLeave}
	class:dragging={isDragging}
	class:resizing={isResizing}
	class:selected={isSelected}
	class:shift-resize-mode={hoverEdge !== null}
>
	{#if block.type === "heading"}
		<div class="block-heading" contenteditable="true">
			{block.content}
		</div>
	{:else if block.type === "text"}
		<div class="block-text" contenteditable="true">
			{block.content}
		</div>
	{:else if block.type === "image"}
		<div class="block-image">
			{#if block.content && block.content !== ""}
				<img src={block.content} alt="User uploaded" />
			{:else}
				<div class="image-placeholder">
					<span class="placeholder-icon">🖼️</span>
					<span class="placeholder-text">Paste image URL in Properties</span>
				</div>
			{/if}
		</div>
	{:else if block.type === "container"}
		<div class="block-container">
			<span class="container-label">{block.content || "Container (drop items here)"}</span>
		</div>
	{:else if block.type === "html"}
		<div class="block-html">
			{#if block.content && block.content !== ""}
				{@html `${block.styles.css ? `<style>${block.styles.css}</style>` : ''}${sanitizeHTML(block.content)}`}
			{:else}
				<div class="html-placeholder">
					<span class="placeholder-icon">&lt;/&gt;</span>
					<span class="placeholder-text">Add HTML in Properties</span>
				</div>
			{/if}
		</div>
	{:else if block.type === "notice-board"}
		<div class="block-notice-board">
			{#if messages.length > 0}
				<div class="messages-container">
					{#each messages as message}
						<div class="message">
							<div class="message-avatar">
								{message.username.charAt(0).toUpperCase()}
							</div>
							<div class="message-body">
								<div class="message-header">
									<strong>{message.username}</strong>
									<span class="timestamp">{formatTimestamp(message.timestamp)}</span>
								</div>
								<div class="message-content">{message.content}</div>
							</div>
						</div>
					{/each}
				</div>
			{:else}
				<div class="notice-placeholder">
					<span class="placeholder-icon">💬</span>
					<span class="placeholder-text">No comments yet. Be the first to comment!</span>
				</div>
			{/if}
			<div class="message-input">
				<input
					type="text"
					bind:value={newMessageText}
					placeholder="Add a comment..."
					onkeydown={(e) => e.key === 'Enter' && postMessage()}
				/>
				<button onclick={postMessage}>Add Comment</button>
			</div>
		</div>
	{:else if block.type === "form"}
		<div class="block-form">
			<form class="custom-form" onsubmit={(e) => { e.preventDefault(); handleFormSubmit(); }}>
				{#each formConfigData.fields as field}
					<div class="form-field">
						<label>
							{field.label}
							{#if field.required}<span class="required">*</span>{/if}
						</label>
						{#if field.type === "textarea"}
							<textarea
								name={field.id}
								placeholder={field.placeholder}
								required={field.required}
								bind:value={formData[field.id]}
							></textarea>
						{:else if field.type === "checkbox"}
							<label class="checkbox-field">
								<input
									type="checkbox"
									name={field.id}
									bind:checked={formData[field.id]}
								/>
								<span>{field.placeholder || field.label}</span>
							</label>
						{:else}
							<input
								type={field.type}
								name={field.id}
								placeholder={field.placeholder}
								required={field.required}
								bind:value={formData[field.id]}
							/>
						{/if}
					</div>
				{/each}
				<button type="submit" class="form-submit-btn">
					{formConfigData.submitButtonText || "Submit"}
				</button>
			</form>
			{#if submitMessage}
				<div class="submit-message {submitMessage.type}">
					{submitMessage.text}
				</div>
			{/if}
		</div>
	{/if}

	<!-- Resize handles -->
	<div class="resize-handle resize-nw" onmousedown={(e) => handleResizeStart(e, "nw")}></div>
	<div class="resize-handle resize-n" onmousedown={(e) => handleResizeStart(e, "n")}></div>
	<div class="resize-handle resize-ne" onmousedown={(e) => handleResizeStart(e, "ne")}></div>
	<div class="resize-handle resize-e" onmousedown={(e) => handleResizeStart(e, "e")}></div>
	<div class="resize-handle resize-se" onmousedown={(e) => handleResizeStart(e, "se")}></div>
	<div class="resize-handle resize-s" onmousedown={(e) => handleResizeStart(e, "s")}></div>
	<div class="resize-handle resize-sw" onmousedown={(e) => handleResizeStart(e, "sw")}></div>
	<div class="resize-handle resize-w" onmousedown={(e) => handleResizeStart(e, "w")}></div>
</div>

<style>
	.block {
		position: absolute;
		transition: box-shadow 0.2s, outline 0.2s;
	}

	.block:hover {
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
	}

	/* Visual feedback when hovering near edge (border-resize ready) */
	.block.shift-resize-mode {
		outline: 4px solid rgba(102, 126, 234, 0.5);
		outline-offset: -4px;
	}

	.block:hover .resize-handle,
	.block.selected .resize-handle {
		opacity: 1;
	}

	.block.selected {
		box-shadow: 0 0 0 4px #667eea;
	}

	.block.dragging {
		opacity: 0.8;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
	}

	.block.resizing {
		opacity: 0.8;
	}

	.block-heading,
	.block-text,
	.block-container,
	.block-image,
	.block-html,
	.block-notice-board {
		width: 100%;
		height: 100%;
		outline: none;
		overflow: auto;
		/* Add padding to prevent scrollbar from covering resize handles */
		box-sizing: border-box;
	}

	.block-html {
		padding: 0;
		background: transparent;
		border: none;
	}

	.block-notice-board {
		display: flex;
		flex-direction: column;
		padding: 16px;
		background: #fafafa;
		font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Arial, sans-serif;
	}

	.messages-container {
		flex: 1;
		overflow-y: auto;
		margin-bottom: 8px;
	}

	.message {
		padding: 8px 0;
		border-bottom: 1px solid #e3e6e8;
		display: flex;
		gap: 8px;
	}

	.message:last-child {
		border-bottom: none;
	}

	.message-avatar {
		width: 32px;
		height: 32px;
		border-radius: 3px;
		background: #0077cc;
		color: white;
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 500;
		font-size: 0.875rem;
		flex-shrink: 0;
	}

	.message-body {
		flex: 1;
		min-width: 0;
	}

	.message-header {
		display: flex;
		align-items: baseline;
		gap: 6px;
		margin-bottom: 4px;
	}

	.message-header strong {
		color: #0077cc;
		font-size: 0.8125rem;
		font-weight: 500;
	}

	.timestamp {
		color: #9199a1;
		font-size: 0.75rem;
	}

	.message-content {
		color: #232629;
		font-size: 0.8125rem;
		line-height: 1.5;
		word-wrap: break-word;
	}

	.notice-placeholder {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		color: #999;
		text-align: center;
		padding: 2rem;
	}

	.notice-placeholder .placeholder-icon {
		font-size: 3rem;
	}

	.notice-placeholder .placeholder-text {
		font-size: 0.875rem;
	}

	.message-input {
		display: flex;
		gap: 8px;
		padding-top: 12px;
		border-top: 1px solid #e3e6e8;
		margin-top: 8px;
	}

	.message-input input {
		flex: 1;
		padding: 8px;
		border: 1px solid #babfc4;
		border-radius: 3px;
		font-size: 0.8125rem;
		font-family: inherit;
	}

	.message-input input:focus {
		outline: none;
		border-color: #6cbbf7;
		box-shadow: 0 0 0 4px rgba(0, 119, 204, 0.1);
	}

	.message-input button {
		padding: 8px 12px;
		background: #0a95ff;
		color: white;
		border: none;
		border-radius: 3px;
		font-size: 0.8125rem;
		font-weight: normal;
		cursor: pointer;
		transition: background 0.1s;
	}

	.message-input button:hover {
		background: #0074cc;
	}

	.message-input button:active {
		background: #0063bf;
	}


	/* Make content scrollable area smaller to avoid resize handle conflict */
	.block.selected .block-heading,
	.block.selected .block-text {
		/* Add small padding when selected to make resize handles easier to grab */
		padding-right: 8px;
		padding-bottom: 8px;
	}

	.block-heading {
		font-weight: bold;
	}

	.block-image {
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
	}

	.block-image img {
		width: 100%;
		height: 100%;
		object-fit: contain;
	}

	.image-placeholder {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		color: #999;
		text-align: center;
		padding: 1rem;
	}

	.placeholder-icon {
		font-size: 3rem;
	}

	.placeholder-text {
		font-size: 0.875rem;
	}

	.html-placeholder {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		color: #999;
		text-align: center;
		padding: 1rem;
	}

	.block-container {
		display: flex;
		align-items: center;
		justify-content: center;
		border: 2px dashed #ccc;
		background: rgba(0, 0, 0, 0.02);
	}

	.container-label {
		color: #999;
		font-size: 0.875rem;
		pointer-events: none;
	}

	[contenteditable="true"] {
		cursor: text;
		user-select: text;
	}

	/* Visual hint: show subtle outline when hovering over block */
	.block:hover:not(.dragging):not(.resizing) {
		outline: 1px dashed rgba(102, 126, 234, 0.3);
		outline-offset: -1px;
	}

	/* Resize handles */
	.resize-handle {
		position: absolute;
		background: #667eea;
		border: 2px solid white;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
		opacity: 0;
		transition: opacity 0.2s, transform 0.2s;
		z-index: 10; /* Ensure handles are above content */
	}

	/* Make handles visible on hover or when selected */
	.block:hover .resize-handle,
	.block.selected .resize-handle {
		opacity: 1;
	}

	/* Enlarge handles on hover for easier grabbing */
	.resize-handle:hover {
		transform: scale(1.3);
		background: #5568d3;
		box-shadow: 0 3px 8px rgba(0, 0, 0, 0.3);
	}

	/* Visual hint for shift+click incremental resize */
	.resize-handle::after {
		content: '';
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 0;
		height: 0;
		border: 3px solid transparent;
		opacity: 0;
		transition: opacity 0.2s;
	}

	/* Show arrows when hovering with potential for shift-click */
	.resize-handle:hover::after {
		opacity: 0.7;
	}

	/* Corner handles - larger for easier grabbing */
	.resize-nw,
	.resize-ne,
	.resize-se,
	.resize-sw {
		width: 16px;
		height: 16px;
		border-radius: 50%;
	}

	/* Edge handles */
	.resize-n,
	.resize-s {
		width: 40px;
		height: 10px;
		left: 50%;
		transform: translateX(-50%);
		border-radius: 5px;
	}

	.resize-e,
	.resize-w {
		width: 10px;
		height: 40px;
		top: 50%;
		transform: translateY(-50%);
		border-radius: 5px;
	}

	/* Positioning - adjusted for larger handles */
	.resize-nw {
		top: -8px;
		left: -8px;
		cursor: nw-resize;
	}

	.resize-n {
		top: -5px;
		cursor: n-resize;
	}

	.resize-ne {
		top: -8px;
		right: -8px;
		cursor: ne-resize;
	}

	.resize-e {
		right: -5px;
		cursor: e-resize;
	}

	.resize-se {
		bottom: -8px;
		right: -8px;
		cursor: se-resize;
	}

	.resize-s {
		bottom: -5px;
		cursor: s-resize;
	}

	.resize-sw {
		bottom: -8px;
		left: -8px;
		cursor: sw-resize;
	}

	.resize-w {
		left: -5px;
		cursor: w-resize;
	}

	/* Form Block */
	.block-form {
		width: 100%;
		height: 100%;
		padding: 20px;
		background: white;
		overflow: auto;
		box-sizing: border-box;
	}

	.custom-form {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.form-field label {
		font-size: 0.875rem;
		font-weight: 500;
		color: #333;
	}

	.form-field .required {
		color: #dc3545;
		margin-left: 2px;
	}

	.form-field input[type="text"],
	.form-field input[type="email"],
	.form-field input[type="number"],
	.form-field textarea {
		padding: 10px 12px;
		border: 1px solid #ced4da;
		border-radius: 4px;
		font-size: 0.875rem;
		font-family: inherit;
		transition: border-color 0.2s;
	}

	.form-field input:focus,
	.form-field textarea:focus {
		outline: none;
		border-color: #667eea;
		box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
	}

	.form-field textarea {
		min-height: 100px;
		resize: vertical;
	}

	.checkbox-field {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.checkbox-field input[type="checkbox"] {
		width: 18px;
		height: 18px;
		cursor: pointer;
	}

	.form-submit-btn {
		padding: 12px 24px;
		background: #0a95ff;
		color: white;
		border: none;
		border-radius: 4px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		transition: background 0.2s;
		align-self: flex-start;
	}

	.form-submit-btn:hover {
		background: #0074cc;
	}

	.form-submit-btn:active {
		background: #0063bf;
	}

	.submit-message {
		margin-top: 12px;
		padding: 12px;
		border-radius: 4px;
		font-size: 0.875rem;
	}

	.submit-message.success {
		background: #d4edda;
		color: #155724;
		border: 1px solid #c3e6cb;
	}

	.submit-message.error {
		background: #f8d7da;
		color: #721c24;
		border: 1px solid #f5c6cb;
	}
</style>
