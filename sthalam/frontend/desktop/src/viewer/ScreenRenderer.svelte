<script lang="ts">
  import { loroCoordinator } from '../shared/loro/loroCoordinator';

  // Get template tree
  const tree = loroCoordinator.getTemplateTree();

  // For now, just log what we have
  const roots = tree.roots();
  console.log('📱 [ScreenRenderer] Root screens:', roots);
</script>

<div class="screen-renderer">
  <!-- Will render screens here once we have block rendering -->
  <p>Screens will render here</p>
</div>

<style>
  .screen-renderer {
    width: 100%;
    height: 100%;
  }
</style>