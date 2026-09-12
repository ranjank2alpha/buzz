import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/shared/ui/alert-dialog";
import { Button } from "@/shared/ui/button";
import type { DriveUploadConfirmRequest } from "@/features/messages/lib/useMediaUpload";
import { requestConnectGoogleDrive } from "@/features/messages/lib/openDriveSettingsEvent";

/**
 * Confirmation shown before a message is uploaded to the sender's own Google
 * Drive and shared as links (one link per file) rather than pushed to Buzz's
 * relay. A message takes this path whenever any file in it is over 5 MB, or a
 * video/audio/program: the whole batch goes to Drive together, so smaller
 * files ride along rather than splitting off to the relay.
 *
 * Driven entirely by `useMediaUpload`'s `driveConfirm` state: `request` is the
 * pending decision (or null when nothing is open), and every exit path calls
 * `request.resolve` exactly once with the user's choice. A Promise ignores a
 * second resolve, so the belt-and-braces resolve on dismiss is harmless.
 */
export function DriveUploadConfirmDialog({
  request,
}: {
  request: DriveUploadConfirmRequest | null;
}) {
  const resolve = (choice: "folder" | "separate" | null) =>
    request?.resolve(choice);
  const count = request?.files.length ?? 0;
  const one = count === 1;
  const names = request?.files.map((file) => file.name).join(", ") ?? "";

  const connectedDescription = one
    ? `This file is over 5 MB, so it will be uploaded to your Google Drive and shared as a link: ${names}.`
    : `This message includes a large file, so all ${count} files will be uploaded to your Google Drive: ${names}. Choose whether to share them as a single folder link or separate links.`;

  const disconnectedDescription = one
    ? `This file is over 5 MB, so it needs Google Drive: ${names}. Connect your Google account under Settings → Voice to send it.`
    : `This message includes a large file, so all ${count} files need Google Drive: ${names}. Connect your Google account under Settings → Voice to send them.`;

  return (
    <AlertDialog
      onOpenChange={(next) => {
        if (!next) resolve(null);
      }}
      open={request !== null}
    >
      <AlertDialogContent>
        {request?.connected ? (
          <>
            <AlertDialogHeader>
              <AlertDialogTitle>Upload to Google Drive?</AlertDialogTitle>
              <AlertDialogDescription>
                {connectedDescription}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel asChild>
                <Button
                  onClick={() => resolve(null)}
                  type="button"
                  variant="outline"
                >
                  Cancel
                </Button>
              </AlertDialogCancel>
              {count > 1 ? (
                <>
                  <AlertDialogAction asChild>
                    <Button
                      onClick={() => resolve("separate")}
                      type="button"
                      variant="outline"
                    >
                      Separate links
                    </Button>
                  </AlertDialogAction>
                  <AlertDialogAction asChild>
                    <Button onClick={() => resolve("folder")} type="button">
                      One folder link
                    </Button>
                  </AlertDialogAction>
                </>
              ) : (
                <AlertDialogAction asChild>
                  <Button onClick={() => resolve("separate")} type="button">
                    Upload to Drive
                  </Button>
                </AlertDialogAction>
              )}
            </AlertDialogFooter>
          </>
        ) : (
          <>
            <AlertDialogHeader>
              <AlertDialogTitle>Connect Google Drive</AlertDialogTitle>
              <AlertDialogDescription>
                {disconnectedDescription}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel asChild>
                <Button
                  onClick={() => resolve(null)}
                  type="button"
                  variant="outline"
                >
                  Cancel
                </Button>
              </AlertDialogCancel>
              <AlertDialogAction asChild>
                <Button
                  onClick={() => {
                    requestConnectGoogleDrive();
                    resolve(null);
                  }}
                  type="button"
                >
                  Connect
                </Button>
              </AlertDialogAction>
            </AlertDialogFooter>
          </>
        )}
      </AlertDialogContent>
    </AlertDialog>
  );
}
