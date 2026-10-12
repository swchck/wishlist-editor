import { invoke } from "@tauri-apps/api/core";

export interface RawItem {
  slug: string;
  text: string;
}

export interface Snapshot {
  root: string;
  configText: string;
  items: RawItem[];
}

/** Error shape produced by the Rust `AppError` serializer. */
export interface AppError {
  kind: string;
  message: string;
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as AppError).message);
  return String(e);
}

export const api = {
  openWorkspace: (path: string) => invoke<Snapshot>("open_workspace", { path }),
  listItems: () => invoke<RawItem[]>("list_items"),
  writeItem: (slug: string, text: string) => invoke<void>("write_item", { slug, text }),
  createItem: (slug: string, text: string) => invoke<void>("create_item", { slug, text }),
  deleteItem: (slug: string) => invoke<void>("delete_item", { slug }),
  renameItem: (from: string, to: string) => invoke<void>("rename_item", { from, to }),
};
