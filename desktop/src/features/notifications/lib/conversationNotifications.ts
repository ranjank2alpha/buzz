export type ConversationNotificationConfig = {
  toastEnabled: boolean;
  soundEnabled: boolean;
};

const STORAGE_KEY = "buzz-conversation-notification-config.v1";

export function getConversationNotificationConfig(
  targetId: string | null | undefined,
): ConversationNotificationConfig {
  if (!targetId || typeof window === "undefined") {
    return { toastEnabled: true, soundEnabled: true };
  }

  try {
    const raw = window.localStorage.getItem(`${STORAGE_KEY}:${targetId}`);
    if (!raw) {
      return { toastEnabled: true, soundEnabled: true };
    }
    const parsed = JSON.parse(raw);
    return {
      toastEnabled:
        typeof parsed.toastEnabled === "boolean" ? parsed.toastEnabled : true,
      soundEnabled:
        typeof parsed.soundEnabled === "boolean" ? parsed.soundEnabled : true,
    };
  } catch {
    return { toastEnabled: true, soundEnabled: true };
  }
}

export function setConversationNotificationConfig(
  targetId: string,
  config: Partial<ConversationNotificationConfig>,
): ConversationNotificationConfig {
  if (!targetId || typeof window === "undefined") {
    return { toastEnabled: true, soundEnabled: true };
  }

  const current = getConversationNotificationConfig(targetId);
  const updated = { ...current, ...config };
  try {
    window.localStorage.setItem(
      `${STORAGE_KEY}:${targetId}`,
      JSON.stringify(updated),
    );
    window.dispatchEvent(
      new CustomEvent("buzz:conversation-notification-config-changed", {
        detail: { targetId, config: updated },
      }),
    );
  } catch {
    // Ignore storage write failures
  }
  return updated;
}
