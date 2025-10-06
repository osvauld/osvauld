
<script lang="ts">
	interface Props {
		size?: number;
	}

	let {  size = 24 }: Props = $props();
</script>
<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path fill-rule="evenodd" clip-rule="evenodd" d="M11.25 14.75V3.99999L9.40497 5.845L8.34497 4.78499L11.115 2.015C11.605 1.53 12.4 1.53 12.885 2.015L15.655 4.78499L14.595 5.845L12.75 3.99999V14.75H11.25ZM14 7H17.25C18.765 7 20 8.235 20 9.75V19.25C20 20.765 18.765 22 17.25 22H6.75C5.235 22 4 20.765 4 19.25V9.75C4 8.235 5.235 7 6.75 7H10V8.5H6.75C6.06 8.5 5.5 9.06 5.5 9.75V19.25C5.5 19.94 6.06 20.5 6.75 20.5H17.25C17.94 20.5 18.5 19.94 18.5 19.25V9.75C18.5 9.06 17.94 8.5 17.25 8.5H14V7Z" fill="currentColor"/>
    </svg>
