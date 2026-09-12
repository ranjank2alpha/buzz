# Brief: Google Drive upload — confirmation + "folder vs separate links" choice

Owner handoff from the frontend/design pass. Repo: `ranjank2alpha/buzz`, branch `main` (currently at the released `0.5.23-1`). Everything below is desktop app (`desktop/`).

## Context

Large files (over 5 MB, or any video/audio/program) upload to the sender's own Google Drive and post as links instead of going to Buzz's relay. Routing lives in `desktop/src/features/messages/lib/routedMediaUpload.ts` + `driveUploadRouting.mjs`. The Rust side (`desktop/src-tauri/src/google_meet/drive.rs`) already uploads every file into a per-user "Buzz uploads" folder (`ensure_uploads_folder`) via a resumable session, and returns each file's own `webViewLink`. That is why today you get one link per file.

Two pieces of work:

1. A confirmation before a Drive upload (already written, unbuilt — build and verify first).
2. A user choice between "one folder link" and "separate links" (design below — implement after 1 is green).

## Part 1 — already written, needs build + verify (do this first)

I added a whole-batch Drive confirmation. It is written but I could not compile or test it (my sandbox lost file access). Please `pnpm typecheck` + `pnpm test`, fix anything, and confirm the dialog appears before layering Part 2.

Files touched:
- `desktop/src/features/messages/lib/routedMediaUpload.ts` — new `uploadMediaFileToDrive(file, progressId?, signal?, onDispatch?)` that forces a file to Drive regardless of size (refuses if Drive not connected).
- `desktop/src/features/messages/lib/useMediaUpload.ts` — new `DriveUploadConfirmRequest` type + `driveConfirm` state; `isDriveBound(file)` helper; `startUploads(files, toDrive=false)` extracted from the old `uploadFiles`; `uploadFiles` now: if ANY file in the batch is Drive-bound, the WHOLE batch goes to Drive after one confirmation (`startUploads(files, true)`), else relay as before. `driveConfirm` is exposed from the hook.
- `desktop/src/features/messages/ui/DriveUploadConfirmDialog.tsx` — new. Connected variant ("Upload to Google Drive?") and not-connected variant ("Connect Google Drive", opens Settings → Voice via `requestConnectGoogleDrive`).
- `desktop/src/features/messages/ui/MessageComposer.tsx` — imports and renders `<DriveUploadConfirmDialog request={media.driveConfirm} />`.

Current behaviour: mixed batch with any large file → one prompt → whole batch to Drive as separate per-file links. Cancel drops the batch. All-small batch → relay, no prompt. Videos/voice notes ride a separate queued path and are not covered by this prompt (known, out of scope for now).

## Part 2 — the folder-vs-separate choice (implement after Part 1 is green)

Goal: when a batch is confirmed for Drive, let the user pick "One folder link" (all files in a single shared Drive folder, one link in the message) or "Separate links" (current behaviour, one link per file).

### Frontend

- `DriveUploadConfirmRequest.resolve` changes from `(proceed: boolean) => void` to `(choice: "folder" | "separate" | null) => void` (null = cancel). Update `DriveUploadConfirmDialog` connected variant to three actions: **One folder link**, **Separate links**, **Cancel**. Not-connected variant stays Connect/Cancel.
- In `useMediaUpload.uploadFiles`, await the choice: `"separate"` → `startUploads(files, true)` (existing per-file Drive path); `"folder"` → new `uploadFilesAsDriveFolder(files)`; `null` → drop the batch.
- `uploadFilesAsDriveFolder(files)` orchestration: create a fresh per-batch folder (Rust, below), upload each file into it with per-file progress previews (reuse `reserveUploadingPreview`/progress), then fill ONE slot with a single external attachment: an `ImetaMedia` with `external: true`, `url = folder.webViewLink`, `filename = <folder name>` (mirror the shape `uploadViaDrive` returns in `routedMediaUpload.ts`). This is the one reshape from the current model: N files uploaded, but ONE attachment/slot produced. Everything downstream (imeta markdown as a plain `[name](url)` link, Files-tab link entry) already handles a single external link.
- `tauriDrive.ts`: add `createDriveBatchFolder(name: string): Promise<{ id: string; webViewLink: string }>` (invokes a new command), and add an optional `parentId` to `uploadFileToDrive` that sends an `x-buzz-parent-id` header so a file lands in the batch folder instead of the default "Buzz uploads" folder.

### Rust (`desktop/src-tauri/src/google_meet/drive.rs`)

- New command `create_drive_batch_folder(name) -> { id, webViewLink }`: create a NEW folder (parent it under the existing "Buzz uploads" folder is fine), and share it `role: "reader", type: "anyone"` via `permissions.create` so recipients outside the sender's org can open it. Return `id` + `webViewLink`. Reuse the existing `DriveFile` struct and folder-creation call in `ensure_uploads_folder` as the template. Important: it must be a NEW folder per batch, never the shared "Buzz uploads" folder, or the link would expose every file the user has ever uploaded.
- Modify `upload_drive_bytes_raw` to read an optional `x-buzz-parent-id` header (via `optional_header`) and, when present, pass it as the `folder_id` to `begin_resumable_session` instead of calling `ensure_uploads_folder`. Default path unchanged.
- Check how single-file links are currently made openable by recipients (see the module's top comment about `permissions.create` possibly being redundant). Whatever makes today's per-file `webViewLink` work for recipients, replicate at the folder level. If per-file links already rely on an explicit anyone-with-link permission, do the same on the folder; if the account has a default that covers it, the folder may inherit it.

### Naming / UX

- Folder name suggestion: something readable and unique, e.g. `Buzz — <channel name> — <YYYY-MM-DD HH:mm>`. The message's single link label can be the folder name or `"N files (Drive folder)"`.
- Optional nicety, not required: remember the last choice (folder vs separate) in `localStorage` and preselect it.

## Verification (both parts)

- `pnpm typecheck`, `pnpm test`, and `cargo check` in `desktop/src-tauri`.
- Manual on a real build: mixed batch (2 small, 2 large). "Separate links" → 4 links, 4 files in Drive. "One folder link" → 1 link, folder in Drive holds all 4, and a second account can open the link. Cancel → nothing sent. All-small batch → relay, no prompt.
- Build the NSIS installer and smoke-launch (window renders) before tagging.

## Release

After green: bump all four version spots to `0.5.23-2` (`desktop/package.json`, `desktop/src-tauri/tauri.conf.json`, `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/Cargo.lock`), add a What's New entry in `desktop/src/features/whatsNew/changelog.ts`, note it in `CONTEXT.md`, then merge to main, tag `v0.5.23-2`, and let CI build the signed release.

## Still open (separate from this)

The version-picker report (picker listing files that should be superseded) is unresolved. `channelFiles.ts` and `FileVersionPicker.tsx` are byte-identical to the last working release, so the filter logic didn't change. The quickest triage: check whether the same files are marked "outdated" in the Files tab. If the Files tab marks them outdated but the picker lists them, that's a real contradiction; if neither does, the supersedes links aren't resolving (a data/fetch question, not the picker).
