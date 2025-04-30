<script lang="ts">
    const { color = "#F2F2F0", size = 24 } = $props();
</script>

<svg 
    width={size} 
    height={size} 
    viewBox="0 0 24 24" 
    fill="none" 
    xmlns="http://www.w3.org/2000/svg"
>
    <path 
        d="M3 6H21M3 12H21M3 18H21" 
        stroke={color} 
        stroke-width="2" 
        stroke-linecap="round" 
        stroke-linejoin="round"
    />
</svg> 