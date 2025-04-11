<script lang="ts">
	import BaseImportPvtKey from "@osvauld/password-manager-common/components/BaseImportPvtKey.svelte";

	// Callback function for login event
	const handleLogin = (loginSuccess) => {
		// Dispatch the event to parent components
		// This replaces the event bubbling behavior in Svelte 4
		dispatchEvent(
			new CustomEvent("login", {
				detail: loginSuccess,
				bubbles: true,
			}),
		);
	};
</script>

<BaseImportPvtKey onLogin={handleLogin} />
