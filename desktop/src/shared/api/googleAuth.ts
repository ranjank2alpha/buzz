import { invoke } from "@tauri-apps/api/core";

export type GoogleAuthResult = {
  identity: {
    pubkey: string;
    displayName: string;
    storage: string;
    lost: boolean;
    locked: boolean;
    resetFailed: boolean;
  };
  email: string;
  name?: string | null;
  googleSub: string;
  nsec: string;
};

export async function startGoogleWorkspaceLogin(): Promise<GoogleAuthResult> {
  return invoke<GoogleAuthResult>("start_google_workspace_login");
}
