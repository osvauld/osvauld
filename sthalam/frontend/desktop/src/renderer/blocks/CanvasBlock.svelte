<script lang="ts">
import type { CanvasBlock, Context, ActionHandler, StateChangeHandler } from '../../lib/types/huml';

interface Props {
  block: CanvasBlock;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

let { block, context }: Props = $props();
</script>

<div style="border: 2px dashed orange; padding: 1rem; background: #fff3cd;">
  <strong>TODO: CanvasBlock</strong>
  <pre>{JSON.stringify(block, null, 2)}</pre>
</div>
