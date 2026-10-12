<script setup lang="ts">
import { onMounted, ref } from "vue";
import {
  ChevronRightIcon,
  HardDriveIcon,
  Loader2Icon,
  LogOutIcon,
  MoreHorizontalIcon,
  PlusIcon,
  RefreshCwIcon,
  Trash2Icon,
  WifiOffIcon,
} from "@lucide/vue";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import RemoveWishlistDialog, { type RemoveTarget } from "./RemoveWishlistDialog.vue";
import CreateWizard from "./CreateWizard.vue";
import { useCreate } from "@/stores/create";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { api, errorMessage, isAppError, type RepoId } from "@/lib/api";
import { plural } from "@/lib/wishlist/changes";
import { useSession } from "@/stores/session";
import { useWishlist } from "@/stores/wishlist";

interface Row extends RepoId {
  title: string | null;
  items: number | null;
  listId: string | null;
}

const session = useSession();
const wishlist = useWishlist();
const rows = ref<Row[] | null>(null);
const offline = ref(session.offline);
const error = ref<string | null>(null);
const opening = ref<string | null>(null);
const create = useCreate();
const removing = ref<{ target: RemoveTarget; mode: "delete" | "forget" } | null>(null);
// a wizard left mid-way (window closed on the progress page) reopens where it stopped
if (create.running || (create.steps.repo !== "waiting" && !create.finished)) create.wizardOpen = true;

async function load() {
  rows.value = null;
  error.value = null;
  try {
    rows.value = await api.listWishlists();
    offline.value = false;
  } catch (e) {
    if (!isAppError(e, "offline")) {
      error.value = errorMessage(e);
      rows.value = [];
      return;
    }
    offline.value = true;
    rows.value = (await api.localWishlists()).map((r) => ({ ...r, title: null, items: null, listId: null }));
  }
}

async function open(r: RepoId) {
  opening.value = `${r.owner}/${r.name}`;
  error.value = null;
  try {
    await wishlist.open(r);
  } catch (e) {
    error.value = errorMessage(e);
  } finally {
    opening.value = null;
  }
}

onMounted(load);
</script>

<template>
  <CreateWizard
    v-if="create.wizardOpen"
    @back="
      create.wizardOpen = false;
      load();
    "
  />
  <div v-else class="grid h-full grid-cols-1 md:grid-cols-2">
    <div data-tauri-drag-region class="bg-muted/40 flex flex-col justify-center gap-5 border-r p-12">
      <div class="text-muted-foreground flex items-center gap-2 text-sm font-medium">
        <span class="text-foreground text-lg">✳</span> Wishlist Editor
      </div>
      <h1 class="text-3xl leading-tight font-bold tracking-tight text-balance">Какой вишлист открыть?</h1>
      <div class="flex items-center gap-2 text-sm">
        <img v-if="session.user?.avatarUrl" :src="session.user.avatarUrl" alt="" class="size-6 rounded-full" />
        <span class="text-muted-foreground">Вошли как</span> {{ session.user?.login }}
        <Button variant="ghost" size="sm" class="ml-auto" @click="session.signOut()"><LogOutIcon /> Выйти</Button>
      </div>
    </div>
    <div class="flex min-h-0 flex-col justify-center gap-4 p-12">
      <Alert v-if="offline">
        <WifiOffIcon />
        <AlertDescription
          >Нет связи с GitHub. Можно открыть вишлисты, уже скачанные на этот компьютер.</AlertDescription
        >
      </Alert>
      <div class="flex items-center justify-between">
        <div class="text-muted-foreground text-xs font-semibold tracking-wider uppercase">
          {{ offline ? "На этом компьютере" : "Ваши вишлисты на GitHub" }}
        </div>
        <Button variant="ghost" size="icon-sm" aria-label="Обновить список" @click="load"><RefreshCwIcon /></Button>
      </div>
      <div v-if="rows === null" class="grid gap-2">
        <Skeleton v-for="i in 2" :key="i" class="h-14 w-full" />
      </div>
      <div v-else-if="rows.length" class="divide-y overflow-auto rounded-lg border">
        <div v-for="r in rows" :key="`${r.owner}/${r.name}`" class="hover:bg-muted/60 flex items-center pr-2">
          <button
            class="flex min-w-0 flex-1 items-center gap-3 py-3 pl-4 text-left"
            :disabled="!!opening"
            @click="open(r)"
          >
            <span class="min-w-0 flex-1">
              <span class="block truncate font-medium">{{ r.title ?? r.name }}</span>
              <span class="text-muted-foreground block truncate font-mono text-xs">{{ r.owner }}/{{ r.name }}</span>
            </span>
            <span v-if="r.items !== null" class="text-muted-foreground text-xs whitespace-nowrap">
              {{ r.items }} {{ plural(r.items, "карточка", "карточки", "карточек") }}
            </span>
            <Loader2Icon v-if="opening === `${r.owner}/${r.name}`" class="size-4 animate-spin" />
            <ChevronRightIcon v-else class="text-muted-foreground size-4" />
          </button>
          <DropdownMenu>
            <DropdownMenuTrigger as-child>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="`Действия с ${r.owner}/${r.name}`"
                :disabled="!!opening"
              >
                <MoreHorizontalIcon />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" class="w-64">
              <DropdownMenuItem @select="removing = { target: r, mode: 'forget' }">
                <HardDriveIcon /> Убрать с этого компьютера
              </DropdownMenuItem>
              <DropdownMenuItem
                v-if="!offline && r.owner === session.user?.login"
                variant="destructive"
                @select="removing = { target: r, mode: 'delete' }"
              >
                <Trash2Icon /> Удалить вишлист…
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
      <p v-else class="text-muted-foreground text-sm">
        {{
          offline ? "На этом компьютере пока нет скачанных вишлистов." : "Пока ни одного вишлиста — создайте первый."
        }}
      </p>
      <Button v-if="!offline" variant="outline" class="w-fit" :disabled="!!opening" @click="create.wizardOpen = true">
        <PlusIcon /> Создать новый вишлист
      </Button>
      <p v-if="opening" class="text-muted-foreground text-xs">
        Первое открытие скачивает репозиторий — это несколько секунд.
      </p>
      <RemoveWishlistDialog
        :target="removing?.target ?? null"
        :mode="removing?.mode ?? 'forget'"
        @close="removing = null"
        @removed="
          removing = null;
          load();
        "
      />
      <Alert v-if="error" variant="destructive">
        <AlertDescription>{{ error }}</AlertDescription>
      </Alert>
    </div>
  </div>
</template>
