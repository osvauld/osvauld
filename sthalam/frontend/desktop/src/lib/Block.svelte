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
		readonly?: boolean;
		noPositioning?: boolean; // Don't apply position styles (for viewer mode)
		onUpdate: (blockId: string, updates: any) => void;
		onSelect: (blockId: string) => void;
		onFormSubmit?: (submitButtonId: string) => void;
		onNavigate?: (navButtonId: string) => void;
	}

	let { block, isSelected, readonly = false, noPositioning = false, onUpdate, onSelect, onFormSubmit, onNavigate }: Props = $props();

	let isDragging = $state(false);
	let isResizing = $state(false);
	let resizeHandle = $state<string | null>(null);
	let dragStart = $state({ x: 0, y: 0 });
	let blockStart = $state({ x: 0, y: 0, width: 0, height: 0 });
	let dragModeEnabled = $state(false); // Ctrl+click to enable
	let hoverEdge = $state<string | null>(null); // Track which edge is hovered

	// References to contenteditable elements
	let headingRef: HTMLDivElement | null = null;
	let textRef: HTMLDivElement | null = null;

	// Form field state (for viewer mode)
	let formFieldValue = $state<any>(block.type === 'form-field-checkbox' ? false : '');

	// Branching question answer state (local only)
	let selectedAnswer = $state<string | null>(null);

	// Handle contenteditable input
	function handleContentInput(e: Event) {
		const target = e.currentTarget as HTMLElement;
		const newContent = target.textContent || "";

		// Simply update Yjs - don't touch the DOM
		onUpdate(block.id, { content: newContent });
	}

	// Sync content from props to DOM only when element is not focused
	$effect(() => {
		// Track block.id to reset content when switching blocks
		const currentBlockId = block.id;
		const currentContent = block.content;

		// Update heading content only if not currently being edited
		if (headingRef) {
			if (document.activeElement !== headingRef && headingRef.textContent !== currentContent) {
				headingRef.textContent = currentContent;
			}
		}

		// Update text content only if not currently being edited
		if (textRef) {
			if (document.activeElement !== textRef && textRef.textContent !== currentContent) {
				textRef.textContent = currentContent;
			}
		}
	});

	function handleMouseDown(e: MouseEvent) {
		// In readonly mode, don't allow any editing interactions
		if (readonly) {
			return;
		}

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
		// In readonly mode, always use default cursor
		if (readonly) return "default";

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
	class:no-positioning={noPositioning}
	style:left={noPositioning ? undefined : `${block.x}px`}
	style:top={noPositioning ? undefined : `${block.y}px`}
	style:width={noPositioning ? "100%" : `${block.width}px`}
	style:height={noPositioning ? "100%" : `${block.height}px`}
	style:z-index={noPositioning ? undefined : block.zIndex}
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
		<div
			bind:this={headingRef}
			class="block-heading"
			contenteditable={!readonly}
			oninput={handleContentInput}
		></div>
	{:else if block.type === "text"}
		<div
			bind:this={textRef}
			class="block-text"
			contenteditable={!readonly}
			oninput={handleContentInput}
		></div>
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
			<!-- Empty container for layout -->
		</div>
	{:else if block.type === "html"}
		<div class="block-html">
			{#if block.content && block.content !== ""}
				{@html `${block.styles['css'] ? `<style>${block.styles['css']}</style>` : ''}${sanitizeHTML(block.content)}`}
			{:else}
				<div class="html-placeholder">
					<span class="placeholder-icon">&lt;/&gt;</span>
					<span class="placeholder-text">Add HTML in Properties</span>
				</div>
			{/if}
		</div>
	{:else if block.type === "form"}
		<div class="block-form-metadata" class:viewer-mode={readonly}>
			<div class="form-metadata-header">
				<span class="form-metadata-icon">📋</span>
				<span class="form-metadata-name">{block.name || "Unnamed Form"}</span>
			</div>
			{#if block.eventName}
				<div class="form-metadata-event">🏷️ {block.eventName}</div>
			{/if}
			{#if block.description}
				<div class="form-metadata-description">{block.description}</div>
			{/if}
		</div>
	{:else if block.type === "form-container"}
		<div class="block-form-container">
			<div class="form-container-label">📋 {block.label || "Form Container"}</div>
			<div class="form-container-hint">Drop form fields inside</div>
		</div>
	{:else if block.type === "form-field-text"}
		<div class="block-form-field">
			<label class="form-field-label">{block.label || "Text Field"}{block.required ? ' *' : ''}</label>
			<input
				type="text"
				placeholder={block.placeholder || "Enter text..."}
				bind:value={formFieldValue}
				disabled={!readonly}
				required={block.required}
				data-field-id={block.id}
			/>
		</div>
	{:else if block.type === "form-field-email"}
		<div class="block-form-field">
			<label class="form-field-label">{block.label || "Email"}{block.required ? ' *' : ''}</label>
			<input
				type="email"
				placeholder={block.placeholder || "Enter email..."}
				bind:value={formFieldValue}
				disabled={!readonly}
				required={block.required}
				data-field-id={block.id}
			/>
		</div>
	{:else if block.type === "form-field-number"}
		<div class="block-form-field">
			<label class="form-field-label">{block.label || "Number"}{block.required ? ' *' : ''}</label>
			<input
				type="number"
				placeholder={block.placeholder || "Enter number..."}
				bind:value={formFieldValue}
				disabled={!readonly}
				required={block.required}
				data-field-id={block.id}
			/>
		</div>
	{:else if block.type === "form-field-textarea"}
		<div class="block-form-field">
			<label class="form-field-label">{block.label || "Message"}{block.required ? ' *' : ''}</label>
			<textarea
				placeholder={block.placeholder || "Enter message..."}
				bind:value={formFieldValue}
				disabled={!readonly}
				required={block.required}
				rows="3"
				data-field-id={block.id}
			></textarea>
		</div>
	{:else if block.type === "form-field-checkbox"}
		<div class="block-form-field checkbox-field">
			<label class="form-field-label checkbox-label">
				<input
					type="checkbox"
					bind:checked={formFieldValue}
					disabled={!readonly}
					data-field-id={block.id}
				/>
				<span>{block.label || "Checkbox"}{block.required ? ' *' : ''}</span>
			</label>
		</div>
	{:else if block.type === "form-submit-button"}
		<div class="block-form-submit">
			<button
				type="button"
				disabled={!readonly}
				onclick={() => readonly && onFormSubmit && onFormSubmit(block.id)}
			>
				{block.content || "Submit"}
			</button>
		</div>
	{:else if block.type === "branching-question"}
		<div class="block-branching-question">
			<div class="question-text">{block.question || "Your question here?"}</div>
			<div class="question-options">
				<button
					class="option-button"
					class:selected={selectedAnswer === 'yes'}
					disabled={!readonly}
					onclick={() => readonly && (selectedAnswer = 'yes')}
					data-answer="yes"
					data-question-id={block.id}
				>
					{block.yesLabel || "Yes"}
				</button>
				<button
					class="option-button"
					class:selected={selectedAnswer === 'no'}
					disabled={!readonly}
					onclick={() => readonly && (selectedAnswer = 'no')}
					data-answer="no"
					data-question-id={block.id}
				>
					{block.noLabel || "No"}
				</button>
			</div>
		</div>
	{:else if block.type === "nav-button"}
		<div class="block-nav-button">
			<button
				type="button"
				disabled={!readonly}
				data-nav-button={block.id}
				onclick={() => readonly && onNavigate && onNavigate(block.id)}
			>
				{block.content || "Next"}
			</button>
		</div>
	{/if}

	<!-- Resize handles (hidden in readonly mode) -->
	{#if !readonly}
		<div class="resize-handle resize-nw" onmousedown={(e) => handleResizeStart(e, "nw")}></div>
		<div class="resize-handle resize-n" onmousedown={(e) => handleResizeStart(e, "n")}></div>
		<div class="resize-handle resize-ne" onmousedown={(e) => handleResizeStart(e, "ne")}></div>
		<div class="resize-handle resize-e" onmousedown={(e) => handleResizeStart(e, "e")}></div>
		<div class="resize-handle resize-se" onmousedown={(e) => handleResizeStart(e, "se")}></div>
		<div class="resize-handle resize-s" onmousedown={(e) => handleResizeStart(e, "s")}></div>
		<div class="resize-handle resize-sw" onmousedown={(e) => handleResizeStart(e, "sw")}></div>
		<div class="resize-handle resize-w" onmousedown={(e) => handleResizeStart(e, "w")}></div>
	{/if}
</div>

<style>
	.block {
		position: absolute;
		transition: box-shadow 0.2s, outline 0.2s;
	}

	.block.no-positioning {
		position: relative;
		width: 100%;
		height: 100%;
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
	.block-html {
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

	/* Form block styles */
	.block-form-metadata {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 12px;
		border: 2px solid #667eea;
		background: rgba(102, 126, 234, 0.1);
		border-radius: 8px;
	}

	.block-form-metadata.viewer-mode {
		display: none; /* Invisible in viewer mode */
	}

	.form-metadata-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.form-metadata-icon {
		font-size: 1.25rem;
	}

	.form-metadata-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: #667eea;
	}

	.form-metadata-event {
		font-size: 0.7rem;
		font-family: monospace;
		color: #48bb78;
		background: rgba(72, 187, 120, 0.1);
		padding: 0.25rem 0.5rem;
		border-radius: 4px;
		font-weight: 600;
	}

	.form-metadata-description {
		font-size: 0.75rem;
		color: #666;
		font-style: italic;
	}

	.block-form-container {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		border: 2px dashed #667eea;
		background: rgba(102, 126, 234, 0.05);
	}

	.form-container-label {
		font-size: 1rem;
		font-weight: 600;
		color: #667eea;
	}

	.form-container-hint {
		font-size: 0.875rem;
		color: #999;
	}

	.block-form-field {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 12px;
	}

	.form-field-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: #333;
	}

	.block-form-field input,
	.block-form-field textarea {
		width: 100%;
		padding: 0.5rem;
		border: 1px solid #ddd;
		border-radius: 4px;
		font-size: 0.875rem;
		background: #f9f9f9;
	}

	.block-form-field textarea {
		resize: vertical;
		min-height: 60px;
	}

	.checkbox-field {
		flex-direction: row;
		align-items: center;
	}

	.checkbox-label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		cursor: pointer;
	}

	.checkbox-label input[type="checkbox"] {
		width: auto;
		cursor: pointer;
	}

	.block-form-submit {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.block-form-submit button {
		padding: 0.75rem 1.5rem;
		border: none;
		border-radius: 4px;
		font-size: 0.875rem;
		font-weight: 600;
		cursor: pointer;
		background: #667eea;
		color: white;
	}

	.block-form-submit button:disabled {
		opacity: 0.7;
		cursor: not-allowed;
		pointer-events: none; /* Allow clicks to pass through to block for dragging */
	}

	/* Branching question styles */
	.block-branching-question {
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 1.5rem;
		justify-content: center;
	}

	.question-text {
		font-size: 1.125rem;
		font-weight: 600;
		color: #333;
		text-align: center;
	}

	.question-options {
		display: flex;
		gap: 1rem;
		justify-content: center;
	}

	.option-button {
		padding: 0.75rem 2rem;
		border: 2px solid #667eea;
		border-radius: 8px;
		font-size: 1rem;
		font-weight: 500;
		background: white;
		color: #667eea;
		cursor: pointer;
		transition: all 0.2s;
	}

	.option-button:hover {
		background: #f0f2ff;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(102, 126, 234, 0.2);
	}

	.option-button.selected {
		background: #667eea;
		color: white;
		box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
	}

	.option-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
		pointer-events: none;
	}

	/* Navigation button styles */
	.block-nav-button {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.block-nav-button button {
		padding: 0.75rem 2rem;
		border: none;
		border-radius: 8px;
		font-size: 1rem;
		font-weight: 600;
		cursor: pointer;
		background: #48bb78;
		color: white;
		transition: all 0.2s;
	}

	.block-nav-button button:hover {
		background: #38a169;
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(72, 187, 120, 0.3);
	}

	.block-nav-button button:disabled {
		cursor: not-allowed;
		pointer-events: none;
	}
</style>
