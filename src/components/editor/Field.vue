<script setup lang="ts">
import { Label } from "@/components/ui/label";

defineProps<{ label: string; for?: string; error?: string }>();
</script>

<template>
  <div class="grid content-start gap-1.5">
    <Label :for="$props.for" class="text-muted-foreground text-xs">{{ label }}</Label>
    <slot />
    <p v-if="error" class="text-destructive text-xs">{{ error }}</p>
  </div>
</template>
