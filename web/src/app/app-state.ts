import type { PresentationMode } from "../render/contracts/runtime.js";
import type { UiAction } from "../render/contracts/ui.js";

export type PresentationUiState =
  | { readonly type: "uninitialized" }
  | { readonly type: "initializing"; readonly requestId: number }
  | { readonly type: "ready"; readonly mode: PresentationMode }
  | {
      readonly type: "transitioning";
      readonly requestId: number;
      readonly from: PresentationMode | null;
      readonly to: PresentationMode;
      readonly phase: "requesting" | "stopping" | "starting";
    }
  | { readonly type: "failed"; readonly message: string }
  | { readonly type: "hidden" };

export interface AppModel {
  readonly status: string;
  readonly webXrAvailable: boolean;
  readonly phoneVrAvailable: boolean;
  readonly presentation: PresentationUiState;
  readonly nextRequestId: number;
}

export type AppMessage =
  | { readonly type: "initialize" }
  | {
      readonly type: "presentation-initialized";
      readonly requestId: number;
      readonly activeMode: PresentationMode | null;
      readonly webXrAvailable: boolean;
      readonly phoneVrAvailable: boolean;
      readonly status: string;
    }
  | { readonly type: "presentation-initialization-failed"; readonly requestId: number; readonly message: string }
  | { readonly type: "ui-action"; readonly action: UiAction }
  | {
      readonly type: "permission-completed";
      readonly requestId: number;
      readonly mode: "webxr" | "phone-vr";
      readonly ok: boolean;
      readonly message: string;
    }
  | {
      readonly type: "backend-transition-completed";
      readonly requestId: number;
      readonly requestedMode: PresentationMode;
      readonly activeMode: PresentationMode | null;
      readonly ok: boolean;
      readonly message: string;
      readonly successStatus: string;
    }
  | { readonly type: "backend-ended"; readonly mode: "webxr" | "phone-vr"; readonly message: string }
  | { readonly type: "page-hidden" };

export type AppEffect =
  | { readonly type: "initialize-presentation"; readonly requestId: number }
  | { readonly type: "request-permission"; readonly mode: "webxr" | "phone-vr"; readonly requestId: number }
  | { readonly type: "switch-backend"; readonly mode: PresentationMode; readonly requestId: number }
  | { readonly type: "cancel-pending-request"; readonly mode: "webxr" | "phone-vr" }
  | { readonly type: "recenter-tracking" }
  | { readonly type: "recenter-menu" }
  | { readonly type: "dispose-presentation" };

export interface AppTransition {
  readonly model: AppModel;
  readonly effects: readonly AppEffect[];
}

export function createInitialAppModel(): AppModel {
  return Object.freeze({
    status: "Screen renderer is initializing",
    webXrAvailable: false,
    phoneVrAvailable: false,
    presentation: Object.freeze({ type: "uninitialized" }),
    nextRequestId: 1
  });
}

export function updateApp(model: AppModel, message: AppMessage): AppTransition {
  if (model.presentation.type === "hidden") {
    if (message.type === "permission-completed" && message.ok) {
      return transition(model, [{ type: "cancel-pending-request", mode: message.mode }]);
    }
    return transition(model);
  }

  switch (message.type) {
    case "initialize": {
      if (model.presentation.type !== "uninitialized") return transition(model);
      const requestId = model.nextRequestId;
      return transition(
        withModel(model, {
          presentation: Object.freeze({ type: "initializing", requestId }),
          nextRequestId: requestId + 1
        }),
        [{ type: "initialize-presentation", requestId }]
      );
    }
    case "presentation-initialized": {
      if (model.presentation.type !== "initializing" || model.presentation.requestId !== message.requestId) {
        return transition(model);
      }
      if (message.activeMode === null) {
        const status = "Presentation initialization completed without an active backend";
        return transition(withModel(model, {
          presentation: Object.freeze({ type: "failed", message: status }),
          webXrAvailable: message.webXrAvailable,
          phoneVrAvailable: message.phoneVrAvailable,
          status
        }));
      }
      return transition(withModel(model, {
        presentation: Object.freeze({ type: "ready", mode: message.activeMode }),
        webXrAvailable: message.webXrAvailable,
        phoneVrAvailable: message.phoneVrAvailable,
        status: message.status
      }));
    }
    case "presentation-initialization-failed": {
      if (model.presentation.type !== "initializing" || model.presentation.requestId !== message.requestId) {
        return transition(model);
      }
      return transition(withModel(model, {
        presentation: Object.freeze({ type: "failed", message: message.message }),
        status: message.message
      }));
    }
    case "ui-action":
      return updateUiAction(model, message.action);
    case "permission-completed":
      return updatePermissionCompletion(model, message);
    case "backend-transition-completed":
      return updateBackendCompletion(model, message);
    case "backend-ended": {
      if (model.presentation.type !== "ready" || model.presentation.mode !== message.mode) return transition(model);
      return beginScreenRecovery(model, message.message, message.mode);
    }
    case "page-hidden": {
      return transition(withModel(model, {
        presentation: Object.freeze({ type: "hidden" }),
        status: "Page hidden"
      }), [
        { type: "cancel-pending-request", mode: "webxr" },
        { type: "cancel-pending-request", mode: "phone-vr" },
        { type: "dispose-presentation" }
      ]);
    }
    default:
      return assertNever(message);
  }
}

