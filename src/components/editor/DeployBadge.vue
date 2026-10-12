<script setup lang="ts">
import { computed } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CircleAlertIcon, CircleCheckIcon, Loader2Icon } from "@lucide/vue";
import type { Deploy } from "@/stores/publish";

const props = defineProps<{ deploy: Deploy }>();

const state = computed(() => {
  const run = props.deploy.run;
  if (run?.status === "completed") return run.conclusion === "success" ? "ok" : "failed";
  return props.deploy.lost ? "lost" : "building";
});
const label = computed(
  () =>
    ({
      building: "Сайт обновляется…",
      ok: "Сайт обновлён",
      failed: "Сборка сайта упала",
      lost: "Статус сборки неизвестен",
    })[state.value],
);
</script>

<template>
  <button
    class="flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs"
    :class="{
      'bg-muted text-muted-foreground': state === 'building' || state === 'lost',
      'bg-emerald-500/10 text-emerald-700 dark:text-emerald-400': state === 'ok',
      'bg-destructive/10 text-destructive': state === 'failed',
    }"
    :disabled="!deploy.run"
    :title="deploy.run ? 'Открыть сборку на GitHub' : ''"
    @click="deploy.run && openUrl(deploy.run.htmlUrl)"
  >
    <Loader2Icon v-if="state === 'building'" class="size-3.5 animate-spin" />
    <CircleCheckIcon v-else-if="state === 'ok'" class="size-3.5" />
    <CircleAlertIcon v-else class="size-3.5" />
    {{ label }}
  </button>
</template>
