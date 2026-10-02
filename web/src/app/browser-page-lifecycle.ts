export interface BrowserPageLifecycleHandlers {
  readonly suspend: () => void;
  readonly restore: () => void;
  readonly dispose: () => void;
}

export function installBrowserPageLifecycle(
  target: Window,
  handlers: BrowserPageLifecycleHandlers
): () => void {
  const removeListeners = (): void => {
    target.removeEventListener("pagehide", onHide);
    target.removeEventListener("pageshow", onShow);
  };
  const onHide = (event: PageTransitionEvent): void => {
    if (event.persisted) handlers.suspend();
    else {
      removeListeners();
      handlers.dispose();
    }
  };
  const onShow = (event: PageTransitionEvent): void => {
    if (event.persisted) handlers.restore();
  };
  target.addEventListener("pagehide", onHide);
  target.addEventListener("pageshow", onShow);
  return removeListeners;
}
