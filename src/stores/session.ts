import { defineStore } from "pinia";
import { ref } from "vue";
import { api, errorMessage, handleUnauthorized, isAppError, type DevicePrompt, type User } from "@/lib/api";

export type SessionPhase = "checking" | "signed-out" | "awaiting-code" | "signed-in";

export const useSession = defineStore("session", () => {
  const phase = ref<SessionPhase>("checking");
  const user = ref<User | null>(null);
  /** GitHub was unreachable at start; local copies still open, publishing waits. */
  const offline = ref(false);
  const prompt = ref<DevicePrompt | null>(null);
  const error = ref<string | null>(null);
  // a cancelled poll only notices on its next tick; by then a newer flow may be running
  let flow = 0;

  async function check() {
    phase.value = "checking";
    error.value = null;
    try {
      const st = await api.authStatus();
      user.value = st.user;
      offline.value = st.offline;
      phase.value = st.user ? "signed-in" : "signed-out";
    } catch (e) {
      error.value = errorMessage(e);
      phase.value = "signed-out";
    }
  }

  /** Runs the device flow; `onPrompt` fires as soon as the code is known, before approval. */
  async function signIn(onPrompt?: (p: DevicePrompt) => void) {
    error.value = null;
    const mine = ++flow;
    try {
      const p = await api.authStart();
      if (mine !== flow) return;
      prompt.value = p;
      onPrompt?.(p);
      phase.value = "awaiting-code";
      const u = await api.authWait();
      if (mine !== flow) return;
      user.value = u;
      offline.value = false;
      phase.value = "signed-in";
      prompt.value = null;
    } catch (e) {
      if (mine !== flow) return;
      if (!isAppError(e, "auth_cancelled")) error.value = errorMessage(e);
      phase.value = "signed-out";
      prompt.value = null;
    }
  }

  async function cancel() {
    flow++;
    await api.authCancel();
    prompt.value = null;
    phase.value = "signed-out";
  }

  async function signOut() {
    await api.signOut();
    user.value = null;
    phase.value = "signed-out";
  }

  handleUnauthorized(() => {
    if (phase.value !== "signed-in") return;
    error.value = "GitHub больше не принимает вход — войдите заново. Несохранённого на сайте ничего не потеряно.";
    void signOut().catch(() => {});
  });

  return { phase, user, offline, prompt, error, check, signIn, cancel, signOut };
});
