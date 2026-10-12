import { defineStore } from "pinia";
import { reactive, ref } from "vue";
import { api, errorMessage, type DeployRun, type ReservationsSettings } from "@/lib/api";

export type StepId = "repo" | "pages" | "setup" | "deploy";
export type StepState = "waiting" | "running" | "done" | "manual" | "error";

const DEPLOY_POLL_MS = 5000;
const DEPLOY_GIVE_UP_MS = 10 * 60_000;

export const useCreate = defineStore("create", () => {
  const name = ref("wishlist");
  const title = ref("Мой вишлист");
  const currency = ref("RUB");
  const reservations = ref<ReservationsSettings | null>(null);
  const keepExamples = ref(false);

  const steps = reactive<Record<StepId, StepState>>({
    repo: "waiting",
    pages: "waiting",
    setup: "waiting",
    deploy: "waiting",
  });
  const error = ref<string | null>(null);
  const pagesSettingsUrl = ref<string | null>(null);
  const siteUrl = ref<string | null>(null);
  const run = ref<DeployRun | null>(null);
  /** Commit that triggers the first deploy; kept so a retry can resume watching it. */
  const sha = ref<string | null>(null);
  const running = ref(false);
  /** The picker shows the wizard instead of the list; the editor's switcher can ask for it too. */
  const wizardOpen = ref(false);
  const finished = ref(false);

  function reset() {
    for (const k of Object.keys(steps) as StepId[]) steps[k] = "waiting";
    error.value = null;
    pagesSettingsUrl.value = null;
    siteUrl.value = null;
    run.value = null;
    sha.value = null;
    finished.value = false;
  }

  async function step(id: StepId, work: () => Promise<StepState | void>) {
    if (steps[id] === "done") return true;
    steps[id] = "running";
    try {
      steps[id] = (await work()) ?? "done";
      return steps[id] === "done";
    } catch (e) {
      steps[id] = "error";
      error.value = errorMessage(e);
      return false;
    }
  }

  /** Runs every step that is not done yet; calling it again resumes after an error or a manual step. */
  async function start() {
    if (running.value) return;
    running.value = true;
    error.value = null;
    try {
      if (!(await step("repo", () => api.createRepo(name.value)))) return;
      const pagesOk = await step("pages", async () => {
        const p = await api.ensurePages(name.value);
        pagesSettingsUrl.value = p.settingsUrl;
        if (p.siteUrl) siteUrl.value = p.siteUrl;
        return p.enabled ? "done" : "manual";
      });
      if (!pagesOk) return;
      const setupOk = await step("setup", async () => {
        const res = await api.finishSetup(name.value, {
          title: title.value,
          defaultCurrency: currency.value,
          reservations: reservations.value,
          keepExamples: keepExamples.value,
        });
        sha.value = res.sha;
        siteUrl.value = res.siteUrl;
      });
      if (!setupOk) return;
      const pushed = sha.value;
      await step("deploy", () => (pushed ? waitForDeploy(pushed) : Promise.resolve("done" as const)));
      finished.value = steps.deploy === "done";
    } finally {
      running.value = false;
    }
  }

  async function waitForDeploy(sha: string): Promise<StepState> {
    const started = Date.now();
    for (;;) {
      try {
        run.value = await api.deployStatus(sha);
      } catch {
        // the API hiccups now and then; the next tick retries
      }
      if (run.value?.status === "completed") {
        if (run.value.conclusion === "success") return "done";
        throw new Error("Первая сборка сайта не удалась — откройте лог сборки на GitHub");
      }
      if (Date.now() - started > DEPLOY_GIVE_UP_MS) {
        throw new Error("Сборка сайта идёт дольше 10 минут — проверьте её на GitHub");
      }
      await new Promise((r) => setTimeout(r, DEPLOY_POLL_MS));
    }
  }

  return {
    name,
    title,
    currency,
    reservations,
    keepExamples,
    steps,
    error,
    pagesSettingsUrl,
    siteUrl,
    run,
    running,
    wizardOpen,
    finished,
    reset,
    start,
  };
});
