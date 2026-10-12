import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api, errorMessage, type Conflict, type DeployRun, type Side } from "@/lib/api";
import { commitMessage, describeChanges } from "@/lib/wishlist/changes";
import { useWishlist } from "./wishlist";

const DEPLOY_POLL_MS = 5000;
// a Pages build of this site takes ~1 min; past 10 the run is stuck or never started
const DEPLOY_GIVE_UP_MS = 10 * 60_000;

export type PublishPhase = "idle" | "publishing" | "conflicts" | "done" | "error";

export interface Deploy {
  sha: string;
  run: DeployRun | null;
  startedAt: number;
  /** True once polling stopped without a finished run. */
  lost: boolean;
}

export const usePublish = defineStore("publish", () => {
  const wishlist = useWishlist();
  const phase = ref<PublishPhase>("idle");
  const error = ref<string | null>(null);
  const conflicts = ref<Conflict[]>([]);
  const deploy = ref<Deploy | null>(null);
  let pollTimer: ReturnType<typeof setTimeout> | undefined;

  const described = computed(() =>
    describeChanges(wishlist.changes, (slug) => {
      const t = wishlist.cards.get(slug)?.data.title;
      return typeof t === "string" ? t : undefined;
    }),
  );
  const suggestedMessage = computed(() => commitMessage(described.value));

  /** Changed cards the site build would reject; publishing is blocked while any remain. */
  const blockers = computed(() =>
    described.value.flatMap((c) => {
      if (!c.slug || c.kind === "deleted") return [];
      const card = wishlist.cards.get(c.slug);
      if (!card) return [];
      return card.parseError || Object.keys(card.errors).length ? [{ slug: c.slug, label: c.label }] : [];
    }),
  );

  async function run(message: string, choices: Record<string, Side> = {}) {
    phase.value = "publishing";
    error.value = null;
    try {
      await wishlist.flushAll();
      const res = await api.publish(message || suggestedMessage.value, choices);
      if (res.outcome === "conflicts") {
        conflicts.value = res.files;
        phase.value = "conflicts";
        return;
      }
      conflicts.value = [];
      await wishlist.reload();
      phase.value = "done";
      if (res.outcome === "pushed") watchDeploy(res.sha);
    } catch (e) {
      error.value = errorMessage(e);
      phase.value = "error";
      // a commit may have been made before the push failed; show it as pending
      await wishlist.refreshStatus().catch(() => {});
    }
  }

  function watchDeploy(sha: string) {
    clearTimeout(pollTimer);
    deploy.value = { sha, run: null, startedAt: Date.now(), lost: false };
    const tick = async () => {
      const d = deploy.value;
      if (!d || d.sha !== sha) return;
      try {
        d.run = await api.deployStatus(sha);
      } catch {
        // transient API trouble; the next tick retries
      }
      if (d.run?.status === "completed") return;
      if (Date.now() - d.startedAt > DEPLOY_GIVE_UP_MS) {
        d.lost = true;
        return;
      }
      pollTimer = setTimeout(tick, DEPLOY_POLL_MS);
    };
    void tick();
  }

  function reset() {
    if (phase.value !== "publishing") phase.value = "idle";
    error.value = null;
  }

  return { phase, error, conflicts, deploy, described, suggestedMessage, blockers, run, reset };
});
