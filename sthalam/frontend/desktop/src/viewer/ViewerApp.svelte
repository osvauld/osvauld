<script lang="ts">
  import { loroCoordinator } from '../shared/loro/loroCoordinator';
  import ScreenRenderer from './ScreenRenderer.svelte';

  // For now, just a placeholder
  console.log('👁️ [ViewerApp] Starting viewer mode');
</script>

<div class="viewer-app">
  <h1>Viewer Mode</h1>
  <p>Template will render here</p>

  <!-- Will add ScreenRenderer when we have screens -->
</div>

<style>
  .viewer-app {
    padding: 20px;
  }
</style>