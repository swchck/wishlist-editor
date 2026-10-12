<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowLeftIcon, CheckIcon, CircleAlertIcon, ExternalLinkIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, errorMessage, type NameCheck, type ReservationsCheck } from "@/lib/api";
import { COMMON_CURRENCIES } from "@/lib/wishlist/schema";
import { useCreate, type StepId } from "@/stores/create";
import { useSession } from "@/stores/session";
import { useWishlist } from "@/stores/wishlist";

const emit = defineEmits<{ back: [] }>();
const create = useCreate();
const session = useSession();
const wishlist = useWishlist();

type Page = "details" | "reservations" | "progress";
const page = ref<Page>(create.running || create.steps.repo !== "waiting" ? "progress" : "details");

// ---- name -------------------------------------------------------------------------------
const NAME_RE = /^[A-Za-z0-9._-]{1,100}$/;
const nameState = ref<NameCheck | "checking" | "invalid" | null>(null);
let nameTimer: ReturnType<typeof setTimeout> | undefined;
watch(
  () => create.name,
  (n) => {
    clearTimeout(nameTimer);
    if (!NAME_RE.test(n) || n === "." || n === "..") {
      nameState.value = "invalid";
      return;
    }
    nameState.value = "checking";
    nameTimer = setTimeout(async () => {
      try {
        const res = await api.checkName(n);
        if (create.name === n) nameState.value = res;
      } catch (e) {
        nameState.value = null;
        create.error = errorMessage(e);
      }
    }, 400);
  },
  { immediate: true },
);
const siteHint = computed(() => {
  const owner = session.user?.login.toLowerCase() ?? "you";
  return create.name.toLowerCase() === `${owner}.github.io`
    ? `https://${owner}.github.io`
    : `https://${owner}.github.io/${create.name}`;
});
const detailsOk = computed(
  () =>
    (nameState.value === "free" || nameState.value === "unfinished") &&
    create.title.trim().length > 0 &&
    /^[A-Z]{3}$/.test(create.currency),
);

// ---- reservations -----------------------------------------------------------------------
const useReservations = ref(!!create.reservations);
const supaUrl = ref(create.reservations?.url ?? "");
const supaKey = ref(create.reservations?.anonKey ?? "");
const probe = ref<ReservationsCheck | "checking" | null>(null);
const probeError = ref<string | null>(null);
watch([supaUrl, supaKey], () => (probe.value = null));

const PROBE_TEXT: Record<ReservationsCheck, string> = {
  ready: "База готова — брони будут работать.",
  oldSchema:
    "База создана по старой схеме. Владельцу базы нужно выполнить supabase/migrations/002-shared-lists.sql, затем schema.sql.",
  noSchema: "В этой базе ещё нет таблиц броней. Выполните supabase/schema.sql в SQL Editor.",
  badKey: "Supabase не принял ключ. Нужен publishable (anon) key из Project Settings → API.",
  secretKey:
    "Это секретный ключ — он открывает доступ ко всей базе и попал бы в публичный репозиторий. Нужен publishable (anon) key.",
  unreachable: "Не удалось связаться с Supabase. Проверьте адрес и сеть.",
};

async function checkReservations() {
  probe.value = "checking";
  probeError.value = null;
  try {
    probe.value = await api.checkReservations(supaUrl.value, supaKey.value);
  } catch (e) {
    probe.value = null;
    probeError.value = errorMessage(e);
  }
}
const reservationsOk = computed(() => !useReservations.value || probe.value === "ready");

function begin() {
  create.reservations = useReservations.value ? { url: supaUrl.value.trim(), anonKey: supaKey.value.trim() } : null;
  create.reset();
  page.value = "progress";
  void create.start();
}

// ---- progress ---------------------------------------------------------------------------
const STEPS: { id: StepId; label: string }[] = [
  { id: "repo", label: "Копия шаблона в вашем GitHub" },
  { id: "pages", label: "Публикация через GitHub Pages" },
  { id: "setup", label: "Настройки вишлиста" },
  { id: "deploy", label: "Первая сборка сайта" },
];

async function openEditor() {
  const login = session.user?.login;
  if (!login) return;
  try {
    await wishlist.open({ owner: login, name: create.name });
    create.reset();
  } catch (e) {
    create.error = errorMessage(e);
  }
}

const nav = computed(() => [
  { id: "details", label: "Название", done: page.value !== "details" },
  { id: "reservations", label: "Брони", done: page.value === "progress" },
  { id: "progress", label: "Создание сайта", done: create.finished },
]);
</script>

