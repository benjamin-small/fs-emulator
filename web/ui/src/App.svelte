<script lang="ts">
  import { Volume } from "./lib/wasm";
  const vol = Volume.formatFat16(undefined);
  vol.createFile("/HELLO.TXT", new TextEncoder().encode("hello"));
  const entries = vol.listDir("/");
</script>
<main class="panel" style="margin: 16px">
  <h2>FAT explorer</h2>
  <p class="muted">{vol.fsType()} · {vol.sectorCount()} sectors</p>
  <ul class="mono">{#each entries as e}<li>{e.name} · {e.size} bytes</li>{/each}</ul>
</main>
