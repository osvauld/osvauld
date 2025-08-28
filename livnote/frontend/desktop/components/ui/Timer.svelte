<script lang="ts">
	import { onDestroy } from "svelte";
	const TIMER = 15 * 60 * 1000; // 15 minutes in ms
	let remainingTime = $state(TIMER);
	let interval: ReturnType<typeof setInterval> | null = null;

	// Format mm:ss
	const formatTime = (ms: number): string => {
		const totalSeconds = Math.floor(ms / 1000);
		const minutes = Math.floor(totalSeconds / 60);
		const seconds = totalSeconds % 60;
		return `${minutes.toString().padStart(2, "0")}:${seconds
			.toString()
			.padStart(2, "0")}`;
	};

	const timerControlLogic = () => {
		if (interval) {
			clearInterval(interval);
			interval = null;
			return;
		}

		interval = setInterval(() => {
			if (remainingTime > 0) {
				remainingTime -= 1000;
			} else {
				clearInterval(interval!);
				interval = null;
			}
		}, 1000);
	};

	const resetTimer = () => {
		clearInterval(interval!);
		interval = null;
		remainingTime = TIMER;
	};

	onDestroy(() => {
		if (interval) {
			clearInterval(interval);
		}
	});
</script>

<button
	class="text-osvauld-fieldText text-base cursor-pointer hover:text-white active:text-osvauld-fieldText"
	aria-label="Start, pause or reset timer"
	aria-pressed={interval ? "true" : "false"}
	onclick={timerControlLogic}
	ondblclick={resetTimer}
>
	<span role="timer" aria-live="polite">
		{formatTime(remainingTime)}
	</span>
</button>

<p id="timer-instructions" class="sr-only">
	Click to start or stop the timer. Double click to reset.
</p>
