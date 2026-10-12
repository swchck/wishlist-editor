import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";

export interface RawItem {
  slug: string;
  text: string;
}

export interface Snapshot {
  root: string;
  configText: string;
  items: RawItem[];
  files: string[];
  /** False when the repo uses another schemaVersion; Rust refuses every write then. */
  writable: boolean;
}

export interface User {
  login: string;
  id: number;
  name: string | null;
  avatarUrl: string;
}

export interface AuthStatus {
  user: User | null;
  offline: boolean;
}

export interface DevicePrompt {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
}

export interface RepoId {
  owner: string;
  name: string;
}

export interface RemoteWishlist extends RepoId {
  title: string | null;
  items: number;
  siteUrl: string;
  updatedAt: string;
}

export interface Change {
  path: string;
  kind: "added" | "modified" | "deleted";
}

export interface RepoState {
  changes: Change[];
  ahead: number;
  behind: number;
}

export type Pull = "up_to_date" | "fast_forwarded" | "blocked" | "diverged";

export interface Opened {
  repo: RepoId;
  snapshot: Snapshot;
  state: RepoState;
  pull: Pull | null;
  siteUrl: string;
}

export interface Conflict {
  path: string;
  local: string | null;
  remote: string | null;
}

export type Side = "local" | "remote";

export type Published =
  { outcome: "pushed"; sha: string } | { outcome: "conflicts"; files: Conflict[] } | { outcome: "nothingToPublish" };

export interface DeployRun {
  status: string;
  conclusion: string | null;
  htmlUrl: string;
}

export type NameCheck = "free" | "unfinished" | "wishlist" | "taken";

export interface PagesState {
  enabled: boolean;
  settingsUrl: string;
  siteUrl: string | null;
}

export type ReservationsCheck = "ready" | "oldSchema" | "noSchema" | "badKey" | "secretKey" | "unreachable";

export interface ReservationsSettings {
  url: string;
  anonKey: string;
}

export interface Setup {
  title: string;
  defaultCurrency: string;
  reservations: ReservationsSettings | null;
  keepExamples: boolean;
}

export interface Finished {
  sha: string | null;
  siteUrl: string;
}

/** Error shape produced by the Rust `AppError` serializer. */
export interface AppError {
  kind: string;
  message: string;
}

export function isAppError(e: unknown, kind?: string): e is AppError {
  return !!e && typeof e === "object" && "kind" in e && (kind === undefined || (e as AppError).kind === kind);
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as AppError).message);
  return String(e);
}

let onUnauthorized: (() => void) | null = null;

/** Registers the one place that reacts to a revoked or expired token (sign-in screen). */
export function handleUnauthorized(fn: () => void) {
  onUnauthorized = fn;
}

async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    if (isAppError(e, "unauthorized")) onUnauthorized?.();
    throw e;
  }
}

export const api = {
  authStatus: () => invoke<AuthStatus>("auth_status"),
  authStart: () => invoke<DevicePrompt>("auth_start"),
  authWait: () => invoke<User>("auth_wait"),
  authCancel: () => invoke<void>("auth_cancel"),
  signOut: () => invoke<void>("sign_out"),

  listWishlists: () => invoke<RemoteWishlist[]>("list_wishlists"),
  localWishlists: () => invoke<RepoId[]>("local_wishlists"),
  openRepo: (owner: string, name: string) => invoke<Opened>("open_repo", { owner, name }),
  closeRepo: () => invoke<void>("close_repo"),
  reload: () => invoke<Snapshot>("reload"),
  repoStatus: () => invoke<RepoState>("repo_status"),
  discardChange: (path: string) => invoke<void>("discard_change", { path }),
  publish: (message: string, choices: Record<string, Side> = {}) => invoke<Published>("publish", { message, choices }),
  deployStatus: (sha: string) => invoke<DeployRun | null>("deploy_status", { sha }),

  checkName: (name: string) => invoke<NameCheck>("check_name", { name }),
  createRepo: (name: string) => invoke<void>("create_repo", { name }),
  ensurePages: (name: string) => invoke<PagesState>("ensure_pages", { name }),
  checkReservations: (url: string, anonKey: string) =>
    invoke<ReservationsCheck>("check_reservations", { url, anonKey }),
  finishSetup: (name: string, setup: Setup) => invoke<Finished>("finish_setup", { name, setup }),

  listItems: () => invoke<RawItem[]>("list_items"),
  writeItem: (slug: string, text: string) => invoke<void>("write_item", { slug, text }),
  createItem: (slug: string, text: string) => invoke<void>("create_item", { slug, text }),
  deleteItem: (slug: string) => invoke<void>("delete_item", { slug }),
  renameItem: (from: string, to: string) => invoke<void>("rename_item", { from, to }),
};
