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
    try {
      prompt.value = await api.authStart();
      onPrompt?.(prompt.value);
      phase.value = "awaiting-code";
      user.value = await api.authWait();
      offline.value = false;
      phase.value = "signed-in";
    } catch (e) {
      if (!isAppError(e, "auth_cancelled")) error.value = errorMessage(e);
      if (phase.value === "awaiting-code") phase.value = "signed-out";
    } finally {
      prompt.value = null;
    }
  }

  async function cancel() {
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
