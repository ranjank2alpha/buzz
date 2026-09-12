# Recording brief: Windows toast demo for PR #6876

Goal: capture a short screen recording that proves the native Windows toast works end to end, to attach to https://github.com/block/buzz/pull/6876. This is the single highest-leverage thing to move that PR: a notifications fix with visual proof is far more likely to get a maintainer to pick it up.

## Environment (important)

- Record on a real Windows 11 machine using an installed fork build (0.5.22-1 or later), not a dev build. The point is to show the shipped path, including AUMID and Start Menu registration, working on a normal install.
- You need two participants so a message can actually arrive: the machine being recorded is the receiver, and you send from a second account (phone, another PC, or a second Buzz instance). Have the second account ready before you start.
- Have one channel and one DM open and reachable, with a couple of realistic-looking prior messages so it does not read as an empty test account. Keep names and content professional (no "test test 123").

## Recording specs

- Region: the whole primary display (the toast appears at the bottom-right of the screen, so do not crop to just the Buzz window).
- Length: 15 to 25 seconds. Shorter is better.
- Frame rate: 30 fps while recording.
- Capture the mouse cursor.
- Save the raw capture as MP4 (H.264).
- Preferred deliverable: MP4. GitHub renders MP4/MOV inline in a PR and it looks better than a GIF. Keep it under 10 MB (GitHub's per-file limit). At 720p and ~20 seconds it will be well under.
- Fallback deliverable: if MP4 is a problem, also produce a GIF at about 900 px wide, 12 to 15 fps, under 5 MB.

## Clip 1 (required): toast delivery and click-through

Storyboard:

1. Buzz is open on the receiver account. Show it briefly, then click another window (or minimise Buzz) so Buzz is not the focused app. Pause about 1 second.
2. From the second account, send a DM to the receiver with a normal sentence, for example "Are we still on for the 3pm review?"
3. The native Windows toast appears at the bottom-right, showing the sender's name and a preview of the message. Let it sit for about 2 seconds so it is clearly readable.
4. Click the toast.
5. Buzz comes to the front and opens that exact DM conversation, with the new message visible.
6. Hold on the opened conversation for about 2 seconds, then stop recording.

If you can fit it without going over 25 seconds, repeat step 2 to 4 once more with a channel message (send an @mention in a channel) to show it works for channels too. If it makes the clip too long, keep the DM version only.

## Clip 2 (optional, for issue #6982): the toggle now survives a restart

This proves the second fix (the permission latch that used to force the Desktop alerts toggle off on every launch). Short and worth having as a second attachment.

Storyboard:

1. Open Settings > Notifications. Show that "Desktop alerts" is On.
2. Fully quit Buzz (exit from the system tray, and confirm the process is gone, not just the window closed).
3. Relaunch Buzz.
4. Open Settings > Notifications again. Show that "Desktop alerts" is still On.

## Where to save

Save the files here so they are easy to find and attach:

`C:\Users\rkart\.gemini\antigravity\scratch\buzz\docs\media\`

Suggested names:
- `pr-6876-toast-delivery.mp4`
- `pr-6876-toggle-persists.mp4` (if you record clip 2)

## After recording

Do not upload anything automatically. Hand the files back so they can be reviewed first, then attached to the PR with the caption below.

Caption to post as a new comment on PR #6876 (attach the clip in the same comment):

> Short demo on Windows 11: an incoming DM raises a native Action Center toast naming the sender, and clicking it focuses Buzz and opens that conversation. Same path handles channel messages, mentions, and thread replies.

If clip 2 is included, add:

> Second clip: the Desktop alerts toggle now stays on across a full restart, which fixes the force-off behaviour reported in #6982.
