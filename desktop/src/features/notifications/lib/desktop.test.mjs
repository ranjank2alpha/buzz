import assert from "node:assert/strict";
import test from "node:test";

import {
  getDesktopNotificationPermissionState,
  requestDesktopNotificationAccess,
  sendDesktopNotification,
} from "./desktop.ts";

test("getDesktopNotificationPermissionState returns default in Tauri even if window.Notification.permission is denied", async () => {
  const originalWindow = globalThis.window;

  try {
    globalThis.isTauri = true;
    globalThis.window = {
      Notification: {
        permission: "denied",
      },
      __TAURI_INTERNALS__: {
        invoke: async (cmd) => {
          if (cmd === "plugin:notification|is_permission_granted") {
            return false;
          }
          throw new Error(`Unexpected invoke command: ${cmd}`);
        },
      },
    };

    const state = await getDesktopNotificationPermissionState();
    assert.equal(
      state,
      "default",
      "In Tauri mode, isPermissionGranted false must yield 'default', not WebView2 'denied'",
    );
  } finally {
    delete globalThis.isTauri;
    globalThis.window = originalWindow;
  }
});

test("getDesktopNotificationPermissionState returns granted in Tauri when isPermissionGranted is true", async () => {
  const originalWindow = globalThis.window;

  try {
    globalThis.isTauri = true;
    globalThis.window = {
      Notification: {
        permission: "denied",
      },
      __TAURI_INTERNALS__: {
        invoke: async (cmd) => {
          if (cmd === "plugin:notification|is_permission_granted") {
            return true;
          }
          throw new Error(`Unexpected invoke command: ${cmd}`);
        },
      },
    };

    const state = await getDesktopNotificationPermissionState();
    assert.equal(state, "granted");
  } finally {
    delete globalThis.isTauri;
    globalThis.window = originalWindow;
  }
});

test("getDesktopNotificationPermissionState falls back to window.Notification.permission when not in Tauri", async () => {
  const originalWindow = globalThis.window;

  try {
    globalThis.window = {
      Notification: {
        permission: "granted",
      },
    };

    const state = await getDesktopNotificationPermissionState();
    assert.equal(state, "granted");
  } finally {
    globalThis.window = originalWindow;
  }
});

test("requestDesktopNotificationAccess invokes request_permission plugin command in Tauri mode", async () => {
  const originalWindow = globalThis.window;
  let invokedCommand = null;

  try {
    globalThis.isTauri = true;
    globalThis.window = {
      __TAURI_INTERNALS__: {
        invoke: async (cmd) => {
          invokedCommand = cmd;
          if (cmd === "plugin:notification|request_permission") {
            return "granted";
          }
          throw new Error(`Unexpected invoke command: ${cmd}`);
        },
      },
    };

    const state = await requestDesktopNotificationAccess();
    assert.equal(invokedCommand, "plugin:notification|request_permission");
    assert.equal(state, "granted");
  } finally {
    delete globalThis.isTauri;
    globalThis.window = originalWindow;
  }
});

test("sendDesktopNotification invokes notify plugin command in Tauri mode when granted", async () => {
  const originalWindow = globalThis.window;
  const invokedCalls = [];

  try {
    globalThis.isTauri = true;
    globalThis.window = {
      __TAURI_INTERNALS__: {
        invoke: async (cmd, args) => {
          invokedCalls.push({ cmd, args });
          if (cmd === "plugin:notification|is_permission_granted") {
            return true;
          }
          if (cmd === "plugin:notification|notify") {
            return null;
          }
          throw new Error(`Unexpected invoke command: ${cmd}`);
        },
      },
    };

    const sent = await sendDesktopNotification({
      title: "Test Alert",
      body: "Hello World",
    });

    assert.equal(sent, true);
    assert.equal(invokedCalls.length, 2);
    assert.equal(invokedCalls[0].cmd, "plugin:notification|is_permission_granted");
    assert.equal(invokedCalls[1].cmd, "plugin:notification|notify");
    assert.deepEqual(invokedCalls[1].args, {
      options: {
        title: "Test Alert",
        body: "Hello World",
        extra: undefined,
      },
    });
  } finally {
    delete globalThis.isTauri;
    globalThis.window = originalWindow;
  }
});