<template>
  <div class="grid h-full grid-cols-[240px_minmax(0,1fr)]">
    <aside data-tauri-drag-region class="bg-muted/40 flex flex-col gap-1 border-r px-4 pt-14 pb-6">
      <Button variant="ghost" size="sm" class="mb-4 w-fit" :disabled="create.running" @click="emit('back')">
        <ArrowLeftIcon /> К списку
      </Button>
      <div
        v-for="(n, i) in nav"
        :key="n.id"
        class="flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm"
        :class="page === n.id ? 'bg-background border font-medium' : 'text-muted-foreground'"
      >
        <span
          class="grid size-5 place-items-center rounded-full border text-[11px] font-semibold"
          :class="{ 'bg-foreground text-background border-foreground': n.done }"
        >
          <CheckIcon v-if="n.done" class="size-3" />
          <template v-else>{{ i + 1 }}</template>
        </span>
        {{ n.label }}
      </div>
    </aside>

    <main class="overflow-auto">
      <div class="grid max-w-xl gap-6 px-10 pt-14 pb-10">
        <!-- 1. details -->
        <template v-if="page === 'details'">
          <div class="grid gap-1">
            <h1 class="text-2xl font-bold tracking-tight">Новый вишлист</h1>
            <p class="text-muted-foreground text-sm">Создадим копию шаблона в вашем GitHub и опубликуем сайт.</p>
          </div>
          <div class="grid gap-1.5">
            <Label for="new-name">Имя репозитория</Label>
            <Input id="new-name" v-model="create.name" class="font-mono" autocomplete="off" />
            <p
              class="text-xs"
              :class="nameState === 'free' || nameState === 'unfinished' ? 'text-muted-foreground' : 'text-destructive'"
            >
              <template v-if="nameState === 'checking'">Проверяю…</template>
              <template v-else-if="nameState === 'free'">Сайт будет на {{ siteHint }}</template>
              <template v-else-if="nameState === 'invalid'">Латиница, цифры, точка, дефис и подчёркивание</template>
              <template v-else-if="nameState === 'unfinished'">
                Создание этого вишлиста не закончено — продолжим с того же места
              </template>
              <template v-else-if="nameState === 'taken'">У вас уже есть репозиторий с таким именем</template>
              <template v-else-if="nameState === 'wishlist'">
                Такой вишлист уже есть —
                <button class="underline underline-offset-2" @click="openEditor">открыть его</button>
              </template>
            </p>
          </div>
          <div class="grid grid-cols-[1fr_120px] gap-3">
            <div class="grid gap-1.5">
              <Label for="new-title">Заголовок сайта</Label>
              <Input id="new-title" v-model="create.title" />
            </div>
            <div class="grid gap-1.5">
              <Label>Валюта</Label>
              <Select v-model="create.currency">
                <SelectTrigger class="w-full"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="c in COMMON_CURRENCIES" :key="c" :value="c">{{ c }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <label class="flex items-center gap-2.5 text-sm">
            <Switch id="keep-examples" v-model="create.keepExamples" />
            Оставить две карточки-примера
          </label>
          <div class="flex justify-end">
            <Button :disabled="!detailsOk" @click="page = 'reservations'">Дальше</Button>
          </div>
        </template>

        <!-- 2. reservations -->
        <template v-else-if="page === 'reservations'">
          <div class="grid gap-1">
            <h1 class="text-2xl font-bold tracking-tight">Брони</h1>
            <p class="text-muted-foreground text-sm">
              Гости смогут отмечать подарок занятым, чтобы не подарить одно и то же дважды. Нужна база Supabase — своя
              или общая: одна база обслуживает сколько угодно вишлистов.
            </p>
          </div>
          <label class="flex items-center gap-2.5 text-sm">
            <Switch id="use-reservations" v-model="useReservations" />
            Подключить брони
          </label>
          <template v-if="useReservations">
            <div class="grid gap-1.5">
              <Label for="supa-url">Project URL</Label>
              <Input id="supa-url" v-model="supaUrl" class="font-mono text-xs" placeholder="https://xxxx.supabase.co" />
            </div>
            <div class="grid gap-1.5">
              <Label for="supa-key">Publishable (anon) key</Label>
              <Input id="supa-key" v-model="supaKey" class="font-mono text-xs" placeholder="sb_publishable_…" />
              <p class="text-muted-foreground text-xs">
                Ключ публичный — он и так виден в коде сайта. Если базой уже пользуется другой вишлист, возьмите его URL
                и ключ.
              </p>
            </div>
            <div class="flex items-center gap-3">
              <Button
                variant="outline"
                :disabled="!supaUrl.trim() || !supaKey.trim() || probe === 'checking'"
                @click="checkReservations"
              >
                <Loader2Icon v-if="probe === 'checking'" class="animate-spin" /> Проверить базу
              </Button>
            </div>
            <p
              v-if="(probe && probe !== 'checking') || probeError"
              class="flex gap-2 text-sm"
              :class="probe === 'ready' ? 'text-emerald-700 dark:text-emerald-400' : 'text-destructive'"
            >
              <CheckIcon v-if="probe === 'ready'" class="mt-0.5 size-4 shrink-0" />
              <CircleAlertIcon v-else class="mt-0.5 size-4 shrink-0" />
              {{ probeError ?? PROBE_TEXT[probe as ReservationsCheck] }}
            </p>
          </template>
          <p v-else class="text-muted-foreground text-sm">
            Можно включить позже: впишите URL и ключ в wishlist.config.json или заведите секреты репозитория.
          </p>
          <div class="flex justify-between">
            <Button variant="ghost" @click="page = 'details'">Назад</Button>
            <Button :disabled="!reservationsOk" @click="begin">Создать вишлист</Button>
          </div>
        </template>

        <!-- 3. progress -->
        <template v-else>
          <div class="grid gap-1">
            <h1 class="text-2xl font-bold tracking-tight">
              {{ create.finished ? "Готово" : `Создаём ${session.user?.login}/${create.name}` }}
            </h1>
            <p class="text-muted-foreground text-sm">
              {{
                create.finished
                  ? "Сайт опубликован. Добавляйте подарки — после «Опубликовать» они появятся на сайте."
                  : "Около двух минут. Если закрыть окно, создание можно продолжить: «Создать новый вишлист» с тем же именем."
              }}
            </p>
          </div>
          <ol class="grid gap-3">
            <li v-for="s in STEPS" :key="s.id" class="grid grid-cols-[20px_1fr] items-start gap-3 text-sm">
              <span class="grid size-5 place-items-center">
                <Loader2Icon v-if="create.steps[s.id] === 'running'" class="size-4 animate-spin" />
                <CheckIcon
                  v-else-if="create.steps[s.id] === 'done'"
                  class="size-4 text-emerald-600 dark:text-emerald-400"
                />
                <CircleAlertIcon
                  v-else-if="create.steps[s.id] === 'error' || create.steps[s.id] === 'manual'"
                  class="text-destructive size-4"
                />
                <span v-else class="bg-border size-2 rounded-full" />
              </span>
              <div class="grid gap-2">
                <span :class="{ 'text-muted-foreground': create.steps[s.id] === 'waiting' }">{{ s.label }}</span>
                <div
                  v-if="s.id === 'pages' && create.steps.pages === 'manual'"
                  class="bg-muted/50 grid gap-2 rounded-lg border p-3 text-sm"
                >
                  <p>
                    GitHub не дал приложению включить Pages автоматически. Откройте настройки, в «Source» выберите
                    <b>GitHub Actions</b> — и вернитесь сюда.
                  </p>
                  <div class="flex gap-2">
                    <Button size="sm" @click="create.pagesSettingsUrl && openUrl(create.pagesSettingsUrl)">
                      <ExternalLinkIcon /> Открыть настройки Pages
                    </Button>
                    <Button size="sm" variant="outline" :disabled="create.running" @click="create.start()">
                      Готово, проверить
                    </Button>
                  </div>
                </div>
                <button
                  v-if="s.id === 'deploy' && create.run && create.steps.deploy !== 'waiting'"
                  class="text-muted-foreground w-fit text-xs underline underline-offset-2"
                  @click="openUrl(create.run.htmlUrl)"
                >
                  Лог сборки на GitHub
                </button>
              </div>
            </li>
          </ol>
          <p v-if="create.error" class="text-destructive flex gap-2 text-sm">
            <CircleAlertIcon class="mt-0.5 size-4 shrink-0" /> {{ create.error }}
          </p>
          <div class="flex flex-wrap gap-2">
            <Button
              v-if="!create.finished && !create.running && Object.values(create.steps).includes('error')"
              @click="create.start()"
            >
              Повторить
            </Button>
            <template v-if="create.finished">
              <Button @click="openEditor">Открыть редактор</Button>
              <Button v-if="create.siteUrl" variant="outline" @click="openUrl(create.siteUrl)">
                <ExternalLinkIcon /> Открыть сайт
              </Button>
            </template>
            <Button
              v-else-if="create.steps.setup === 'done'"
              variant="ghost"
              :disabled="create.running && create.steps.deploy !== 'running'"
              @click="openEditor"
            >
              Открыть редактор, не дожидаясь сайта
            </Button>
          </div>
        </template>
      </div>
    </main>
  </div>
</template>
