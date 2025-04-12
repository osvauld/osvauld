<script lang="ts">
	import BaseImportPvtKey from "@osvauld/password-manager-common/components/BaseImportPvtKey.svelte";

	let { onLogin } = $props();
</script>

<BaseImportPvtKey {onLogin} />
