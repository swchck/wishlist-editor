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
  const blockers = computed(() => {
    const broken = (slug: string) => {
      const card = wishlist.cards.get(slug);
      return !!card && (!!card.parseError || Object.keys(card.errors).length > 0);
    };
    const label = (slug: string) => String(wishlist.cards.get(slug)?.data.title ?? slug);
    // unpushed commits may hold any card, and a conflict resolved to the GitHub side
    // brings in content nobody validated here, so then every card is checked
    const slugs = wishlist.ahead
      ? [...wishlist.cards.keys()]
      : described.value.flatMap((c) => (c.slug && c.kind !== "deleted" ? [c.slug] : []));
    return slugs.filter(broken).map((slug) => ({ slug, label: label(slug) }));
  });

  async function run(message: string, choices: Record<string, Side> = {}) {
    phase.value = "publishing";
    error.value = null;
    wishlist.locked = true;
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
    } finally {
      wishlist.locked = false;
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
