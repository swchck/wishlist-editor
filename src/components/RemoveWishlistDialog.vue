<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CircleAlertIcon, CopyIcon, ExternalLinkIcon, Loader2Icon } from "@lucide/vue";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { api, errorMessage, isAppError, type DevicePrompt, type RepoId } from "@/lib/api";
import { plural } from "@/lib/wishlist/changes";

export interface RemoveTarget extends RepoId {
  title: string | null;
  /** `null` for an entry known only from the local copies list (offline). */
  listId: string | null;
}

const props = defineProps<{ target: RemoveTarget | null; mode: "delete" | "forget" }>();
const emit = defineEmits<{ close: []; removed: [] }>();

type Phase = "confirm" | "code" | "working";
const phase = ref<Phase>("confirm");
const typed = ref("");
const unpublished = ref(0);
const prompt = ref<DevicePrompt | null>(null);
const error = ref<string | null>(null);

const open = computed({
  get: () => !!props.target,
  set: (v) => {
    if (!v && phase.value !== "working") cancel();
  },
});

watch(
  () => props.target,
  async (t) => {
    phase.value = "confirm";
    typed.value = "";
    error.value = null;
    prompt.value = null;
    unpublished.value = 0;
    if (t) unpublished.value = (await api.localStatus(t.owner, t.name).catch(() => null))?.unpublished ?? 0;
  },
);

const fullName = computed(() => (props.target ? `${props.target.owner}/${props.target.name}` : ""));
const cleanupSql = computed(() => {
  const list = (props.target?.listId ?? fullName.value).replace(/'/g, "''");
  return `delete from public.reservation_secrets where list = '${list}';\ndelete from public.reservations where list = '${list}';`;
});

function cancel() {
  if (phase.value === "code") void api.deleteCancel();
  emit("close");
}

async function forget() {
  if (!props.target) return;
  phase.value = "working";
  try {
    await api.forgetLocal(props.target.owner, props.target.name);
    toast.success("Копия на этом компьютере удалена");
    emit("removed");
  } catch (e) {
    error.value = errorMessage(e);
    phase.value = "confirm";
  }
}

async function remove() {
  if (!props.target || typed.value !== props.target.name) return;
  error.value = null;
  try {
    prompt.value = await api.deleteStart(props.target.name);
    phase.value = "code";
    void openUrl(prompt.value.verificationUri);
    await api.deleteWait();
    phase.value = "working";
    toast.success(`Вишлист ${fullName.value} удалён`);
    emit("removed");
  } catch (e) {
    if (isAppError(e, "auth_cancelled")) return;
    error.value = errorMessage(e);
    phase.value = "confirm";
  }
}

async function copySql() {
  await navigator.clipboard.writeText(cleanupSql.value).catch(() => {});
  toast.success("SQL скопирован");
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>
          {{ mode === "delete" ? `Удалить ${fullName}?` : "Убрать с этого компьютера?" }}
        </DialogTitle>
        <DialogDescription v-if="mode === 'forget'">
          Удалится только копия в приложении. На GitHub и на сайте ничего не изменится — открыть вишлист можно снова в
          любой момент.
        </DialogDescription>
        <DialogDescription v-else>
          Удалятся репозиторий на GitHub со всей историей, сайт и копия на этом компьютере. Это не отменить.
        </DialogDescription>
      </DialogHeader>

      <div
        v-if="unpublished"
        class="border-destructive/40 bg-destructive/5 text-destructive flex gap-2 rounded-lg border p-3 text-sm"
      >
        <CircleAlertIcon class="mt-0.5 size-4 shrink-0" />
        Здесь {{ unpublished }}
        {{
          plural(unpublished, "неопубликованное изменение", "неопубликованных изменения", "неопубликованных изменений")
        }}
        — они пропадут.
      </div>

      <template v-if="mode === 'delete' && phase === 'confirm'">
        <div class="grid gap-1.5">
          <Label for="confirm-name"
            >Введите <b class="font-mono">{{ target?.name }}</b
            >, чтобы подтвердить</Label
          >
          <Input id="confirm-name" v-model="typed" class="font-mono" autocomplete="off" />
        </div>
        <p class="text-muted-foreground text-xs">
          Для удаления GitHub попросит отдельное разрешение. Приложение использует его один раз и не сохраняет.
        </p>
        <details class="text-muted-foreground text-xs">
          <summary class="cursor-pointer">Брони этого вишлиста в Supabase останутся</summary>
          <p class="mt-2">Ключом сайта их не стереть. Если нужно, выполните в SQL Editor базы:</p>
          <pre class="bg-muted mt-1.5 overflow-auto rounded-md p-2 font-mono text-[11px] whitespace-pre-wrap">{{
            cleanupSql
          }}</pre>
          <Button variant="ghost" size="sm" class="mt-1 px-1" @click="copySql"><CopyIcon /> Скопировать</Button>
        </details>
      </template>

      <template v-if="phase === 'code' && prompt">
        <p class="text-sm">Подтвердите удаление на github.com/login/device этим кодом:</p>
        <div class="flex items-center gap-1.5" aria-label="Код подтверждения">
          <span
            v-for="(ch, i) in prompt.userCode"
            :key="i"
            :class="
              ch === '-'
                ? 'text-muted-foreground w-3 text-center'
                : 'bg-card grid h-11 w-9 place-items-center rounded-lg border font-mono text-lg font-semibold'
            "
            >{{ ch }}</span
          >
        </div>
        <div class="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2Icon class="size-4 animate-spin" /> Ждём подтверждения…
          <Button variant="ghost" size="sm" @click="openUrl(prompt.verificationUri)">
            <ExternalLinkIcon /> Открыть страницу снова
          </Button>
        </div>
      </template>

      <p v-if="error" class="text-destructive flex gap-2 text-sm">
        <CircleAlertIcon class="mt-0.5 size-4 shrink-0" /> {{ error }}
      </p>

      <DialogFooter>
        <Button variant="ghost" :disabled="phase === 'working'" @click="cancel">Отмена</Button>
        <Button v-if="mode === 'forget'" variant="destructive" :disabled="phase === 'working'" @click="forget">
          Убрать
        </Button>
        <Button v-else variant="destructive" :disabled="phase !== 'confirm' || typed !== target?.name" @click="remove">
          <Loader2Icon v-if="phase === 'working'" class="animate-spin" /> Удалить навсегда
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
