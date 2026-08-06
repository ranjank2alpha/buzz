import { invoke } from "@tauri-apps/api/core";
import type { Identity } from "@/shared/api/types";

export type GoogleAuthResult = {
  identity: Identity;
  email: string;
  name?: string | null;
  isFreshKey: boolean;
};

export async function startGoogleWorkspaceLogin(): Promise<GoogleAuthResult> {
  return invoke<GoogleAuthResult>("start_google_workspace_login");
}

export async function isGoogleSsoAvailable(): Promise<boolean> {
  return invoke<boolean>("google_sso_available");
}
