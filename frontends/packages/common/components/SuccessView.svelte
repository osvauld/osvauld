<script>
	export let message = "";
	export let status = true;
	let changePassword = false;
</script>

<style>
	.content {
		margin: 40px auto 0;
		display: block;
		width: 100px;
	}
	.circle {
		stroke-dasharray: 1194;
		stroke-dashoffset: 1194;
		animation: dc 0.5s ease-in-out;
		animation-fill-mode: forwards;
	}
	.tick {
		stroke-dasharray: 350;
		stroke-dashoffset: 350;
		animation: dt 0.4s ease-out;
		animation-fill-mode: forwards;
		animation-delay: 0.95s;
	}
	.x-line {
		stroke-dasharray: 350;
		stroke-dashoffset: 350;
		animation: dt 0.4s ease-out;
		animation-fill-mode: forwards;
		animation-delay: 0.3s;
	}
	p {
		text-align: center;
		margin: 20px 0 60px;
		font-size: 1.25em;
	}
	p.success {
		color: #52e376c2;
	}
	p.error {
		color: #d06079;
	}
	@keyframes dc {
		from {
			stroke-dashoffset: 1194;
		}
		to {
			stroke-dashoffset: 2388;
		}
	}
	@keyframes dt {
		from {
			stroke-dashoffset: 350;
		}
		to {
			stroke-dashoffset: 0;
		}
	}
	/* Add WebKit prefixes for better cross-browser compatibility */
	@-webkit-keyframes dc {
		from {
			stroke-dashoffset: 1194;
		}
		to {
			stroke-dashoffset: 2388;
		}
	}
	@-webkit-keyframes dt {
		from {
			stroke-dashoffset: 350;
		}
		to {
			stroke-dashoffset: 0;
		}
	}
</style>

{#if status}
	<div class="content">
		<svg width="100" height="100" viewBox="0 0 400 400">
			<circle
				fill="none"
				stroke="#52e376c2"
				stroke-width="20"
				cx="200"
				cy="200"
				r="190"
				stroke-linecap="round"
				transform="rotate(-90 200 200)"
				class="circle"></circle>
			<polyline
				fill="none"
				stroke="#52e376c2"
				points="88,214 173,284 304,138"
				stroke-width="24"
				stroke-linecap="round"
				stroke-linejoin="round"
				class="tick"></polyline>
		</svg>
		<p class="success">
			{changePassword
				? "Passphrase Changed"
				: message
					? message
					: "Copied to Clipboard"}
		</p>
	</div>
{:else}
	<div class="content">
		<svg width="100" height="100" viewBox="0 0 400 400">
			<circle
				fill="none"
				stroke="#D06079"
				stroke-width="20"
				cx="200"
				cy="200"
				r="190"
				stroke-linecap="round"
				transform="rotate(-90 200 200)"
				class="circle"></circle>
			<line
				fill="none"
				stroke="#D06079"
				stroke-width="24"
				stroke-linecap="round"
				x1="100"
				y1="100"
				x2="300"
				y2="300"
				class="x-line"></line>
			<line
				fill="none"
				stroke="#D06079"
				stroke-width="24"
				stroke-linecap="round"
				x1="300"
				y1="100"
				x2="100"
				y2="300"
				class="x-line"></line>
		</svg>
		<p class="error">
			{changePassword
				? "Wrong Passphrase"
				: message
					? message
					: "Import Incomplete!"}
		</p>
	</div>
{/if}
