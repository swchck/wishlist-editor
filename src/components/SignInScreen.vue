<script setup lang="ts">
import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CheckIcon, CopyIcon, ExternalLinkIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useSession } from "@/stores/session";

const session = useSession();
const copied = ref(false);

function start() {
  void session.signIn((p) => void openUrl(p.verificationUri));
}

async function copy() {
  if (!session.prompt) return;
  await navigator.clipboard.writeText(session.prompt.userCode).catch(() => {});
  copied.value = true;
  setTimeout(() => (copied.value = false), 1500);
}
</script>

<template>
  <div class="grid h-full grid-cols-1 md:grid-cols-2">
    <div data-tauri-drag-region class="bg-muted/40 flex flex-col justify-center gap-5 border-r p-12">
      <div class="text-muted-foreground flex items-center gap-2 text-sm font-medium">
        <span class="text-foreground text-lg">✳</span> Wishlist Editor
      </div>
      <h1 class="text-3xl leading-tight font-bold tracking-tight text-balance">Ваш вишлист живёт на GitHub</h1>
      <p class="text-muted-foreground max-w-sm">
        Войдите, чтобы открыть его здесь. Правки сохраняются сразу, а на сайт попадают, когда вы нажмёте «Опубликовать».
      </p>
    </div>
    <div class="flex flex-col justify-center gap-5 p-12">
      <template v-if="session.phase === 'awaiting-code' && session.prompt">
        <div class="text-2xl font-bold tracking-tight">Подтвердите вход на GitHub</div>
        <p class="text-muted-foreground max-w-sm text-sm">
          Страница github.com/login/device уже открыта в браузере. Введите там этот код:
        </p>
        <div class="flex items-center gap-1.5" aria-label="Код входа">
          <span
            v-for="(ch, i) in session.prompt.userCode"
            :key="i"
            :class="
              ch === '-'
                ? 'text-muted-foreground w-3 text-center'
                : 'bg-card grid h-12 w-10 place-items-center rounded-lg border font-mono text-xl font-semibold'
            "
            >{{ ch }}</span
          >
        </div>
        <div class="text-muted-foreground flex items-center gap-2 text-sm">
          <Loader2Icon class="size-4 animate-spin" /> Ждём подтверждения…
        </div>
        <div class="flex flex-wrap gap-2">
          <Button variant="outline" @click="copy">
            <CheckIcon v-if="copied" /><CopyIcon v-else /> {{ copied ? "Скопировано" : "Скопировать код" }}
          </Button>
          <Button variant="ghost" @click="openUrl(session.prompt.verificationUri)">
            <ExternalLinkIcon /> Открыть страницу снова
          </Button>
          <Button variant="ghost" @click="session.cancel()">Отмена</Button>
        </div>
      </template>
      <template v-else>
        <Button size="lg" class="w-fit" :disabled="session.phase === 'checking'" @click="start">
          <Loader2Icon v-if="session.phase === 'checking'" class="animate-spin" />
          Войти через GitHub
        </Button>
        <p class="text-muted-foreground max-w-sm text-xs">
          Приложение получит доступ к вашим публичным репозиториям — чтобы читать вишлист и публиковать правки. Отозвать
          доступ можно в настройках GitHub → Applications.
        </p>
      </template>
      <Alert v-if="session.error" variant="destructive" class="max-w-md">
        <AlertDescription>{{ session.error }}</AlertDescription>
      </Alert>
    </div>
  </div>
</template>