function updateUiAction(model: AppModel, action: UiAction): AppTransition {
  if (action.type === "focus" || action.type === "back" || action.type === "scroll") return transition(model);
  if (action.type === "recenter-menu") {
    if (isInVr(model.presentation)) {
      return transition(withModel(model, { status: "Menu placement updated" }), [{ type: "recenter-menu" }]);
    }
    return transition(model);
  }
  if (action.type !== "activate") {
    return transition(withModel(model, { status: `Action ${action.type} is unavailable in Boot` }));
  }
  if (action.controlId === "boot-enter-webxr") {
    return beginPermissionRequest(model, "webxr");
  }
  if (action.controlId === "boot-enter-phone-vr") {
    return beginPermissionRequest(model, "phone-vr");
  }
  if (action.controlId === "boot-exit-vr") {
    return beginBackendSwitch(model, "screen", "Ending presentation session");
  }
  if (action.controlId === "boot-recenter-phone-tracking") {
    if (model.presentation.type !== "ready" || model.presentation.mode !== "phone-vr") return transition(model);
    return transition(withModel(model, { status: "Phone VR tracking reference updated" }), [{ type: "recenter-tracking" }]);
  }
  if (action.controlId === "boot-recenter-menu") {
    if (!isInVr(model.presentation)) return transition(model);
    return transition(withModel(model, { status: "Menu placement updated" }), [{ type: "recenter-menu" }]);
  }
  return transition(withModel(model, { status: `Action ${action.controlId} is unavailable in Boot` }));
}

function beginPermissionRequest(model: AppModel, mode: "webxr" | "phone-vr"): AppTransition {
  const available = mode === "webxr" ? model.webXrAvailable : model.phoneVrAvailable;
  if (!available || !canRequestPresentation(model.presentation)) return transition(model);
  const requestId = model.nextRequestId;
  const from = model.presentation.type === "ready" ? model.presentation.mode : null;
  const status = mode === "webxr" ? "Waiting for WebXR permission" : "Waiting for phone orientation permission";
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", requestId, from, to: mode, phase: "requesting" }),
    nextRequestId: requestId + 1,
    status
  }), [{ type: "request-permission", mode, requestId }]);
}

function beginBackendSwitch(model: AppModel, mode: PresentationMode, status: string): AppTransition {
  if (model.presentation.type !== "ready" || model.presentation.mode === mode) return transition(model);
  const requestId = model.nextRequestId;
  const from = model.presentation.mode;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", requestId, from, to: mode, phase: "stopping" }),
    nextRequestId: requestId + 1,
    status
  }), [{ type: "switch-backend", mode, requestId }]);
}

function updatePermissionCompletion(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "permission-completed" }>
): AppTransition {
  const current = model.presentation;
  if (current.type !== "transitioning" || current.requestId !== message.requestId || current.to !== message.mode || current.phase !== "requesting") {
    return transition(model, message.ok ? [{ type: "cancel-pending-request", mode: message.mode }] : []);
  }
  if (!message.ok) {
    const status = message.message;
    const presentation = current.from === null
      ? Object.freeze({ type: "failed" as const, message: status })
      : Object.freeze({ type: "ready" as const, mode: current.from });
    return transition(withModel(model, { presentation, status }));
  }
  return transition(withModel(model, {
    presentation: Object.freeze({ ...current, phase: "starting" }),
    status: message.mode === "webxr" ? "Starting WebXR presentation" : "Starting Phone VR presentation"
  }), [{ type: "switch-backend", mode: message.mode, requestId: message.requestId }]);
}

function updateBackendCompletion(
  model: AppModel,
  message: Extract<AppMessage, { readonly type: "backend-transition-completed" }>
): AppTransition {
  const current = model.presentation;
  if (current.type !== "transitioning" || current.requestId !== message.requestId || current.to !== message.requestedMode) {
    return transition(model);
  }
  if (message.activeMode !== null) {
    const status = message.ok && message.activeMode === message.requestedMode
      ? message.successStatus
      : `${message.message}; ${labelForMode(message.activeMode)} is active`;
    return transition(withModel(model, {
      presentation: Object.freeze({ type: "ready", mode: message.activeMode }),
      status
    }));
  }
  const status = message.ok
    ? `Backend ${message.requestedMode} reported success without an active backend`
    : message.message;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "failed", message: status }),
    status
  }));
}

function beginScreenRecovery(
  model: AppModel,
  message: string,
  from: "webxr" | "phone-vr"
): AppTransition {
  const requestId = model.nextRequestId;
  return transition(withModel(model, {
    presentation: Object.freeze({ type: "transitioning", requestId, from, to: "screen", phase: "stopping" }),
    nextRequestId: requestId + 1,
    status: `${message}; restoring Screen`
  }), [{ type: "switch-backend", mode: "screen", requestId }]);
}

function canRequestPresentation(state: PresentationUiState): boolean {
  return state.type === "failed" || (state.type === "ready" && state.mode === "screen");
}

function isInVr(state: PresentationUiState): boolean {
  return state.type === "ready" && state.mode !== "screen";
}

function labelForMode(mode: PresentationMode): string {
  switch (mode) {
    case "screen": return "Screen";
    case "webxr": return "WebXR";
    case "phone-vr": return "Phone VR";
  }
}

function withModel(model: AppModel, changes: Partial<AppModel>): AppModel {
  return Object.freeze({ ...model, ...changes });
}

function transition(model: AppModel, effects: readonly AppEffect[] = []): AppTransition {
  return Object.freeze({ model, effects: Object.freeze([...effects]) });
}

function assertNever(value: never): never {
  throw new Error(`Unhandled application message: ${JSON.stringify(value)}`);
}
