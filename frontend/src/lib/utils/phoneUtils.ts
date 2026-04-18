let wakeLock: WakeLockSentinel | null = null

async function reacquireWakeLock() {
  if (document.visibilityState === "visible") {
    await preventScreenLock();
  }
}

export async function preventScreenLock() {
  try {
    wakeLock = await navigator.wakeLock.request("screen");
    wakeLock.addEventListener("release", () => {
      document.addEventListener("visibilitychange", reacquireWakeLock, {
        once: true,
      });
    });
  } catch {
    /* empty */
  }
}

export function toggleFullScreen() {
  if (!document.fullscreenElement) {
    document.body.requestFullscreen();
    return true;
  } else {
    document.exitFullscreen?.();
    return false;
  }
}