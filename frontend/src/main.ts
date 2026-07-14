import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { availableMonitors, currentMonitor, getCurrentWindow, Window } from "@tauri-apps/api/window";
import * as PIXI from "pixi.js";
import type { Live2DModel as Live2DModelInstance } from "pixi-live2d-display/cubism4";
import "./style.css";
import { availableLanguages, setLanguage, t } from "./i18n";

declare global {
  interface Window {
    PIXI: typeof PIXI;
  }
}

window.PIXI = PIXI;

interface ModelInfo {
  manufacturer: string;
  model_name: string;
  file_path: string;
  size_bytes: number | null;
}

interface ConfigSnapshot {
  app: {
    name: string;
    theme: { mode: string };
    language: { ui: string; system_prompt: string; memory: string };
    avatar: {
      enabled: boolean;
      image_path: string;
      model_type: string;
      auto_select: boolean;
      idle_expression: string;
      thinking_expression: string;
      speaking_expression: string;
      error_expression: string;
      idle_motion: string;
      thinking_motion: string;
      speaking_motion: string;
    };
  };
  companion: {
    window: { x: number | null; y: number | null; width: number; height: number };
  };
  remote_api: { base_url: string; model: string; has_api_key: boolean };
  local_llm: {
    backend: string;
    base_url: string;
    model: string;
    enabled: boolean;
    available: boolean;
    auto_load: boolean;
    models_dir: string;
    load_command: string;
    unload_command: string;
    managed_process?: boolean;
  };
  persona_rewrite: { enabled: boolean; temperature: number };
  personality: { default_profile: string };
  initiative: { enabled: boolean; level: number; cooldown_ms: number };
  multimodal: {
    screenshot: { enabled: boolean; interval_ms: number; retention: number };
    comfyui: {
      enabled: boolean;
      available: boolean;
      base_url: string;
      root_dir: string;
      python_path: string;
      env_type: string;
      auto_start: boolean;
      launch_command: string;
      workflow_path: string;
      image_text_workflow_path: string;
      output_dir: string;
      default_mode: string;
      default_denoise: number;
      startup_timeout_ms: number;
      managed_process?: boolean;
    };
    vision: {
      enabled: boolean;
      available: boolean;
      base_url: string;
      model: string;
      has_api_key: boolean;
      api_key_env: string;
      system_prompt: string;
      default_prompt: string;
      max_image_bytes: number;
    };
  };
}

type AvatarConfig = ConfigSnapshot["app"]["avatar"];
type HistoryEntry = { role: string; content: string };

const chatHistory: HistoryEntry[] = [];

function createRequestId(prefix: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

interface ChatResponse {
  content: string;
  rewritten?: boolean;
  streamed?: boolean;
  generated_image?: boolean;
  image_prompt?: string;
  input_image_path?: string;
  mode?: string;
  images?: string[];
}

interface ChatStreamDelta {
  request_id: string;
  delta: string;
}

interface SystemPromptTemplate {
  id: string;
  title: string;
  description: string;
  content: string;
  default_content: string;
  overridden: boolean;
}

interface VisionResponse {
  content: string;
  model?: string;
  source?: string;
  image_path?: string;
}

interface InitiativeDecision {
  allowed: boolean;
  reasons: string[];
  score: number;
  idle_ms: number;
  min_idle_ms: number;
  cooldown_remaining_ms: number;
}

interface InitiativeResponse {
  allowed: boolean;
  content: string | null;
  decision: InitiativeDecision;
}

interface MemoryItem {
  id: string;
  kind: string;
  content: string;
  source: string;
  confidence: number;
  created_at: number;
  updated_at: number;
  last_used_at?: number | null;
  pinned: boolean;
  archived: boolean;
}

interface RoleProfile {
  schema_version: number;
  id: string;
  name: string;
  aliases: string[];
  identity: string;
  species: string;
  appearance: string;
  avatar?: {
    enabled: boolean;
    model_type: string;
    image_path: string;
  };
  personality: string;
  language_style: string;
  scenario: string;
  tone: string;
  initiative: number;
  humor: number;
  verbosity: string;
  pinned: boolean;
}

interface SetActiveRoleResponse {
  active_role: string;
  avatar?: AvatarConfig;
}

interface RoleStoragePaths {
  role: string;
  assets: string;
  memory: string;
}

type CompanionAvatarEventType = "expression" | "motion" | "speak_start" | "speak_stop" | "look_at" | "idle";

interface CompanionAvatarEventPayload {
  name?: string;
  index?: number;
  weight?: number;
  duration_ms?: number;
  transition_ms?: number;
  x?: number;
  y?: number;
}

interface CompanionAvatarEvent {
  type: CompanionAvatarEventType;
  data?: CompanionAvatarEventPayload;
}

interface Live2DMotionOption {
  group: string;
  index: number;
  label: string;
  file?: string;
  sound?: string;
}

interface Live2DManifestInfo {
  expressions: string[];
  motions: Live2DMotionOption[];
  hasAudio: boolean;
}

interface Live2DAutonomousSelection {
  expression?: string;
  motion_group?: string;
  motion_index?: number;
}

interface Live2DMotionState {
  shouldRequestIdleMotion?: () => boolean;
}

interface Live2DMotionManager {
  state?: Live2DMotionState;
  stopAllMotions?: () => void;
}

type Live2DModelWithMotionManager = Live2DModelInstance & {
  internalModel?: {
    motionManager?: Live2DMotionManager;
  };
};

let lastActivityReport = 0;

const iconPaths = {
  brush:
    "M12 20h9 M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z",
  check:
    "M20 6 9 17l-5-5",
  chevronsUp:
    "M7 11l5-5 5 5 M7 18l5-5 5 5",
  companion:
    "M4 10l2-5 4 3h4l4-3 2 5v5a6 6 0 0 1-6 6h-4a6 6 0 0 1-6-6v-5z M9 14h.01 M15 14h.01 M10 17c1.3.8 2.7.8 4 0",
  message:
    "M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4v8z",
  eye:
    "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  gear:
    "M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.5a2 2 0 0 1-1 1.8l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.8v-.5a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  image:
    "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M21 9V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v4 M3 15l5-5 4 4 2-2 7 7 M14 8h.01",
  memory:
    "M4 19.5A2.5 2.5 0 0 1 6.5 17H20 M4 4.5A2.5 2.5 0 0 1 6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15z M8 6h8 M8 10h8 M8 14h5",
  moon:
    "M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z",
  refresh:
    "M21 12a9 9 0 0 1-9 9 9.8 9.8 0 0 1-6.7-2.7 M3 12a9 9 0 0 1 15.7-6.3 M21 3v6h-6 M3 21v-6h6",
  send:
    "M22 2 11 13 M22 2l-7 20-4-9-9-4 20-7z",
  sparkles:
    "M12 3l1.5 4.5L18 9l-4.5 1.5L12 15l-1.5-4.5L6 9l4.5-1.5L12 3z M5 14l.8 2.2L8 17l-2.2.8L5 20l-.8-2.2L2 17l2.2-.8L5 14z M19 14l.8 2.2L22 17l-2.2.8L19 20l-.8-2.2L16 17l2.2-.8L19 14z",
  trash:
    "M3 6h18 M8 6V4h8v2 M19 6l-1 14H6L5 6 M10 11v6 M14 11v6",
  x:
    "M18 6 6 18 M6 6l12 12",
};

const COMPANION_DEFAULT_SIZE = { width: 260, height: 380 };
const COMPANION_MIN_SIZE = { width: 220, height: 320 };
const COMPANION_MAX_SIZE = { width: 420, height: 620 };

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string> = {},
  ...children: (string | Node)[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    element.setAttribute(key, value);
  }
  for (const child of children) {
    element.append(child);
  }
  return element;
}

function icon(name: keyof typeof iconPaths, size = 18): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(ns, "path");
  path.setAttribute("d", iconPaths[name]);
  svg.append(path);
  return svg;
}

function applyTheme(mode: string) {
  const resolved =
    mode === "system"
      ? window.matchMedia("(prefers-color-scheme:dark)").matches
        ? "dark"
        : "light"
      : mode;
  document.documentElement.setAttribute("data-theme", resolved);
}

function watchSystemTheme() {
  window.matchMedia("(prefers-color-scheme:dark)").addEventListener("change", () => {
    const select = document.getElementById("theme-select") as HTMLSelectElement | null;
    if (select?.value === "system") {
      applyTheme("system");
    }
  });
}

interface AvatarAdapter {
  type: string;
  mount(container: HTMLElement): void;
  unmount(): void;
  onEvent?(type: CompanionAvatarEventType, data?: CompanionAvatarEventPayload): void;
}

function isLocalFilesystemPath(path: string): boolean {
  return path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path);
}

function avatarResourceUrl(path: string): string {
  if (/^(https?:|asset:|file:)/.test(path)) {
    return path;
  }
  if (isLocalFilesystemPath(path)) {
    return convertFileSrc(path);
  }
  return `/${path.replace(/^\/+/, "")}`;
}

function cacheBustedResourceUrl(path: string): string {
  const url = avatarResourceUrl(path);
  const separator = url.includes("?") ? "&" : "?";
  return `${url}${separator}v=${Date.now()}`;
}

function live2DMotionManager(model: Live2DModelInstance | null): Live2DMotionManager | undefined {
  return (model as Live2DModelWithMotionManager | null)?.internalModel?.motionManager;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function createPlaceholderAvatarAdapter(imagePath: string): AvatarAdapter {
  let container: HTMLElement | null = null;
  let image: HTMLImageElement | null = null;
  const setState = (state: string) => {
    if (!container) return;
    container.dataset.avatarState = state;
  };
  return {
    type: "placeholder",
    mount(mountPoint: HTMLElement) {
      container = mountPoint;
      image = el("img", { src: avatarResourceUrl(imagePath), alt: "Hestia companion", draggable: "false" }) as HTMLImageElement;
      mountPoint.append(image);
      container.dataset.avatarAdapter = "placeholder";
      setState("idle");
    },
    unmount() {
      image?.remove();
      if (container) {
        delete container.dataset.avatarAdapter;
        delete container.dataset.avatarState;
        container.style.removeProperty("--avatar-look-x");
        container.style.removeProperty("--avatar-look-y");
      }
      image = null;
      container = null;
    },
    onEvent(type: CompanionAvatarEventType, data?: CompanionAvatarEventPayload) {
      if (!container) return;
      if (type === "look_at") {
        const x = Math.max(-1, Math.min(1, data?.x ?? 0));
        const y = Math.max(-1, Math.min(1, data?.y ?? 0));
        container.style.setProperty("--avatar-look-x", String(x));
        container.style.setProperty("--avatar-look-y", String(y));
        return;
      }
      if (type === "expression" || type === "motion") {
        setState(data?.name || type);
        return;
      }
      setState(type === "speak_start" ? "speaking" : type === "speak_stop" ? "idle" : type);
    },
  };
}

const LIVE2D_EXPRESSION_MAP: Record<string, string> = {
  angry: "Angry",
  blushing: "Blushing",
  confused: "Surprised",
  error: "Surprised",
  excited: "Surprised",
  happy: "Blushing",
  idle: "Normal",
  normal: "Normal",
  sad: "Sad",
  speaking: "Normal",
  surprised: "Surprised",
  thinking: "f01",
};

const LIVE2D_MOTION_MAP: Record<string, string> = {
  idle: "Idle",
  speaking: "Tap",
  thinking: "Flick",
  tap: "Tap",
};

function configuredLive2DExpression(name: string | undefined, avatar: AvatarConfig): string | undefined {
  const semantic = (name || "normal").toLowerCase();
  const configured: Record<string, string> = {
    normal: avatar.idle_expression,
    idle: avatar.idle_expression,
    speaking: avatar.speaking_expression,
    thinking: avatar.thinking_expression,
    error: avatar.error_expression,
    confused: avatar.error_expression,
  };
  return configured[semantic] || LIVE2D_EXPRESSION_MAP[semantic] || name;
}

function configuredLive2DMotion(name: string | undefined, avatar: AvatarConfig): string | undefined {
  const semantic = (name || "idle").toLowerCase();
  const configured: Record<string, string> = {
    idle: avatar.idle_motion,
    speaking: avatar.speaking_motion,
    tap: avatar.speaking_motion,
    thinking: avatar.thinking_motion,
  };
  return configured[semantic] || LIVE2D_MOTION_MAP[semantic] || name;
}

let cubismCoreScript: Promise<void> | null = null;
let live2dRuntime: Promise<typeof import("pixi-live2d-display/cubism4")> | null = null;
let loadedLive2dRuntime: typeof import("pixi-live2d-display/cubism4") | null = null;

function ensureCubismCore(): Promise<void> {
  const globalWindow = globalThis as typeof globalThis & { Live2DCubismCore?: unknown };
  if (globalWindow.Live2DCubismCore) {
    return Promise.resolve();
  }
  if (cubismCoreScript) {
    return cubismCoreScript;
  }
  cubismCoreScript = new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = "/vendor/live2dcubismcore.min.js";
    script.async = true;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error("failed to load Live2D Cubism Core"));
    document.head.append(script);
  });
  return cubismCoreScript;
}

async function ensureLive2DRuntime(): Promise<typeof import("pixi-live2d-display/cubism4")> {
  await ensureCubismCore();
  if (loadedLive2dRuntime) {
    return loadedLive2dRuntime;
  }
  if (!live2dRuntime) {
    live2dRuntime = import("pixi-live2d-display/cubism4").then((runtime) => {
      runtime.config.sound = false;
      runtime.SoundManager.volume = 0;
      runtime.SoundManager.destroy();
      loadedLive2dRuntime = runtime;
      return runtime;
    });
  }
  return live2dRuntime;
}

function createLive2DAvatarAdapter(avatarConfig: AvatarConfig): AvatarAdapter {
  const modelPath = avatarImagePath(avatarConfig);
  let container: HTMLElement | null = null;
  let app: PIXI.Application | null = null;
  let model: Live2DModelInstance | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let destroyed = false;
  const pendingEvents: CompanionAvatarEvent[] = [];

  const stopLive2DMotions = () => {
    live2DMotionManager(model)?.stopAllMotions?.();
    loadedLive2dRuntime?.SoundManager.destroy();
  };

  const fitModel = () => {
    if (!container || !model || !app) return;
    const width = Math.max(1, container.clientWidth);
    const height = Math.max(1, container.clientHeight);
    app.renderer.resize(width, height);
    const bounds = model.getLocalBounds();
    const modelWidth = Math.max(1, bounds.width);
    const modelHeight = Math.max(1, bounds.height);
    const scale = Math.min(width / modelWidth, height / modelHeight) * 0.94;
    model.scale.set(scale);
    model.x = width / 2;
    model.y = height * 0.53;
  };

  const applyEvent = (type: CompanionAvatarEventType, data?: CompanionAvatarEventPayload) => {
    if (!model || !container) return;
    if (type === "look_at") {
      const x = Math.max(-1, Math.min(1, data?.x ?? 0));
      const y = Math.max(-1, Math.min(1, data?.y ?? 0));
      const rect = container.getBoundingClientRect();
      model.focus(rect.width * (0.5 + x * 0.35), rect.height * (0.45 + y * 0.25));
      return;
    }

    if (type === "expression") {
      const expression = configuredLive2DExpression(data?.name, avatarConfig);
      if (expression) {
        model.expression(expression).catch(() => {});
      }
      return;
    }

    if (type === "motion") {
      const motion = configuredLive2DMotion(data?.name, avatarConfig);
      if (motion && loadedLive2dRuntime) {
        const index = Number.isFinite(data?.index) ? data?.index : undefined;
        stopLive2DMotions();
        model.motion(motion, index, loadedLive2dRuntime.MotionPriority.FORCE).catch(() => {});
        loadedLive2dRuntime.SoundManager.destroy();
      }
      return;
    }

    if (type === "speak_start") {
      const expression = configuredLive2DExpression("speaking", avatarConfig);
      if (expression) model.expression(expression).catch(() => {});
      if (loadedLive2dRuntime) {
        stopLive2DMotions();
        const motion = configuredLive2DMotion("speaking", avatarConfig);
        if (motion) model.motion(motion, undefined, loadedLive2dRuntime.MotionPriority.FORCE).catch(() => {});
        loadedLive2dRuntime.SoundManager.destroy();
      }
      return;
    }

    if (type === "speak_stop" || type === "idle") {
      const expression = configuredLive2DExpression("idle", avatarConfig);
      if (expression) model.expression(expression).catch(() => {});
      if (loadedLive2dRuntime) {
        stopLive2DMotions();
        const motion = configuredLive2DMotion("idle", avatarConfig);
        if (motion) model.motion(motion, undefined, loadedLive2dRuntime.MotionPriority.FORCE).catch(() => {});
        loadedLive2dRuntime.SoundManager.destroy();
      }
    }
  };

  const load = async (mountPoint: HTMLElement) => {
    try {
      const { Live2DModel, MotionPreloadStrategy, MotionPriority } = await ensureLive2DRuntime();
      if (destroyed || container !== mountPoint) return;
      const pixiOptions: PIXI.IApplicationOptions & {
        premultipliedAlpha: boolean;
        useContextAlpha: "notMultiplied";
      } = {
        antialias: true,
        autoDensity: true,
        backgroundAlpha: 0,
        clearBeforeRender: true,
        height: Math.max(1, mountPoint.clientHeight),
        premultipliedAlpha: false,
        preserveDrawingBuffer: false,
        resolution: window.devicePixelRatio || 1,
        useContextAlpha: "notMultiplied",
        width: Math.max(1, mountPoint.clientWidth),
      };
      app = new PIXI.Application(pixiOptions);
      app.ticker.add(() => {
        if (!app) return;
        const gl = (app.renderer as unknown as { gl?: WebGLRenderingContext }).gl;
        if (!gl) return;
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
      }, undefined, PIXI.UPDATE_PRIORITY.HIGH);
      app.view.classList.add("live2d-canvas");
      mountPoint.append(app.view);

      model = await Live2DModel.from(cacheBustedResourceUrl(modelPath), {
        autoInteract: false,
        motionPreload: MotionPreloadStrategy.IDLE,
      });
      if (destroyed || !app) {
        model.destroy();
        model = null;
        return;
      }

      const motionManager = live2DMotionManager(model);
      if (motionManager?.state?.shouldRequestIdleMotion) {
        motionManager.state.shouldRequestIdleMotion = () => false;
      }
      model.anchor.set(0.5, 0.5);
      app.stage.addChild(model);
      fitModel();
      const idleMotion = configuredLive2DMotion("idle", avatarConfig);
      if (idleMotion) model.motion(idleMotion, undefined, MotionPriority.FORCE).catch(() => {});
      loadedLive2dRuntime?.SoundManager.destroy();
      while (pendingEvents.length > 0) {
        const event = pendingEvents.shift();
        if (event) applyEvent(event.type, event.data);
      }
    } catch (error) {
      console.error("failed to load Live2D avatar", error);
      if (destroyed || container !== mountPoint) return;
      model?.destroy();
      app?.destroy(true, { children: true, texture: true, baseTexture: true });
      model = null;
      app = null;
      mountPoint.replaceChildren();
      if (mountPoint.isConnected) {
        createPlaceholderAvatarAdapter("companion-cat-placeholder.png").mount(mountPoint);
      }
    }
  };

  return {
    type: "live2d",
    mount(mountPoint: HTMLElement) {
      container = mountPoint;
      destroyed = false;
      container.dataset.avatarAdapter = "live2d";
      resizeObserver = new ResizeObserver(fitModel);
      resizeObserver.observe(mountPoint);
      void load(mountPoint);
    },
    unmount() {
      destroyed = true;
      resizeObserver?.disconnect();
      resizeObserver = null;
      pendingEvents.length = 0;
      stopLive2DMotions();
      model?.destroy();
      app?.destroy(true, { children: true, texture: true, baseTexture: true });
      if (container) {
        delete container.dataset.avatarAdapter;
        container.replaceChildren();
      }
      model = null;
      app = null;
      container = null;
    },
    onEvent(type: CompanionAvatarEventType, data?: CompanionAvatarEventPayload) {
      if (!model) {
        pendingEvents.push({ type, data });
        return;
      }
      applyEvent(type, data);
    },
  };
}

function createAvatarAdapter(avatarConfig: AvatarConfig): AvatarAdapter {
  const imagePath = avatarImagePath(avatarConfig);
  if (avatarConfig.model_type === "live2d" && imagePath.endsWith(".model3.json")) {
    return createLive2DAvatarAdapter(avatarConfig);
  }
  if (avatarConfig.model_type === "placeholder") {
    return createPlaceholderAvatarAdapter(imagePath || "companion-cat-placeholder.png");
  }
  return createPlaceholderAvatarAdapter("companion-cat-placeholder.png");
}

function motionLabel(group: string, index: number, file?: string): string {
  const name = file?.split(/[\\/]/).pop()?.replace(/\.motion3?\.json$/i, "");
  return `${group} #${index}${name ? ` (${name})` : ""}`;
}

async function loadLive2DManifestInfo(modelPath: string): Promise<Live2DManifestInfo> {
  const response = await fetch(cacheBustedResourceUrl(modelPath));
  if (!response.ok) {
    throw new Error(`failed to load ${modelPath}: ${response.status}`);
  }
  const manifest = await response.json();
  const fileReferences = manifest.FileReferences ?? manifest.fileReferences ?? manifest;
  const expressions = Array.isArray(fileReferences.Expressions)
    ? fileReferences.Expressions.map((expression: { Name?: string; name?: string; File?: string; file?: string }, index: number) => {
        const file = expression.File ?? expression.file;
        return expression.Name ?? expression.name ?? file?.split(/[\\/]/).pop()?.replace(/\.exp3\.json$/i, "") ?? `Expression ${index}`;
      })
    : [];
  const motionDefinitions = (fileReferences.Motions ?? fileReferences.motions ?? {}) as Record<string, unknown[]>;
  const motions: Live2DMotionOption[] = [];
  let hasAudio = false;
  for (const [group, definitions] of Object.entries(motionDefinitions)) {
    if (!Array.isArray(definitions)) continue;
    definitions.forEach((definition, index) => {
      if (!definition || typeof definition !== "object") return;
      const motion = definition as { File?: string; file?: string; Sound?: string; sound?: string };
      const file = motion.File ?? motion.file;
      const sound = motion.Sound ?? motion.sound;
      if (sound) hasAudio = true;
      motions.push({ group, index, label: motionLabel(group, index, file), file, sound });
    });
  }
  return { expressions, motions, hasAudio };
}

function extractJsonObject(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    const match = text.match(/\{[\s\S]*\}/);
    if (!match) return null;
    try {
      return JSON.parse(match[0]);
    } catch {
      return null;
    }
  }
}

function normalizeOptionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function buildLive2DSelectionPrompt(info: Live2DManifestInfo, userText: string, assistantText: string): string {
  const expressionList = info.expressions.map((name) => `- ${name}`).join("\n") || "- none";
  const motionList =
    info.motions.map((motion) => `- group=${motion.group}, index=${motion.index}, label=${motion.label}`).join("\n") ||
    "- none";
  return [
    "You choose a Live2D avatar reaction for a desktop companion.",
    "First decide whether each candidate is a real emotion/action word, not just an internal id, file code, or sequence name.",
    "Then choose at most one expression and at most one motion that fit the dialogue.",
    "Use only exact candidate values. If no candidate is meaningful or suitable, use null.",
    "Return strict JSON only, with this schema:",
    "{\"expression\": string|null, \"motion_group\": string|null, \"motion_index\": number|null}",
    "",
    "Available expression candidates:",
    expressionList,
    "",
    "Available motion candidates:",
    motionList,
    "",
    "Current user message:",
    userText,
    "",
    "Current assistant reply:",
    assistantText,
  ].join("\n");
}

function parseLive2DSelection(raw: string, info: Live2DManifestInfo): Live2DAutonomousSelection | null {
  const parsed = extractJsonObject(raw);
  if (!parsed || typeof parsed !== "object") return null;
  const data = parsed as Record<string, unknown>;
  const expression = normalizeOptionalString(data.expression);
  const motionGroup = normalizeOptionalString(data.motion_group);
  const motionIndex = typeof data.motion_index === "number" && Number.isInteger(data.motion_index) ? data.motion_index : undefined;
  const selection: Live2DAutonomousSelection = {};

  if (expression && info.expressions.includes(expression)) {
    selection.expression = expression;
  }
  if (
    motionGroup &&
    typeof motionIndex === "number" &&
    info.motions.some((motion) => motion.group === motionGroup && motion.index === motionIndex)
  ) {
    selection.motion_group = motionGroup;
    selection.motion_index = motionIndex;
  }
  return selection.expression || selection.motion_group ? selection : null;
}

async function selectLive2DAvatarReaction(
  avatarConfig: AvatarConfig,
  userText: string,
  assistantText: string,
): Promise<Live2DAutonomousSelection | null> {
  if (!avatarConfig.enabled || avatarConfig.model_type !== "live2d" || !avatarConfig.auto_select) return null;
  const modelPath = avatarImagePath(avatarConfig);
  if (!modelPath.endsWith(".model3.json")) return null;
  const info = await loadLive2DManifestInfo(modelPath);
  if (info.expressions.length === 0 && info.motions.length === 0) return null;
  const prompt = buildLive2DSelectionPrompt(info, userText, assistantText);
  const raw = await invoke<string>("send_chat_message", { message: prompt, history: [] });
  const response = JSON.parse(raw) as ChatResponse;
  return parseLive2DSelection(response.content || "", info);
}

async function emitAutonomousLive2DReaction(
  avatarConfig: AvatarConfig,
  userText: string,
  assistantText: string,
  emitAvatarEvent: (event: CompanionAvatarEvent) => void,
): Promise<void> {
  try {
    const selection = await selectLive2DAvatarReaction(avatarConfig, userText, assistantText);
    if (!selection) return;
    if (selection.expression) {
      emitAvatarEvent({ type: "expression", data: { name: selection.expression } });
    }
    if (selection.motion_group && typeof selection.motion_index === "number") {
      emitAvatarEvent({ type: "motion", data: { name: selection.motion_group, index: selection.motion_index } });
    }
  } catch (error) {
    console.warn("failed to select Live2D avatar reaction", error);
  }
}

async function emitCompanionAvatarEvent(event: CompanionAvatarEvent): Promise<void> {
  await invoke("set_companion_visible", { visible: true });
  let lastError: unknown = null;
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      await getCurrentWindow().emitTo("companion", "companion-avatar-event", event);
      return;
    } catch (error) {
      lastError = error;
      await delay(120);
    }
  }
  throw lastError;
}

function buildLive2DDebugPanel(modelPath: string): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel live2d-debug-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });
  const expressionSelect = el("select") as HTMLSelectElement;
  const motionSelect = el("select") as HTMLSelectElement;
  const expressionBtn = el("button", { class: "btn btn-primary", type: "button" }, "Expression");
  const motionBtn = el("button", { class: "btn btn-primary", type: "button" }, "Motion");
  const idleBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Idle");
  const speakStartBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Speak");
  const speakStopBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Stop");
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Close");

  const sendDebugEvent = async (event: CompanionAvatarEvent, label: string) => {
    try {
      await emitCompanionAvatarEvent(event);
      setStatus(status, true, `${label} sent.`);
    } catch (error) {
      setStatus(status, false, String(error));
    }
  };

  const setUnavailable = (message: string) => {
    expressionSelect.replaceChildren(option("", "No expressions", ""));
    motionSelect.replaceChildren(option("", "No motions", ""));
    expressionBtn.setAttribute("disabled", "true");
    motionBtn.setAttribute("disabled", "true");
    setStatus(status, false, message);
  };

  void loadLive2DManifestInfo(modelPath)
    .then((info) => {
      expressionSelect.replaceChildren();
      motionSelect.replaceChildren();
      if (info.expressions.length === 0) {
        expressionSelect.append(option("", "No expressions", ""));
        expressionBtn.setAttribute("disabled", "true");
      } else {
        info.expressions.forEach((name) => expressionSelect.append(option(name, name, expressionSelect.value)));
      }
      if (info.motions.length === 0) {
        motionSelect.append(option("", "No motions", ""));
        motionBtn.setAttribute("disabled", "true");
      } else {
        info.motions.forEach((motion) => {
          const item = option(`${motion.group}::${motion.index}`, motion.label, motionSelect.value);
          item.dataset.group = motion.group;
          item.dataset.index = String(motion.index);
          motionSelect.append(item);
        });
      }
      setStatus(status, true, info.hasAudio ? "Loaded. Motion audio is muted by Hestia." : "Loaded.");
    })
    .catch((error) => setUnavailable(String(error)));

  expressionBtn.addEventListener("click", () => {
    if (!expressionSelect.value) return;
    void sendDebugEvent({ type: "expression", data: { name: expressionSelect.value } }, `Expression ${expressionSelect.value}`);
  });
  motionBtn.addEventListener("click", () => {
    const selected = motionSelect.selectedOptions[0];
    const group = selected?.dataset.group;
    const index = Number(selected?.dataset.index);
    if (!group || !Number.isFinite(index)) return;
    void sendDebugEvent({ type: "motion", data: { name: group, index } }, `${group} #${index}`);
  });
  idleBtn.addEventListener("click", () => void sendDebugEvent({ type: "idle" }, "Idle"));
  speakStartBtn.addEventListener("click", () =>
    void sendDebugEvent({ type: "speak_start", data: { duration_ms: 3000 } }, "Speak"),
  );
  speakStopBtn.addEventListener("click", () => void sendDebugEvent({ type: "speak_stop" }, "Stop"));
  closeBtn.addEventListener("click", () => overlay.remove());
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) overlay.remove();
  });

  panel.append(
    el("h2", {}, "Live2D"),
    status,
    el("div", { class: "settings-section" }, fieldRow("Expression", expressionSelect), fieldRow("Motion", motionSelect)),
    el("div", { class: "settings-actions live2d-debug-actions" }, expressionBtn, motionBtn, idleBtn, speakStartBtn, speakStopBtn, closeBtn),
  );
  overlay.append(panel);
  return overlay;
}

function option(value: string, label: string, selectedValue: string): HTMLOptionElement {
  const item = el("option", { value }, label);
  if (value === selectedValue) {
    item.setAttribute("selected", "");
  }
  return item;
}

function fieldRow(label: string, control: HTMLElement, hint?: string): HTMLElement {
  const row = el("label", { class: "settings-row" });
  row.append(el("span", { class: "settings-label" }, label), control);
  if (hint) {
    row.append(el("span", { class: "hint" }, hint));
  }
  return row;
}

function statusLine(label: string, active: boolean, detail: string): HTMLElement {
  const line = el("div", { class: "status-line" });
  line.append(
    el("span", { class: active ? "status-dot online" : "status-dot" }),
    el("span", { class: "status-label" }, label),
    el("span", { class: "status-detail" }, detail),
  );
  return line;
}

function setStatus(status: HTMLElement, ok: boolean, text: string) {
  status.className = ok ? "settings-status ok" : "settings-status error";
  status.textContent = text;
  status.style.display = "block";
}

function renderMarkdown(markdown: string): HTMLElement {
  const root = el("div", { class: "markdown-preview" });
  const lines = markdown.split(/\r?\n/);
  let paragraph: string[] = [];
  let list: HTMLUListElement | null = null;
  let codeBlock: HTMLElement | null = null;

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      root.append(el("p", {}, paragraph.join(" ")));
      paragraph = [];
    }
  };
  const closeList = () => {
    list = null;
  };

  lines.forEach((line) => {
    if (line.startsWith("```")) {
      flushParagraph();
      closeList();
      if (codeBlock) {
        codeBlock = null;
      } else {
        codeBlock = el("pre", {}, el("code"));
        root.append(codeBlock);
      }
      return;
    }
    if (codeBlock) {
      const code = codeBlock.querySelector("code");
      if (code) code.textContent = `${code.textContent ?? ""}${line}\n`;
      return;
    }
    if (!line.trim()) {
      flushParagraph();
      closeList();
      return;
    }
    const heading = line.match(/^(#{1,3})\s+(.+)$/);
    if (heading) {
      flushParagraph();
      closeList();
      const level = heading[1].length === 1 ? "h4" : "h5";
      root.append(el(level, {}, heading[2]));
      return;
    }
    const bullet = line.match(/^[-*]\s+(.+)$/);
    if (bullet) {
      flushParagraph();
      if (!list) {
        list = el("ul");
        root.append(list);
      }
      list.append(el("li", {}, bullet[1]));
      return;
    }
    closeList();
    paragraph.push(line.trim());
  });
  flushParagraph();
  return root;
}

function buildSystemPromptSettingsPage(cfg: ConfigSnapshot, status: HTMLElement): HTMLElement {
  const page = el("div", { class: "settings-section prompt-settings" });
  const list = el("div", { class: "prompt-template-list" });
  const language = cfg.app.language.system_prompt || "en";

  const renderTemplateCard = (template: SystemPromptTemplate) => {
    let editing = false;
    let draft = template.content;
    const preview = el("div", { class: "prompt-template-preview" });
    const editor = el("textarea", { class: "prompt-template-editor", spellcheck: "false" }) as HTMLTextAreaElement;
    editor.value = draft;
    const editBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.prompt.edit"));
    const resetBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.prompt.restore"));
    const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("check", 16), t("settings.prompt.save"));
    const state = el("span", { class: template.overridden ? "prompt-template-state changed" : "prompt-template-state" }, template.overridden ? t("settings.prompt.overridden") : t("settings.prompt.default"));
    const body = el("div", { class: "prompt-template-body" });

    const renderBody = () => {
      body.replaceChildren();
      preview.replaceChildren(renderMarkdown(draft));
      if (editing) {
        editor.value = draft;
        body.append(editor);
        editBtn.textContent = t("settings.prompt.preview");
        saveBtn.style.display = "";
      } else {
        body.append(preview);
        editBtn.textContent = t("settings.prompt.edit");
        saveBtn.style.display = "none";
      }
    };

    editBtn.addEventListener("click", () => {
      if (editing) {
        draft = editor.value;
      }
      editing = !editing;
      renderBody();
    });
    saveBtn.addEventListener("click", async () => {
      draft = editor.value;
      try {
        await invoke("save_system_prompt_template", { language, id: template.id, content: draft });
        state.className = "prompt-template-state changed";
        state.textContent = t("settings.prompt.overridden");
        editing = false;
        renderBody();
        setStatus(status, true, t("settings.prompt.saved"));
      } catch (error) {
        setStatus(status, false, String(error));
      }
    });
    resetBtn.addEventListener("click", async () => {
      try {
        await invoke("reset_system_prompt_template", { language, id: template.id });
        draft = template.default_content;
        state.className = "prompt-template-state";
        state.textContent = t("settings.prompt.default");
        editing = false;
        renderBody();
        setStatus(status, true, t("settings.prompt.restored"));
      } catch (error) {
        setStatus(status, false, String(error));
      }
    });

    renderBody();
    return el(
      "article",
      { class: "prompt-template-card" },
      el("div", { class: "prompt-template-header" }, el("div", {}, el("h4", {}, template.title), el("p", {}, template.description)), state),
      body,
      el("div", { class: "settings-actions" }, editBtn, saveBtn, resetBtn),
    );
  };

  const loadTemplates = async () => {
    list.replaceChildren(el("div", { class: "settings-hint" }, t("settings.prompt.loading")));
    try {
      const templates = JSON.parse(await invoke<string>("list_system_prompt_templates", { language })) as SystemPromptTemplate[];
      list.replaceChildren(...templates.map(renderTemplateCard));
    } catch (error) {
      list.replaceChildren();
      setStatus(status, false, String(error));
    }
  };

  page.append(
    el("h3", {}, t("settings.module.prompts")),
    el("div", { class: "settings-hint" }, t("settings.prompt.languageHint", { language: t(`lang.${language}`) })),
    list,
  );
  void loadTemplates();
  return page;
}

function buildPersonaEditor(cfg: ConfigSnapshot): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel persona-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });
  const textarea = el("textarea", { class: "persona-editor", spellcheck: "false" }) as HTMLTextAreaElement;
  const profile = cfg.personality.default_profile;

  const loadBtn = el("button", { class: "btn btn-secondary", type: "button" }, icon("refresh", 16), "Load");
  const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("check", 16), "Save");
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Close");

  loadBtn.addEventListener("click", async () => {
    try {
      textarea.value = await invoke<string>("get_persona_content", { profile });
      setStatus(status, true, `Loaded ${profile}.json`);
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });

  saveBtn.addEventListener("click", async () => {
    try {
      await invoke("save_persona_content", { profile, content: textarea.value });
      setStatus(status, true, `Saved user override for ${profile}.json. The next message will use it.`);
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });

  closeBtn.addEventListener("click", () => overlay.remove());
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) {
      overlay.remove();
    }
  });

  panel.append(
    el("h2", {}, "Persona"),
    status,
    el("div", { class: "modal-scroll-body" }, textarea),
    el("div", { class: "settings-actions" }, loadBtn, saveBtn, closeBtn),
  );
  overlay.append(panel);

  invoke<string>("get_persona_content", { profile })
    .then((value) => {
      textarea.value = value;
    })
    .catch((error) => setStatus(status, false, String(error)));

  return overlay;
}

function roleIdFromName(value: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized || `role-${Date.now()}`;
}

function emptyRole(): RoleProfile {
  return {
    schema_version: 2,
    id: "",
    name: "",
    aliases: [],
    identity: "",
    species: "",
    appearance: "",
    avatar: {
      enabled: false,
      model_type: "placeholder",
      image_path: "",
    },
    personality: "",
    language_style: "",
    scenario: "",
    tone: "",
    initiative: 0.3,
    humor: 0.2,
    verbosity: "medium",
    pinned: false,
  };
}

function buildRoleManager(cfg: ConfigSnapshot, onRoleChange: (roleId: string) => void): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel role-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });
  const roleSelect = el("select") as HTMLSelectElement;
  const idInput = el("input", { type: "text", placeholder: "role-id" }) as HTMLInputElement;
  const nameInput = el("input", { type: "text", placeholder: "Name, e.g. Hestia" }) as HTMLInputElement;
  const aliasesInput = el("input", { type: "text", placeholder: "Aliases separated by comma" }) as HTMLInputElement;
  const identityInput = el("input", { type: "text", placeholder: t("role.identity") }) as HTMLInputElement;
  const speciesInput = el("input", { type: "text", placeholder: t("role.species") }) as HTMLInputElement;
  const appearanceInput = el("textarea", { class: "role-editor", rows: "3", placeholder: t("role.appearance") }) as HTMLTextAreaElement;
  const avatarEnabled = el("input", { type: "checkbox" }) as HTMLInputElement;
  const avatarType = el("select") as HTMLSelectElement;
  [
    ["placeholder", "Image"],
    ["live2d", "Live2D"],
    ["digital_human", "3D"],
  ].forEach(([value, label]) => avatarType.append(option(value, label, "placeholder")));
  const avatarPath = el("input", { type: "text", placeholder: t("role.avatarPlaceholder") }) as HTMLInputElement;
  const avatarBrowse = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  const avatarHint = el("div", { class: "settings-hint" }, t("role.avatarHint"));
  const personalityInput = el("textarea", { class: "role-editor", rows: "4", placeholder: t("role.personality") }) as HTMLTextAreaElement;
  const languageInput = el("textarea", { class: "role-editor", rows: "3", placeholder: t("role.language") }) as HTMLTextAreaElement;
  const scenarioInput = el("textarea", { class: "role-editor", rows: "3", placeholder: t("role.scenario") }) as HTMLTextAreaElement;
  const toneInput = el("input", { type: "text", placeholder: t("role.tone") }) as HTMLInputElement;
  const pinnedInput = el("input", { type: "checkbox" }) as HTMLInputElement;
  const pathHint = el("div", { class: "artifact-path" });
  let roles: RoleProfile[] = [];

  const roleFromForm = (): RoleProfile => {
    const name = nameInput.value.trim();
    const id = idInput.value.trim() || roleIdFromName(name);
    return {
      schema_version: 2,
      id,
      name: name || id,
      aliases: aliasesInput.value
        .split(",")
        .map((alias) => alias.trim())
        .filter(Boolean),
      identity: identityInput.value.trim(),
      species: speciesInput.value.trim(),
      appearance: appearanceInput.value.trim(),
      avatar: {
        enabled: avatarEnabled.checked,
        model_type: avatarType.value,
        image_path: avatarPath.value.trim(),
      },
      personality: personalityInput.value.trim(),
      language_style: languageInput.value.trim(),
      scenario: scenarioInput.value.trim(),
      tone: toneInput.value.trim(),
      initiative: 0.3,
      humor: 0.2,
      verbosity: "medium",
      pinned: pinnedInput.checked,
    };
  };

  const applyRole = async (role: RoleProfile) => {
    idInput.value = role.id || "";
    nameInput.value = role.name || "";
    aliasesInput.value = (role.aliases ?? []).join(", ");
    identityInput.value = role.identity || "";
    speciesInput.value = role.species || "";
    appearanceInput.value = role.appearance || "";
    avatarEnabled.checked = role.avatar?.enabled ?? Boolean(role.avatar?.image_path);
    avatarType.value = role.avatar?.model_type || "placeholder";
    avatarPath.value = role.avatar?.image_path || "";
    personalityInput.value = role.personality || "";
    languageInput.value = role.language_style || "";
    scenarioInput.value = role.scenario || "";
    toneInput.value = role.tone || "";
    pinnedInput.checked = Boolean(role.pinned);
    try {
      const paths = JSON.parse(await invoke<string>("role_storage_paths", { profile: role.id || "default" })) as RoleStoragePaths;
      pathHint.textContent = `Role: ${paths.role}\nAssets: ${paths.assets}\nMemory: ${paths.memory}`;
    } catch {
      pathHint.textContent = "";
    }
  };

  const loadRoles = async (selected = cfg.personality.default_profile) => {
    try {
      roles = JSON.parse(await invoke<string>("list_roles")) as RoleProfile[];
      roleSelect.replaceChildren();
      roles.forEach((role) => {
        const label = `${role.pinned ? "* " : ""}${role.name || role.id} (${role.id})`;
        roleSelect.append(option(role.id, label, selected));
      });
      const role = roles.find((item) => item.id === selected) ?? roles[0] ?? emptyRole();
      roleSelect.value = role.id;
      await applyRole(role);
    } catch (error) {
      setStatus(status, false, String(error));
    }
  };

  roleSelect.addEventListener("change", async () => {
    const role = roles.find((item) => item.id === roleSelect.value);
    if (role) await applyRole(role);
  });

  avatarBrowse.addEventListener("click", async () => {
    const role = roleFromForm();
    if (!role.id) {
      avatarHint.textContent = t("role.setIdFirst");
      return;
    }
    const selected =
      avatarType.value === "live2d"
        ? await open({ multiple: false, directory: true })
        : await open({
            multiple: false,
            filters:
              avatarType.value === "digital_human"
                ? [{ name: "3D Model", extensions: ["vrm", "glb", "gltf"] }]
                : [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
          });
    if (!selected || typeof selected !== "string") return;
    try {
      avatarPath.value = await invoke<string>("prepare_role_avatar_content", {
        profile: role.id,
        path: selected,
        modelType: avatarType.value,
      });
      avatarEnabled.checked = true;
      avatarHint.textContent = t("role.avatarCopied");
    } catch (error) {
      avatarHint.textContent = t("settings.avatarSelectionFailed", { error: String(error) });
    }
  });

  avatarType.addEventListener("change", () => {
    if (avatarType.value === "live2d") {
      avatarPath.placeholder = t("settings.live2dPlaceholder");
      avatarHint.textContent = t("role.live2dHint");
    } else if (avatarType.value === "digital_human") {
      avatarPath.placeholder = t("role.digitalHumanPlaceholder");
      avatarHint.textContent = t("role.digitalHumanHint");
    } else {
      avatarPath.placeholder = t("role.imagePlaceholder");
      avatarHint.textContent = t("role.imageHint");
    }
  });

  const newBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("role.new"));
  const generateBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("role.generate"));
  const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("check", 16), t("role.save"));
  const activateBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("role.activate"));
  const deleteBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("role.delete"));
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("role.close"));
  const activateRole = async (role: RoleProfile) => {
    const raw = await invoke<string>("set_active_role", { profile: role.id });
    const response = JSON.parse(raw) as SetActiveRoleResponse;
    cfg.personality.default_profile = response.active_role || role.id;
    if (response.avatar) {
      cfg.app.avatar = response.avatar;
    }
    onRoleChange(cfg.personality.default_profile);
  };

  newBtn.addEventListener("click", () => {
    void applyRole(emptyRole());
    setStatus(status, true, t("role.editingNew"));
  });
  generateBtn.addEventListener("click", async () => {
    generateBtn.setAttribute("disabled", "true");
    try {
      const generated = JSON.parse(
        await invoke<string>("generate_role_profile", {
          seed: roleFromForm(),
        }),
      ) as RoleProfile;
      await applyRole({ ...emptyRole(), ...generated });
      setStatus(status, true, t("role.generatedDraft"));
    } catch (error) {
      setStatus(status, false, String(error));
    } finally {
      generateBtn.removeAttribute("disabled");
    }
  });
  saveBtn.addEventListener("click", async () => {
    const role = roleFromForm();
    try {
      await invoke("save_persona_content", { profile: role.id, content: JSON.stringify(role, null, 2) });
      await activateRole(role);
      setStatus(status, true, t("role.savedActivated", { name: role.name }));
      await loadRoles(role.id);
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });
  activateBtn.addEventListener("click", async () => {
    const role = roleFromForm();
    try {
      await activateRole(role);
      setStatus(status, true, t("role.activated", { name: role.name || role.id }));
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });
  deleteBtn.addEventListener("click", async () => {
    const role = roleFromForm();
    const expected = `我确认删除${role.id}`;
    const confirmation = window.prompt(t("role.deletePrompt", { text: expected }));
    if (confirmation === null) return;
    try {
      await invoke("delete_role", { profile: role.id, confirmation });
      if (cfg.personality.default_profile === role.id) {
        cfg.personality.default_profile = "default";
        onRoleChange("default");
      }
      setStatus(status, true, t("role.deleted", { id: role.id }));
      await loadRoles(cfg.personality.default_profile);
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });
  closeBtn.addEventListener("click", () => overlay.remove());
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) overlay.remove();
  });

  panel.append(
    el("h2", {}, t("role.title")),
    status,
    el(
      "div",
      { class: "modal-scroll-body" },
      el(
        "div",
        { class: "settings-section" },
        fieldRow(t("role.select"), roleSelect),
        fieldRow(t("role.id"), idInput, t("role.idHint")),
        fieldRow(t("role.name"), nameInput, t("role.nameHint")),
        fieldRow(t("role.aliases"), aliasesInput),
        fieldRow(t("role.identity"), identityInput),
        fieldRow(t("role.species"), speciesInput),
        fieldRow(t("role.appearance"), appearanceInput),
        fieldRow(`${t("role.avatar")} ${t("settings.enable").toLowerCase()}`, avatarEnabled),
        fieldRow(`${t("role.avatar")} ${t("settings.type").toLowerCase()}`, avatarType),
        fieldRow(`${t("role.avatar")} ${t("settings.content").toLowerCase()}`, el("div", { class: "inline-controls" }, avatarPath, avatarBrowse)),
        avatarHint,
        fieldRow(t("role.personality"), personalityInput),
        fieldRow(t("role.language"), languageInput),
        fieldRow(t("role.scenario"), scenarioInput),
        fieldRow(t("role.tone"), toneInput),
        fieldRow(t("role.pinned"), pinnedInput),
        pathHint,
      ),
    ),
    el("div", { class: "settings-actions" }, newBtn, generateBtn, saveBtn, activateBtn, deleteBtn, closeBtn),
  );
  overlay.append(panel);
  void loadRoles();
  return overlay;
}

function buildMemoryPanel(cfg: ConfigSnapshot): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel memory-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });
  const queryInput = el("input", { type: "text", placeholder: "Search memories" }) as HTMLInputElement;
  const includeArchived = el("input", { type: "checkbox" }) as HTMLInputElement;
  const kindSelect = el("select") as HTMLSelectElement;
  ["fact", "preference", "project", "relationship", "note"].forEach((value) => {
    kindSelect.append(option(value, value, "note"));
  });
  const pinnedInput = el("input", { type: "checkbox" }) as HTMLInputElement;
  const contentInput = el("textarea", {
    class: "memory-editor",
    rows: "4",
    spellcheck: "true",
    placeholder: "Add a stable fact, preference, project note, or relationship detail.",
  }) as HTMLTextAreaElement;
  const list = el("div", { class: "memory-list" });
  const pathHint = el("div", { class: "artifact-path" });
  let memories: MemoryItem[] = [];
  let editingId: string | null = null;

  const resetEditor = () => {
    editingId = null;
    kindSelect.value = "note";
    pinnedInput.checked = false;
    contentInput.value = "";
  };

  const renderMemories = () => {
    list.replaceChildren();
    if (memories.length === 0) {
      list.append(el("div", { class: "memory-empty" }, "No memories."));
      return;
    }
    memories.forEach((memory) => {
      const meta = `${memory.kind}${memory.pinned ? " · pinned" : ""}${memory.archived ? " · archived" : ""}`;
      const editBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Edit");
      const pinBtn = el("button", { class: "btn btn-secondary", type: "button" }, memory.pinned ? "Unpin" : "Pin");
      const archiveBtn = el("button", { class: "btn btn-secondary", type: "button" }, memory.archived ? "Restore" : "Archive");
      const deleteBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Delete");

      editBtn.addEventListener("click", () => {
        editingId = memory.id;
        kindSelect.value = memory.kind;
        pinnedInput.checked = memory.pinned;
        contentInput.value = memory.content;
        contentInput.focus();
      });
      pinBtn.addEventListener("click", async () => {
        await updateMemory(memory.id, { pinned: !memory.pinned });
      });
      archiveBtn.addEventListener("click", async () => {
        await updateMemory(memory.id, { archived: !memory.archived });
      });
      deleteBtn.addEventListener("click", async () => {
        try {
          await invoke("delete_memory", { id: memory.id });
          setStatus(status, true, "Deleted memory.");
          await loadMemories();
        } catch (error) {
          setStatus(status, false, String(error));
        }
      });

      list.append(
        el(
          "article",
          { class: "memory-item" },
          el("div", { class: "memory-item-meta" }, meta),
          el("div", { class: "memory-item-content" }, memory.content),
          el("div", { class: "memory-item-actions" }, editBtn, pinBtn, archiveBtn, deleteBtn),
        ),
      );
    });
  };

  const loadMemories = async () => {
    try {
      const paths = JSON.parse(await invoke<string>("role_storage_paths", { profile: cfg.personality.default_profile })) as RoleStoragePaths;
      pathHint.textContent = `Active role: ${cfg.personality.default_profile}\nMemory: ${paths.memory}`;
      const raw = await invoke<string>("list_memories", {
        query: queryInput.value.trim() || null,
        includeArchived: includeArchived.checked,
      });
      memories = JSON.parse(raw) as MemoryItem[];
      renderMemories();
    } catch (error) {
      setStatus(status, false, String(error));
    }
  };

  const updateMemory = async (id: string, patch: Partial<MemoryItem>) => {
    try {
      await invoke("update_memory", { id, patch });
      setStatus(status, true, "Updated memory.");
      await loadMemories();
    } catch (error) {
      setStatus(status, false, String(error));
    }
  };

  const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("check", 16), "Save");
  const newBtn = el("button", { class: "btn btn-secondary", type: "button" }, "New");
  const reloadBtn = el("button", { class: "btn btn-secondary", type: "button" }, icon("refresh", 16), "Load");
  const compressBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Compress");
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Close");

  saveBtn.addEventListener("click", async () => {
    const content = contentInput.value.trim();
    if (!content) {
      setStatus(status, false, "Memory content is empty.");
      return;
    }
    try {
      if (editingId) {
        await invoke("update_memory", {
          id: editingId,
          patch: { kind: kindSelect.value, content, pinned: pinnedInput.checked },
        });
        setStatus(status, true, "Updated memory.");
      } else {
        await invoke("create_memory", {
          kind: kindSelect.value,
          content,
          source: "user",
          pinned: pinnedInput.checked,
        });
        setStatus(status, true, "Created memory.");
      }
      resetEditor();
      await loadMemories();
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });
  newBtn.addEventListener("click", resetEditor);
  reloadBtn.addEventListener("click", () => void loadMemories());
  compressBtn.addEventListener("click", async () => {
    compressBtn.setAttribute("disabled", "true");
    setStatus(status, true, "Compressing memories...");
    try {
      await invoke<string>("compress_memories");
      setStatus(status, true, "Compressed memories.");
      await loadMemories();
    } catch (error) {
      setStatus(status, false, String(error));
    } finally {
      compressBtn.removeAttribute("disabled");
    }
  });
  queryInput.addEventListener("input", () => void loadMemories());
  includeArchived.addEventListener("change", () => void loadMemories());
  closeBtn.addEventListener("click", () => overlay.remove());
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) overlay.remove();
  });

  panel.append(
    el("h2", {}, "Memory"),
    status,
    el(
      "div",
      { class: "modal-scroll-body" },
      pathHint,
      el(
        "div",
        { class: "settings-section" },
        fieldRow("Search", queryInput),
        fieldRow("Archived", includeArchived),
        fieldRow("Kind", kindSelect),
        fieldRow("Pinned", pinnedInput),
        fieldRow("Content", contentInput),
        el("div", { class: "settings-actions" }, newBtn, reloadBtn, compressBtn, saveBtn),
      ),
      list,
    ),
    el("div", { class: "settings-actions" }, closeBtn),
  );
  overlay.append(panel);
  void loadMemories();
  return overlay;
}

function buildImageTestPanel(): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });
  const promptInput = el("textarea", {
    class: "test-prompt",
    rows: "4",
    spellcheck: "true",
  }) as HTMLTextAreaElement;
  promptInput.value = "evening sunset scenery, a glass bottle with a galaxy inside, detailed, cinematic light";
  const negativeInput = el("input", {
    type: "text",
    value: "text, watermark",
  }) as HTMLInputElement;
  const modeSelect = el("select") as HTMLSelectElement;
  [
    ["text_to_image", t("image.mode.text")],
    ["image_text_to_image", t("image.mode.imageText")],
  ].forEach(([value, label]) => modeSelect.append(option(value, label, "text_to_image")));
  const inputImagePath = el("input", { type: "text", placeholder: t("image.inputPlaceholder") }) as HTMLInputElement;
  const browseInputImage = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  browseInputImage.addEventListener("click", async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    if (selected && typeof selected === "string") inputImagePath.value = selected;
  });
  const denoiseInput = el("input", {
    type: "range",
    min: "0",
    max: "1",
    step: "0.05",
    value: "0.65",
  }) as HTMLInputElement;
  const denoiseHint = el("span", { class: "hint" }, `${t("image.denoise")} 0.65`);
  denoiseInput.addEventListener("input", () => {
    denoiseHint.textContent = `${t("image.denoise")} ${Number(denoiseInput.value).toFixed(2)}`;
  });
  const result = el("div", { class: "image-result" });
  const runBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("image", 16), "Generate");
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Close");

  runBtn.addEventListener("click", async () => {
    runBtn.setAttribute("disabled", "true");
    result.innerHTML = "";
    setStatus(status, true, "Starting ComfyUI if needed...");
    try {
      const raw = await invoke<string>("generate_test_image", {
        prompt: promptInput.value.trim(),
        negativePrompt: negativeInput.value.trim() || null,
        inputImagePath: inputImagePath.value.trim() || null,
        imageMode: modeSelect.value,
        denoise: Number(denoiseInput.value),
      });
      const response = JSON.parse(raw) as { prompt_id: string; images: string[]; workflow_path: string; mode?: string };
      setStatus(status, true, `Completed ${response.prompt_id}`);
      for (const path of response.images) {
        const img = el("img", { alt: "Generated image" });
        invoke<string>("read_image_artifact", { path })
          .then((src) => img.setAttribute("src", src))
          .catch((error) => {
            img.replaceWith(el("div", { class: "artifact-path" }, `Preview failed: ${String(error)}`));
          });
        result.append(el("div", { class: "artifact-path" }, path), img);
      }
      if (response.images.length === 0) {
        result.append(el("div", { class: "artifact-path" }, "No image outputs found."));
      }
    } catch (error) {
      setStatus(status, false, String(error));
    } finally {
      runBtn.removeAttribute("disabled");
    }
  });

  closeBtn.addEventListener("click", () => overlay.remove());
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) {
      overlay.remove();
    }
  });

  panel.append(
    el("h2", {}, "Image Test"),
    status,
    el(
      "div",
      { class: "settings-section" },
      fieldRow("Prompt", promptInput),
      fieldRow("Negative", negativeInput),
      fieldRow(t("image.mode"), modeSelect),
      fieldRow(t("image.input"), el("div", { class: "inline-controls two" }, inputImagePath, browseInputImage)),
      fieldRow(t("image.denoise"), denoiseInput, t("image.denoiseHint")),
      denoiseHint,
    ),
    result,
    el("div", { class: "settings-actions" }, runBtn, closeBtn),
  );
  overlay.append(panel);
  return overlay;
}

const LOCAL_LLM_ONBOARDING_KEY = "hestia.localLlmOnboarding.v1";

function buildLocalLlmOnboarding(cfg: ConfigSnapshot, onDone: () => void): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel onboarding-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });
  const backendSelect = el("select") as HTMLSelectElement;
  const backendLabels: Record<string, string> = {
    llama_cpp: "llama.cpp",
    ollama: "Ollama",
    vllm: "vLLM",
  };
  ["llama_cpp", "ollama", "vllm"].forEach((value) => {
    backendSelect.append(option(value, backendLabels[value] ?? value, cfg.local_llm.backend));
  });
  const baseUrlInput = el("input", {
    type: "text",
    value: cfg.local_llm.base_url || "http://127.0.0.1:8080",
    placeholder: "http://127.0.0.1:8080",
  }) as HTMLInputElement;
  const modelsDirInput = el("input", {
    type: "text",
    value: cfg.local_llm.models_dir || "~/models",
    placeholder: "~/models",
  }) as HTMLInputElement;
  const modelInput = el("input", {
    type: "text",
    value: cfg.local_llm.model || "",
    placeholder: "qwen3-8b or path/to/model.gguf",
  }) as HTMLInputElement;
  const autoLoad = el("input", { type: "checkbox" }) as HTMLInputElement;
  autoLoad.checked = cfg.local_llm.auto_load;
  const browseModelsDir = el("button", { class: "btn btn-secondary", type: "button" }, "Browse");
  const browseModel = el("button", { class: "btn btn-secondary", type: "button" }, "Browse");
  const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, "Save");
  const skipBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Skip");

  backendSelect.addEventListener("change", () => {
    if (backendSelect.value === "llama_cpp") baseUrlInput.value = "http://127.0.0.1:8080";
    if (backendSelect.value === "ollama") baseUrlInput.value = "http://127.0.0.1:11434";
    if (backendSelect.value === "vllm") baseUrlInput.value = "http://127.0.0.1:8000";
  });
  browseModelsDir.addEventListener("click", async () => {
    const selected = await open({ multiple: false, directory: true });
    if (selected && typeof selected === "string") modelsDirInput.value = selected;
  });
  browseModel.addEventListener("click", async () => {
    const selected = await open({ multiple: false, filters: [{ name: "Models", extensions: ["gguf", "bin", "safetensors"] }] });
    if (selected && typeof selected === "string") modelInput.value = selected;
  });
  skipBtn.addEventListener("click", () => {
    localStorage.setItem(LOCAL_LLM_ONBOARDING_KEY, "skipped");
    overlay.remove();
  });
  saveBtn.addEventListener("click", async () => {
    saveBtn.setAttribute("disabled", "true");
    try {
      await invoke("update_settings", {
        updates: {
          local_llm_backend: backendSelect.value,
          local_llm_base_url: baseUrlInput.value.trim(),
          local_llm_models_dir: modelsDirInput.value.trim(),
          local_llm_model: modelInput.value.trim(),
          local_llm_enabled: true,
          local_llm_auto_load: autoLoad.checked,
          persona_rewrite_enabled: false,
        },
      });
      localStorage.setItem(LOCAL_LLM_ONBOARDING_KEY, "configured");
      overlay.remove();
      onDone();
    } catch (error) {
      setStatus(status, false, String(error));
    } finally {
      saveBtn.removeAttribute("disabled");
    }
  });

  panel.append(
    el("h2", {}, "Local LLM Setup"),
    status,
    el(
      "div",
      { class: "settings-section" },
      fieldRow("Backend", backendSelect),
      fieldRow("Base URL", baseUrlInput),
      fieldRow("Models dir", el("div", { class: "inline-controls two" }, modelsDirInput, browseModelsDir)),
      fieldRow("Model", el("div", { class: "inline-controls two" }, modelInput, browseModel)),
      fieldRow("Auto-load", autoLoad),
    ),
    el("div", { class: "settings-actions" }, skipBtn, saveBtn),
  );
  overlay.append(panel);
  return overlay;
}

function buildSettingsPanel(cfg: ConfigSnapshot, onClose: () => void): HTMLElement {
  const overlay = el("div", { class: "settings-overlay" });
  const panel = el("section", { class: "settings-panel", role: "dialog", "aria-modal": "true" });
  const status = el("div", { class: "settings-status", style: "display:none" });

  const apiKeyInput = el("input", {
    type: "password",
    placeholder: cfg.remote_api.has_api_key ? t("settings.existingKey") : "sk-...",
  }) as HTMLInputElement;
  const baseUrlInput = el("input", { type: "text", value: cfg.remote_api.base_url }) as HTMLInputElement;
  const modelInput = el("input", { type: "text", value: cfg.remote_api.model }) as HTMLInputElement;

  const avatarEnabled = el("input", { type: "checkbox" }) as HTMLInputElement;
  avatarEnabled.checked = cfg.app.avatar.enabled;
  const avatarType = el("select") as HTMLSelectElement;
  [
    ["placeholder", "Image"],
    ["live2d", "Live2D"],
    ["digital_human", "3D (future)"],
  ].forEach(([value, label]) => {
    avatarType.append(option(value, label, cfg.app.avatar.model_type));
  });
  const avatarPath = el("input", {
    type: "text",
    value: cfg.app.avatar.image_path || "",
    placeholder: "companion-cat-placeholder.png",
  }) as HTMLInputElement;
  const avatarHint = el("span", { class: "hint" }, t("settings.avatarHint"));
  const avatarAutoSelect = el("input", { type: "checkbox" }) as HTMLInputElement;
  avatarAutoSelect.checked = cfg.app.avatar.auto_select;
  const avatarIdleExpression = el("input", {
    type: "text",
    value: cfg.app.avatar.idle_expression,
    placeholder: "Normal",
  }) as HTMLInputElement;
  const avatarThinkingExpression = el("input", {
    type: "text",
    value: cfg.app.avatar.thinking_expression,
    placeholder: "f01",
  }) as HTMLInputElement;
  const avatarSpeakingExpression = el("input", {
    type: "text",
    value: cfg.app.avatar.speaking_expression,
    placeholder: "Normal",
  }) as HTMLInputElement;
  const avatarErrorExpression = el("input", {
    type: "text",
    value: cfg.app.avatar.error_expression,
    placeholder: "Surprised",
  }) as HTMLInputElement;
  const avatarIdleMotion = el("input", {
    type: "text",
    value: cfg.app.avatar.idle_motion,
    placeholder: "Idle",
  }) as HTMLInputElement;
  const avatarThinkingMotion = el("input", {
    type: "text",
    value: cfg.app.avatar.thinking_motion,
    placeholder: "Flick",
  }) as HTMLInputElement;
  const avatarSpeakingMotion = el("input", {
    type: "text",
    value: cfg.app.avatar.speaking_motion,
    placeholder: "Tap",
  }) as HTMLInputElement;

  const backendSelect = el("select") as HTMLSelectElement;
  const backendLabels: Record<string, string> = {
    llama_cpp: "llama.cpp",
    ollama: "Ollama",
    vllm: "vLLM",
  };
  ["llama_cpp", "ollama", "vllm"].forEach((value) => {
    backendSelect.append(option(value, backendLabels[value] ?? value, cfg.local_llm.backend));
  });

  const localUrlInput = el("input", {
    type: "text",
    value: cfg.local_llm.base_url,
    placeholder: "http://127.0.0.1:8080",
  }) as HTMLInputElement;
  const llmToggle = el("input", { type: "checkbox" }) as HTMLInputElement;
  llmToggle.checked = cfg.local_llm.enabled;
  const autoLoadToggle = el("input", { type: "checkbox" }) as HTMLInputElement;
  autoLoadToggle.checked = cfg.local_llm.auto_load;
  const localModelInput = el("input", {
    type: "text",
    value: cfg.local_llm.model || "",
    placeholder: "qwen3-8b or qwen/Qwen3-8B-Q4_K_M",
  }) as HTMLInputElement;
  const loadCmdInput = el("input", {
    type: "text",
    value: cfg.local_llm.load_command || "",
    placeholder: "Use built-in command",
  }) as HTMLInputElement;
  const unloadCmdInput = el("input", {
    type: "text",
    value: cfg.local_llm.unload_command || "",
    placeholder: "Use process SIGTERM",
  }) as HTMLInputElement;
  const modelHint = el("span", { class: "hint" }, t("settings.scanHint"));

  const comfyEnabled = el("input", { type: "checkbox" }) as HTMLInputElement;
  comfyEnabled.checked = cfg.multimodal.comfyui.enabled;
  const comfyAutoStart = el("input", { type: "checkbox" }) as HTMLInputElement;
  comfyAutoStart.checked = cfg.multimodal.comfyui.auto_start;
  const comfyBaseUrl = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.base_url,
    placeholder: "http://127.0.0.1:8188",
  }) as HTMLInputElement;
  const comfyRootDir = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.root_dir,
    placeholder: "~/models/ComfyUI or %USERPROFILE%\\models\\ComfyUI",
  }) as HTMLInputElement;
  const comfyPython = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.python_path,
    placeholder: "~/miniconda3/envs/comfyui/bin/python or %USERPROFILE%\\miniconda3\\envs\\comfyui\\python.exe",
  }) as HTMLInputElement;
  const comfyEnv = el("select") as HTMLSelectElement;
  ["conda", "venv", "system"].forEach((value) => comfyEnv.append(option(value, value, cfg.multimodal.comfyui.env_type)));
  const comfyWorkflow = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.workflow_path,
    placeholder: "assets/workflows/sdxl.json",
  }) as HTMLInputElement;
  const comfyImg2ImgWorkflow = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.image_text_workflow_path,
    placeholder: "assets/workflows/sdxl-image-text.json",
  }) as HTMLInputElement;
  const comfyOutputDir = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.output_dir,
    placeholder: "data/artifacts/images",
  }) as HTMLInputElement;
  const comfyDefaultMode = el("select") as HTMLSelectElement;
  [
    ["text_to_image", t("image.mode.text")],
    ["image_text_to_image", t("image.mode.imageText")],
  ].forEach(([value, label]) => comfyDefaultMode.append(option(value, label, cfg.multimodal.comfyui.default_mode)));
  const comfyDenoise = el("input", {
    type: "range",
    min: "0",
    max: "1",
    step: "0.05",
    value: String(cfg.multimodal.comfyui.default_denoise),
  }) as HTMLInputElement;
  const comfyDenoiseHint = el("span", { class: "hint" }, `${t("image.denoise")} ${Number(comfyDenoise.value).toFixed(2)}`);
  comfyDenoise.addEventListener("input", () => {
    comfyDenoiseHint.textContent = `${t("image.denoise")} ${Number(comfyDenoise.value).toFixed(2)}`;
  });
  const comfyLaunchCommand = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.launch_command,
    placeholder: "Use {python} main.py --listen {host} --port {port}",
  }) as HTMLInputElement;
  const visionEnabled = el("input", { type: "checkbox" }) as HTMLInputElement;
  visionEnabled.checked = cfg.multimodal.vision.enabled;
  const visionApiKey = el("input", {
    type: "password",
    placeholder: cfg.multimodal.vision.has_api_key ? t("settings.existingKey") : "MOONSHOT_API_KEY",
  }) as HTMLInputElement;
  const visionBaseUrl = el("input", {
    type: "text",
    value: cfg.multimodal.vision.base_url,
    placeholder: "https://api.moonshot.ai",
  }) as HTMLInputElement;
  const visionModel = el("input", {
    type: "text",
    value: cfg.multimodal.vision.model,
    placeholder: "kimi-k2.6",
  }) as HTMLInputElement;
  const visionDefaultPrompt = el("textarea", {
    rows: "3",
    spellcheck: "true",
  }) as HTMLTextAreaElement;
  visionDefaultPrompt.value = cfg.multimodal.vision.default_prompt;
  const visionMaxBytes = el("input", {
    type: "number",
    min: "1048576",
    step: "1048576",
    value: String(cfg.multimodal.vision.max_image_bytes),
  }) as HTMLInputElement;
  const initiativeEnabled = el("input", { type: "checkbox" }) as HTMLInputElement;
  initiativeEnabled.checked = cfg.initiative.enabled;
  const initiativeLevel = el("input", {
    type: "range",
    min: "0",
    max: "1",
    step: "0.1",
    value: String(cfg.initiative.level),
  }) as HTMLInputElement;
  const initiativeLevelHint = el("span", { class: "hint" }, `Level ${cfg.initiative.level.toFixed(1)}`);
  initiativeLevel.addEventListener("input", () => {
    initiativeLevelHint.textContent = `Level ${Number(initiativeLevel.value).toFixed(1)}`;
  });
  const initiativeCooldown = el("input", {
    type: "number",
    min: "30000",
    step: "30000",
    value: String(cfg.initiative.cooldown_ms),
  }) as HTMLInputElement;

  backendSelect.addEventListener("change", () => {
    if (backendSelect.value === "llama_cpp") {
      localUrlInput.value = "http://127.0.0.1:8080";
    } else if (backendSelect.value === "ollama") {
      localUrlInput.value = "http://127.0.0.1:11434";
    } else if (backendSelect.value === "vllm") {
      localUrlInput.value = "http://127.0.0.1:8000";
    }
  });

  const browseBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse")) as HTMLButtonElement;
  const scanBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.scan")) as HTMLButtonElement;

  browseBtn.addEventListener("click", async () => {
    const selected = await open({ multiple: false, filters: [{ name: "GGUF Models", extensions: ["gguf"] }] });
    if (selected && typeof selected === "string") {
      localModelInput.value = selected;
      modelHint.textContent = `Selected ${selected.split("/").pop() ?? selected}`;
    }
  });

  scanBtn.addEventListener("click", async () => {
    try {
      const models = JSON.parse(await invoke<string>("list_available_models")) as ModelInfo[];
      if (models.length === 0) {
        modelHint.textContent = t("settings.noModels");
        return;
      }
      modelHint.textContent = models
        .slice(0, 4)
        .map((model) => `${model.manufacturer}/${model.model_name}`)
        .join(", ");
      if (models.length > 4) {
        modelHint.textContent += `, ${t("settings.moreModels", { count: models.length - 4 })}`;
      }
    } catch (error) {
      modelHint.textContent = t("settings.scanFailed", { error: String(error) });
    }
  });

  const browseAvatar = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse")) as HTMLButtonElement;
  const live2dDebugBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.live2dTest")) as HTMLButtonElement;
  browseAvatar.addEventListener("click", async () => {
    const selected =
      avatarType.value === "live2d"
        ? await open({ multiple: false, directory: true })
        : await open({
            multiple: false,
            filters:
              avatarType.value === "digital_human"
                ? [{ name: "3D Model", extensions: ["vrm", "glb", "gltf"] }]
                : [{ name: "Image", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
          });
    if (selected && typeof selected === "string") {
      try {
        avatarPath.value = await invoke<string>("prepare_avatar_content", {
          path: selected,
          modelType: avatarType.value,
        });
        avatarHint.textContent =
          avatarType.value === "live2d"
            ? t("settings.avatarPreparedLive2d")
            : avatarType.value === "placeholder"
              ? t("settings.avatarPreparedImage")
              : t("settings.avatarPrepared3d");
      } catch (error) {
        avatarHint.textContent = t("settings.avatarSelectionFailed", { error: String(error) });
      }
    }
  });
  live2dDebugBtn.addEventListener("click", () => {
    const modelPath = avatarPath.value.trim();
    if (avatarType.value !== "live2d" || !modelPath.endsWith(".model3.json")) {
      avatarHint.textContent = t("settings.live2dNeedsContent");
      return;
    }
    document.body.append(buildLive2DDebugPanel(modelPath));
  });
  avatarType.addEventListener("change", () => {
    if (avatarType.value === "live2d") {
      avatarPath.placeholder = t("settings.live2dPlaceholder");
      avatarHint.textContent = t("settings.live2dHint");
    } else if (avatarType.value === "digital_human") {
      avatarPath.placeholder = t("settings.digitalHumanPlaceholder");
      avatarHint.textContent = t("settings.digitalHumanHint");
    } else {
      avatarPath.placeholder = "companion-cat-placeholder.png";
      avatarHint.textContent = t("settings.imageHint");
    }
  });

  const browseComfyRoot = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  browseComfyRoot.addEventListener("click", async () => {
    const selected = await open({ multiple: false, directory: true });
    if (selected && typeof selected === "string") comfyRootDir.value = selected;
  });

  const browseComfyPython = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  browseComfyPython.addEventListener("click", async () => {
    const selected = await open({ multiple: false });
    if (selected && typeof selected === "string") comfyPython.value = selected;
  });

  const browseWorkflow = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  browseWorkflow.addEventListener("click", async () => {
    const selected = await open({ multiple: false, filters: [{ name: "JSON Workflow", extensions: ["json"] }] });
    if (selected && typeof selected === "string") comfyWorkflow.value = selected;
  });

  const browseImg2ImgWorkflow = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  browseImg2ImgWorkflow.addEventListener("click", async () => {
    const selected = await open({ multiple: false, filters: [{ name: "JSON Workflow", extensions: ["json"] }] });
    if (selected && typeof selected === "string") comfyImg2ImgWorkflow.value = selected;
  });

  const browseOutputDir = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.browse"));
  browseOutputDir.addEventListener("click", async () => {
    const selected = await open({ multiple: false, directory: true });
    if (selected && typeof selected === "string") comfyOutputDir.value = selected;
  });

  const themeSelect = el("select") as HTMLSelectElement;
  ["system", "dark", "light"].forEach((value) => {
    themeSelect.append(option(value, t(`settings.theme.${value}`), cfg.app.theme.mode));
  });
  const uiLanguageSelect = el("select") as HTMLSelectElement;
  const systemPromptLanguageSelect = el("select") as HTMLSelectElement;
  const memoryLanguageSelect = el("select") as HTMLSelectElement;
  availableLanguages().forEach((value) => {
    uiLanguageSelect.append(option(value, t(`lang.${value}`), cfg.app.language.ui));
    systemPromptLanguageSelect.append(option(value, t(`lang.${value}`), cfg.app.language.system_prompt));
    memoryLanguageSelect.append(option(value, t(`lang.${value}`), cfg.app.language.memory));
  });
  uiLanguageSelect.addEventListener("change", () => {
    setStatus(status, true, t("settings.saveToApply"));
  });
  systemPromptLanguageSelect.addEventListener("change", () => {
    setStatus(status, true, t("settings.language.systemPromptHint"));
  });
  memoryLanguageSelect.addEventListener("change", () => {
    setStatus(status, true, t("settings.language.memoryHint"));
  });

  const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("check", 16), t("settings.save"));
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, t("settings.close"));

  saveBtn.addEventListener("click", async () => {
    const updates: Record<string, string | boolean | number> = {};
    const apiKey = apiKeyInput.value.trim();
    const localModel = localModelInput.value.trim();
    const localUrl = localUrlInput.value.trim();
    const loadCommand = loadCmdInput.value.trim();
    const unloadCommand = unloadCmdInput.value.trim();
    const avatarPathValue = avatarPath.value.trim();
    const idleExpression = avatarIdleExpression.value.trim();
    const thinkingExpression = avatarThinkingExpression.value.trim();
    const speakingExpression = avatarSpeakingExpression.value.trim();
    const errorExpression = avatarErrorExpression.value.trim();
    const idleMotion = avatarIdleMotion.value.trim();
    const thinkingMotion = avatarThinkingMotion.value.trim();
    const speakingMotion = avatarSpeakingMotion.value.trim();

    if (avatarEnabled.checked !== cfg.app.avatar.enabled) updates.avatar_enabled = avatarEnabled.checked;
    if (avatarType.value !== cfg.app.avatar.model_type) updates.avatar_model_type = avatarType.value;
    if (avatarPathValue !== cfg.app.avatar.image_path) updates.avatar_image_path = avatarPathValue;
    if (avatarAutoSelect.checked !== cfg.app.avatar.auto_select) updates.avatar_auto_select = avatarAutoSelect.checked;
    if (idleExpression !== cfg.app.avatar.idle_expression) updates.avatar_idle_expression = idleExpression;
    if (thinkingExpression !== cfg.app.avatar.thinking_expression) updates.avatar_thinking_expression = thinkingExpression;
    if (speakingExpression !== cfg.app.avatar.speaking_expression) updates.avatar_speaking_expression = speakingExpression;
    if (errorExpression !== cfg.app.avatar.error_expression) updates.avatar_error_expression = errorExpression;
    if (idleMotion !== cfg.app.avatar.idle_motion) updates.avatar_idle_motion = idleMotion;
    if (thinkingMotion !== cfg.app.avatar.thinking_motion) updates.avatar_thinking_motion = thinkingMotion;
    if (speakingMotion !== cfg.app.avatar.speaking_motion) updates.avatar_speaking_motion = speakingMotion;
    if (apiKey) updates.api_key = apiKey;
    if (baseUrlInput.value !== cfg.remote_api.base_url) updates.base_url = baseUrlInput.value;
    if (modelInput.value !== cfg.remote_api.model) updates.model = modelInput.value;
    if (themeSelect.value !== cfg.app.theme.mode) updates.theme_mode = themeSelect.value;
    if (uiLanguageSelect.value !== cfg.app.language.ui) updates.ui_language = uiLanguageSelect.value;
    if (systemPromptLanguageSelect.value !== cfg.app.language.system_prompt) {
      updates.system_prompt_language = systemPromptLanguageSelect.value;
    }
    if (memoryLanguageSelect.value !== cfg.app.language.memory) updates.memory_language = memoryLanguageSelect.value;
    if (backendSelect.value !== cfg.local_llm.backend) updates.local_llm_backend = backendSelect.value;
    if (localUrl !== cfg.local_llm.base_url) updates.local_llm_base_url = localUrl;
    if (localModel !== cfg.local_llm.model) updates.local_llm_model = localModel;
    if (llmToggle.checked !== cfg.local_llm.enabled) updates.local_llm_enabled = llmToggle.checked;
    if (autoLoadToggle.checked !== cfg.local_llm.auto_load) updates.local_llm_auto_load = autoLoadToggle.checked;
    if (cfg.persona_rewrite.enabled) updates.persona_rewrite_enabled = false;
    if (loadCommand !== (cfg.local_llm.load_command || "")) updates.local_llm_load_command = loadCommand;
    if (unloadCommand !== (cfg.local_llm.unload_command || "")) updates.local_llm_unload_command = unloadCommand;
    if (comfyEnabled.checked !== cfg.multimodal.comfyui.enabled) updates.comfyui_enabled = comfyEnabled.checked;
    if (comfyBaseUrl.value.trim() !== cfg.multimodal.comfyui.base_url) updates.comfyui_base_url = comfyBaseUrl.value.trim();
    if (comfyRootDir.value.trim() !== cfg.multimodal.comfyui.root_dir) updates.comfyui_root_dir = comfyRootDir.value.trim();
    if (comfyPython.value.trim() !== cfg.multimodal.comfyui.python_path) {
      updates.comfyui_python_path = comfyPython.value.trim();
    }
    if (comfyEnv.value !== cfg.multimodal.comfyui.env_type) updates.comfyui_env_type = comfyEnv.value;
    if (comfyAutoStart.checked !== cfg.multimodal.comfyui.auto_start) {
      updates.comfyui_auto_start = comfyAutoStart.checked;
    }
    if (comfyLaunchCommand.value.trim() !== (cfg.multimodal.comfyui.launch_command || "")) {
      updates.comfyui_launch_command = comfyLaunchCommand.value.trim();
    }
    if (comfyWorkflow.value.trim() !== cfg.multimodal.comfyui.workflow_path) {
      updates.comfyui_workflow_path = comfyWorkflow.value.trim();
    }
    if (comfyImg2ImgWorkflow.value.trim() !== cfg.multimodal.comfyui.image_text_workflow_path) {
      updates.comfyui_image_text_workflow_path = comfyImg2ImgWorkflow.value.trim();
    }
    if (comfyOutputDir.value.trim() !== cfg.multimodal.comfyui.output_dir) {
      updates.comfyui_output_dir = comfyOutputDir.value.trim();
    }
    if (comfyDefaultMode.value !== cfg.multimodal.comfyui.default_mode) {
      updates.comfyui_default_mode = comfyDefaultMode.value;
    }
    const defaultDenoise = Number(comfyDenoise.value);
    if (Number.isFinite(defaultDenoise) && defaultDenoise !== cfg.multimodal.comfyui.default_denoise) {
      updates.comfyui_default_denoise = defaultDenoise;
    }
    if (visionEnabled.checked !== cfg.multimodal.vision.enabled) updates.vision_enabled = visionEnabled.checked;
    if (visionApiKey.value.trim()) updates.vision_api_key = visionApiKey.value.trim();
    if (visionBaseUrl.value.trim() !== cfg.multimodal.vision.base_url) updates.vision_base_url = visionBaseUrl.value.trim();
    if (visionModel.value.trim() !== cfg.multimodal.vision.model) updates.vision_model = visionModel.value.trim();
    if (visionDefaultPrompt.value.trim() !== cfg.multimodal.vision.default_prompt) {
      updates.vision_default_prompt = visionDefaultPrompt.value.trim();
    }
    const maxBytes = Number(visionMaxBytes.value);
    if (Number.isFinite(maxBytes) && maxBytes > 0 && maxBytes !== cfg.multimodal.vision.max_image_bytes) {
      updates.vision_max_image_bytes = maxBytes;
    }
    if (initiativeEnabled.checked !== cfg.initiative.enabled) updates.initiative_enabled = initiativeEnabled.checked;
    const initiativeLevelValue = Number(initiativeLevel.value);
    if (Number.isFinite(initiativeLevelValue) && initiativeLevelValue !== cfg.initiative.level) {
      updates.initiative_level = initiativeLevelValue;
    }
    const cooldownMs = Number(initiativeCooldown.value);
    if (Number.isFinite(cooldownMs) && cooldownMs > 0 && cooldownMs !== cfg.initiative.cooldown_ms) {
      updates.initiative_cooldown_ms = cooldownMs;
    }

    if (Object.keys(updates).length === 0) {
      setStatus(status, true, t("settings.noChanges"));
      return;
    }

    try {
      await invoke("update_settings", { updates });
      Object.assign(cfg, await loadConfig());
      if (updates.theme_mode) {
        applyTheme(String(updates.theme_mode));
      }
      if (updates.ui_language) {
        setLanguage(String(updates.ui_language));
      }
      setStatus(status, true, t("settings.saved"));
    } catch (error) {
      setStatus(status, false, String(error));
    }
  });

  closeBtn.addEventListener("click", () => {
    overlay.remove();
    onClose();
  });
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) {
      overlay.remove();
      onClose();
    }
  });

  const modelControls = el("div", { class: "inline-controls" }, localModelInput, browseBtn, scanBtn);
  const avatarPathControls = el("div", { class: "inline-controls" }, avatarPath, browseAvatar, live2dDebugBtn);
  const comfyRootControls = el("div", { class: "inline-controls two" }, comfyRootDir, browseComfyRoot);
  const comfyPythonControls = el("div", { class: "inline-controls two" }, comfyPython, browseComfyPython);
  const comfyWorkflowControls = el("div", { class: "inline-controls two" }, comfyWorkflow, browseWorkflow);
  const comfyImg2ImgWorkflowControls = el("div", { class: "inline-controls two" }, comfyImg2ImgWorkflow, browseImg2ImgWorkflow);
  const comfyOutputControls = el("div", { class: "inline-controls two" }, comfyOutputDir, browseOutputDir);

  const pages: Array<[string, HTMLElement]> = [
    [
      "general",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.general")),
        fieldRow(t("settings.language.ui"), uiLanguageSelect),
        fieldRow(t("settings.language.systemPrompt"), systemPromptLanguageSelect, t("settings.language.systemPromptHint")),
        fieldRow(t("settings.language.memory"), memoryLanguageSelect, t("settings.language.memoryHint")),
      ),
    ],
    [
      "avatar",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.avatar")),
        fieldRow(t("settings.enable"), avatarEnabled),
        fieldRow(t("settings.type"), avatarType),
        fieldRow(t("settings.content"), avatarPathControls),
        avatarHint,
        fieldRow(t("settings.autoMood"), avatarAutoSelect, t("settings.autoMoodHint")),
        fieldRow(t("settings.idleExpression"), avatarIdleExpression),
        fieldRow(t("settings.thinkingExpression"), avatarThinkingExpression),
        fieldRow(t("settings.speakingExpression"), avatarSpeakingExpression),
        fieldRow(t("settings.errorExpression"), avatarErrorExpression),
        fieldRow(t("settings.idleMotion"), avatarIdleMotion),
        fieldRow(t("settings.thinkingMotion"), avatarThinkingMotion),
        fieldRow(t("settings.speakingMotion"), avatarSpeakingMotion),
      ),
    ],
    ["prompts", buildSystemPromptSettingsPage(cfg, status)],
    [
      "remote",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.remote")),
        fieldRow(t("settings.apiKey"), apiKeyInput, t("settings.keepKey")),
        fieldRow(t("settings.baseUrl"), baseUrlInput),
        fieldRow(t("settings.model"), modelInput),
      ),
    ],
    [
      "local",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.local")),
        fieldRow(t("settings.backend"), backendSelect),
        fieldRow(t("settings.baseUrl"), localUrlInput),
        fieldRow(t("settings.enable"), llmToggle),
        fieldRow(t("settings.autoLoad"), autoLoadToggle, t("settings.autoLoadHint")),
        fieldRow(t("settings.model"), modelControls, t("settings.modelHint")),
        modelHint,
        fieldRow(t("settings.loadCommand"), loadCmdInput, t("settings.loadCommandHint")),
        fieldRow(t("settings.unloadCommand"), unloadCmdInput, t("settings.unloadCommandHint")),
      ),
    ],
    [
      "comfyui",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.comfyui")),
        fieldRow(t("settings.enable"), comfyEnabled),
        fieldRow(t("settings.onDemandStart"), comfyAutoStart, t("settings.onDemandStartHint")),
        fieldRow(t("settings.baseUrl"), comfyBaseUrl),
        fieldRow(t("settings.rootDir"), comfyRootControls),
        fieldRow(t("settings.python"), comfyPythonControls, t("settings.pythonHint")),
        fieldRow(t("settings.envType"), comfyEnv),
        fieldRow(t("settings.textWorkflow"), comfyWorkflowControls),
        fieldRow(t("settings.imageWorkflow"), comfyImg2ImgWorkflowControls, t("settings.imageWorkflowHint")),
        fieldRow(t("settings.defaultImageMode"), comfyDefaultMode),
        fieldRow(t("settings.defaultDenoise"), comfyDenoise, t("settings.defaultDenoiseHint")),
        comfyDenoiseHint,
        fieldRow(t("settings.outputDir"), comfyOutputControls),
        fieldRow(t("settings.launchCommand"), comfyLaunchCommand, t("settings.launchCommandHint")),
      ),
    ],
    [
      "vision",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.vision")),
        fieldRow(t("settings.enable"), visionEnabled),
        fieldRow(t("settings.apiKey"), visionApiKey, `${t("settings.keepKey")} Env fallback: ${cfg.multimodal.vision.api_key_env}.`),
        fieldRow(t("settings.baseUrl"), visionBaseUrl),
        fieldRow(t("settings.model"), visionModel),
        fieldRow(t("settings.defaultPrompt"), visionDefaultPrompt),
        fieldRow(t("settings.maxBytes"), visionMaxBytes, t("settings.maxBytesHint")),
      ),
    ],
    [
      "initiative",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.initiative")),
        fieldRow(t("settings.enable"), initiativeEnabled),
        fieldRow(t("settings.level"), initiativeLevel, t("settings.levelHint")),
        initiativeLevelHint,
        fieldRow(t("settings.cooldownMs"), initiativeCooldown),
      ),
    ],
    [
      "appearance",
      el(
        "div",
        { class: "settings-section" },
        el("h3", {}, t("settings.module.appearance")),
        fieldRow(t("settings.theme"), themeSelect),
      ),
    ],
  ];
  const nav = el("nav", { class: "settings-nav", "aria-label": "Settings modules" });
  const content = el("div", { class: "settings-content" });
  const selectPage = (key: string) => {
    Array.from(nav.querySelectorAll("button")).forEach((button) => {
      button.classList.toggle("active", button.getAttribute("data-page") === key);
    });
    content.replaceChildren(pages.find(([pageKey]) => pageKey === key)?.[1] ?? pages[0][1]);
  };
  pages.forEach(([key]) => {
    const button = el("button", { class: "settings-nav-btn", type: "button", "data-page": key }, t(`settings.module.${key}`));
    button.addEventListener("click", () => selectPage(key));
    nav.append(button);
  });
  selectPage("general");

  panel.append(
    el("div", { class: "settings-header" }, el("h2", {}, t("settings.title")), el("div", { class: "settings-actions" }, saveBtn, closeBtn)),
    status,
    el("div", { class: "settings-shell" }, nav, content),
  );
  overlay.append(panel);
  return overlay;
}

function fallbackConfig(): ConfigSnapshot {
  return {
    app: {
      name: "Hestia",
      theme: { mode: "system" },
      language: { ui: "en", system_prompt: "en", memory: "en" },
      avatar: {
        enabled: true,
        image_path: "companion-cat-placeholder.png",
        model_type: "placeholder",
        auto_select: true,
        idle_expression: "Normal",
        thinking_expression: "f01",
        speaking_expression: "Normal",
        error_expression: "Surprised",
        idle_motion: "Idle",
        thinking_motion: "Flick",
        speaking_motion: "Tap",
      },
    },
    companion: { window: { x: null, y: null, width: COMPANION_DEFAULT_SIZE.width, height: COMPANION_DEFAULT_SIZE.height } },
    remote_api: { base_url: "https://api.deepseek.com", model: "deepseek-chat", has_api_key: false },
    local_llm: {
      backend: "llama_cpp",
      base_url: "http://127.0.0.1:8080",
      model: "qwen3-8b",
      enabled: false,
      available: false,
      auto_load: false,
      models_dir: "",
      load_command: "",
      unload_command: "",
    },
    persona_rewrite: { enabled: false, temperature: 0.7 },
    personality: { default_profile: "default" },
    initiative: { enabled: false, level: 0.3, cooldown_ms: 600000 },
    multimodal: {
      screenshot: { enabled: false, interval_ms: 5000, retention: 20 },
      comfyui: {
        enabled: false,
        available: false,
        base_url: "http://127.0.0.1:8188",
        root_dir: "",
        python_path: "",
        env_type: "venv",
        auto_start: false,
        launch_command: "",
        workflow_path: "assets/workflows/sdxl.json",
        image_text_workflow_path: "assets/workflows/sdxl.json",
        output_dir: "data/artifacts/images",
        default_mode: "text_to_image",
        default_denoise: 0.65,
        startup_timeout_ms: 20000,
      },
      vision: {
        enabled: false,
        available: false,
        base_url: "https://api.moonshot.ai",
        model: "kimi-k2.6",
        has_api_key: false,
        api_key_env: "MOONSHOT_API_KEY",
        system_prompt: "",
        default_prompt: "请简要描述这张图片, 重点说明对桌宠对话有用的内容. 如果图中有文字, 摘录关键文字.",
        max_image_bytes: 20971520,
      },
    },
  };
}

async function loadConfig(): Promise<ConfigSnapshot> {
  try {
    const cfg = JSON.parse(await invoke("get_config_snapshot")) as ConfigSnapshot;
    setLanguage(cfg.app.language?.ui ?? "en");
    return cfg;
  } catch {
    const cfg = fallbackConfig();
    setLanguage(cfg.app.language.ui);
    return cfg;
  }
}

function avatarImagePath(avatar: AvatarConfig): string {
  return avatar.image_path || "companion-cat-placeholder.png";
}

function reportUserActivity() {
  const now = Date.now();
  if (now - lastActivityReport < 5000) return;
  lastActivityReport = now;
  invoke("record_user_activity").catch(() => {});
}

async function buildApp() {
  let cfg = await loadConfig();
  applyTheme(cfg.app.theme.mode);
  setLanguage(cfg.app.language.ui);
  watchSystemTheme();

  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";

  const layout = el("div", { class: "app-layout" });
  const sidebar = el("aside", { class: "sidebar" });
  const avatar = el("div", { class: "avatar-container", id: "avatar-container" });
  let sidebarAvatarAdapter: AvatarAdapter | null = null;
  const mountSidebarAvatar = (avatarConfig: AvatarConfig) => {
    sidebarAvatarAdapter?.unmount();
    sidebarAvatarAdapter = null;
    avatar.replaceChildren();
    if (!avatarConfig.enabled) return;
    sidebarAvatarAdapter = createAvatarAdapter(avatarConfig);
    sidebarAvatarAdapter.mount(avatar);
  };
  mountSidebarAvatar(cfg.app.avatar);
  listen<AvatarConfig>("avatar-config-changed", (event) => {
    cfg.app.avatar = event.payload;
    mountSidebarAvatar(event.payload);
  }).catch(() => {});

  const themeSelect = el("select", { id: "theme-select", "aria-label": "Theme" }) as HTMLSelectElement;
  ["system", "dark", "light"].forEach((value) => {
    themeSelect.append(option(value, value.charAt(0).toUpperCase() + value.slice(1), cfg.app.theme.mode));
  });
  themeSelect.addEventListener("change", () => applyTheme(themeSelect.value));

  const roleBtn = el("button", { class: "sidebar-btn", type: "button" }, icon("brush", 16), "Roles");
  roleBtn.addEventListener("click", () =>
    document.body.append(
      buildRoleManager(cfg, (roleId) => {
        cfg.personality.default_profile = roleId;
      }),
    ),
  );

  const memoryBtn = el("button", { class: "sidebar-btn", type: "button" }, icon("memory", 16), "Memory");
  memoryBtn.addEventListener("click", () => document.body.append(buildMemoryPanel(cfg)));

  const imageBtn = el("button", { class: "sidebar-btn", type: "button" }, icon("image", 16), "Image Test");
  imageBtn.addEventListener("click", () => document.body.append(buildImageTestPanel()));

  const companionBtn = el("button", { class: "sidebar-btn", type: "button" }, icon("companion", 16), "Show Companion");
  let companionVisible = false;
  const setCompanionVisibleState = (visible: boolean) => {
    companionVisible = visible;
    companionBtn.replaceChildren(icon("companion", 16), companionVisible ? "Hide Companion" : "Show Companion");
  };
  companionBtn.addEventListener("click", async () => {
    companionBtn.setAttribute("disabled", "true");
    const nextVisible = !companionVisible;
    try {
      await invoke("set_companion_visible", { visible: nextVisible });
      setCompanionVisibleState(nextVisible);
    } catch (error) {
      addMessage(document.getElementById("chat-messages") ?? document.body, "error", String(error));
    } finally {
      companionBtn.removeAttribute("disabled");
    }
  });
  listen<boolean>("companion-visible-changed", (event) => {
    setCompanionVisibleState(event.payload);
  }).catch(() => {});

  const settingsBtn = el("button", { class: "sidebar-btn", type: "button" }, icon("gear", 16), "Settings");

  sidebar.append(
    avatar,
    el("div", { class: "companion-name" }, cfg.app.name),
    el(
      "div",
      { class: "companion-status" },
      statusLine("Remote API", cfg.remote_api.has_api_key, cfg.remote_api.has_api_key ? "ready" : "missing key"),
      statusLine("Local LLM", cfg.local_llm.available, cfg.local_llm.available ? cfg.local_llm.backend : "off"),
      statusLine(
        "ComfyUI",
        cfg.multimodal.comfyui.available,
        cfg.multimodal.comfyui.available
          ? cfg.multimodal.comfyui.managed_process
            ? "managed"
            : "ready"
          : "off",
      ),
      statusLine("Vision", cfg.multimodal.vision.available, cfg.multimodal.vision.available ? cfg.multimodal.vision.model : "off"),
      statusLine("Initiative", cfg.initiative.enabled, cfg.initiative.enabled ? `level ${cfg.initiative.level.toFixed(1)}` : "off"),
    ),
    el("div", { class: "sidebar-spacer" }),
    el("div", { class: "theme-toggle" }, icon("moon", 16), themeSelect),
    el("div", { class: "sidebar-actions" }, roleBtn, memoryBtn, imageBtn, companionBtn, settingsBtn),
  );

  const main = el("main", { class: "main-content" });
  const chatContainer = el("section", { class: "chat-container" });
  const header = el("header", { class: "chat-header" });
  const clearBtn = el("button", { class: "icon-btn", type: "button", title: "Clear chat", "aria-label": "Clear chat" }, icon("trash", 16));
  const restartBtn = el("button", { class: "icon-btn", type: "button", title: "Restart backend", "aria-label": "Restart backend" }, icon("refresh", 16)) as HTMLButtonElement;
  const initiativeBtn = el("button", { class: "icon-btn", type: "button", title: "Check initiative", "aria-label": "Check initiative" }, icon("sparkles", 16)) as HTMLButtonElement;
  const messages = el("div", { class: "chat-messages", id: "chat-messages" });
  clearBtn.addEventListener("click", () => {
    messages.innerHTML = "";
    chatHistory.length = 0;
  });
  restartBtn.addEventListener("click", async () => {
    restartBtn.disabled = true;
    try {
      await invoke("restart_backend");
      addMessage(messages, "assistant", "Backend processes restarted.");
    } catch (error) {
      addMessage(messages, "error", String(error));
    } finally {
      restartBtn.disabled = false;
    }
  });
  header.append(el("span", {}, "Chat"), el("div", { class: "chat-header-actions" }, initiativeBtn, restartBtn, clearBtn));

  const input = el("textarea", {
    id: "chat-input",
    placeholder: "Ask, write a note, or use \\image ...",
    rows: "1",
    autofocus: "",
  }) as HTMLTextAreaElement;
  const visionBtn = el("button", { class: "icon-btn image-send-btn", type: "button", title: "Recognize image", "aria-label": "Recognize image" }, icon("eye", 17)) as HTMLButtonElement;
  const imageSendBtn = el("button", { class: "icon-btn image-send-btn", type: "button", title: "Generate image from input", "aria-label": "Generate image from input" }, icon("image", 17)) as HTMLButtonElement;
  const imageReferenceBtn = el("button", { class: "icon-btn image-send-btn", type: "button", title: t("image.generateReference"), "aria-label": t("image.generateReference") }, icon("brush", 17)) as HTMLButtonElement;
  const sendBtn = el(
    "button",
    { id: "send-btn", type: "button", title: "Send" },
    icon("send", 17),
    el("span", { class: "send-label" }, "Send"),
  ) as HTMLButtonElement;
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !sendBtn.disabled) {
      event.preventDefault();
      sendBtn.click();
    }
  });
  const chatButtons = [sendBtn, imageSendBtn, imageReferenceBtn, visionBtn, initiativeBtn];
  sendBtn.addEventListener("click", () => handleSend(input, chatButtons, messages, "chat"));
  imageSendBtn.addEventListener("click", () => handleSend(input, chatButtons, messages, "image"));
  imageReferenceBtn.addEventListener("click", () => handleImageReferenceGeneration(input, chatButtons, messages));
  visionBtn.addEventListener("click", () => handleVisionUpload(input, chatButtons, messages));
  initiativeBtn.addEventListener("click", () => handleInitiative(input, chatButtons, messages));
  input.addEventListener("input", reportUserActivity);

  chatContainer.append(
    header,
    messages,
    el("div", { class: "chat-input-area" }, input, visionBtn, imageReferenceBtn, imageSendBtn, sendBtn),
  );
  main.append(chatContainer);
  layout.append(sidebar, main);
  app.append(layout);

  settingsBtn.addEventListener("click", () => {
    document.body.append(
      buildSettingsPanel(cfg, async () => {
        cfg = await loadConfig();
      }),
    );
  });
  listen("open-settings", () => {
    document.body.append(
      buildSettingsPanel(cfg, async () => {
        cfg = await loadConfig();
      }),
    );
  }).catch(() => {});
  listen("show-chat", () => {
    document.querySelectorAll(".settings-overlay").forEach((overlay) => overlay.remove());
    input.focus();
  }).catch(() => {});

  if (!localStorage.getItem(LOCAL_LLM_ONBOARDING_KEY)) {
    document.body.append(
      buildLocalLlmOnboarding(cfg, () => {
        void buildApp();
      }),
    );
  }
}

async function buildCompanionView() {
  const cfg = await loadConfig();
  applyTheme(cfg.app.theme.mode);
  setLanguage(cfg.app.language.ui);
  watchSystemTheme();

  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";
  document.body.classList.add("companion-body");

  const currentWindow = getCurrentWindow();
  let proactiveEnabled = cfg.initiative.enabled;
  let alwaysOnTop = true;
  let inFlight = false;
  let dialogVisible = false;
  let companionWindowVisible = await currentWindow.isVisible().catch(() => false);
  let restoringCompanionWindow = true;
  let persistWindowTimer: number | null = null;

  const shell = el("main", { class: "companion-surface" });
  const controls = el("div", { class: "companion-controls" });
  const pinBtn = el("button", { class: "companion-control active", type: "button", title: "Always on top", "aria-label": "Always on top" }, icon("chevronsUp", 15));
  const proactiveBtn = el("button", { class: proactiveEnabled ? "companion-control active" : "companion-control", type: "button", title: "Proactive speech", "aria-label": "Proactive speech" }, icon("sparkles", 15));
  const openMainBtn = el("button", { class: "companion-control", type: "button", title: "Open chat window", "aria-label": "Open chat window" }, icon("message", 15));
  const dialogBtn = el("button", { class: "companion-control", type: "button", title: "Dialogue", "aria-label": "Dialogue" }, icon("send", 15));
  const closeBtn = el("button", { class: "companion-control", type: "button", title: "Close companion", "aria-label": "Close companion" }, icon("x", 15));
  controls.append(pinBtn, proactiveBtn, openMainBtn, dialogBtn, closeBtn);

  const avatar = el("div", {
    class: "companion-avatar",
    title: "Drag companion",
    "aria-label": "Hestia companion",
    "data-tauri-drag-region": "",
  });
  const resizeHandle = el("button", { class: "companion-resize", type: "button", title: "Resize", "aria-label": "Resize companion" });
  let avatarAdapter: AvatarAdapter | null = null;
  const mountCompanionAvatar = (avatarConfig: AvatarConfig) => {
    avatarAdapter?.unmount();
    avatarAdapter = null;
    avatar.replaceChildren();
    if (!avatarConfig.enabled) return;
    avatarAdapter = createAvatarAdapter(avatarConfig);
    avatarAdapter.mount(avatar);
  };
  mountCompanionAvatar(cfg.app.avatar);
  shell.append(controls, avatar, resizeHandle);
  app.append(shell);

  let dialogWindow = await Window.getByLabel("companion_dialog");
  const getDialogWindow = async () => {
    dialogWindow = await Window.getByLabel("companion_dialog");
    return dialogWindow;
  };
  const clampCompanionSize = (width: number, height: number) => ({
    width: Math.max(COMPANION_MIN_SIZE.width, Math.min(COMPANION_MAX_SIZE.width, Math.round(width))),
    height: Math.max(COMPANION_MIN_SIZE.height, Math.min(COMPANION_MAX_SIZE.height, Math.round(height))),
  });
  const companionWindowConfig = cfg.companion?.window ?? {
    x: null,
    y: null,
    width: COMPANION_DEFAULT_SIZE.width,
    height: COMPANION_DEFAULT_SIZE.height,
  };
  const resolveVisibleCompanionPosition = async (x: number, y: number, width: number, height: number) => {
    const monitors = await availableMonitors().catch(() => []);
    const intersectsMonitor = monitors.some((monitor) => {
      const area = monitor.workArea ?? { position: monitor.position, size: monitor.size };
      return (
        x < area.position.x + area.size.width &&
        x + width > area.position.x &&
        y < area.position.y + area.size.height &&
        y + height > area.position.y
      );
    });
    if (intersectsMonitor) return new PhysicalPosition(x, y);

    const monitor = (await currentMonitor().catch(() => null)) ?? monitors[0] ?? null;
    if (!monitor) return null;
    const area = monitor.workArea ?? { position: monitor.position, size: monitor.size };
    return new PhysicalPosition(
      Math.round(area.position.x + Math.max(0, area.size.width - width) / 2),
      Math.round(area.position.y + Math.max(0, area.size.height - height) / 2),
    );
  };
  const restoreCompanionWindowBounds = async () => {
    const size = clampCompanionSize(companionWindowConfig.width, companionWindowConfig.height);
    await currentWindow.setSize(new LogicalSize(size.width, size.height)).catch(() => {});
    if (Number.isFinite(companionWindowConfig.x) && Number.isFinite(companionWindowConfig.y)) {
      const scaleFactor = await currentWindow.scaleFactor().catch(() => 1);
      const position = await resolveVisibleCompanionPosition(
        Math.round(companionWindowConfig.x ?? 0),
        Math.round(companionWindowConfig.y ?? 0),
        Math.round(size.width * scaleFactor),
        Math.round(size.height * scaleFactor),
      );
      if (position) {
        await currentWindow.setPosition(position).catch(() => {});
      }
    }
    restoringCompanionWindow = false;
  };
  const persistCompanionWindowBounds = async () => {
    if (!companionWindowVisible || restoringCompanionWindow) return;
    try {
      const [position, size, scaleFactor] = await Promise.all([
        currentWindow.outerPosition(),
        currentWindow.outerSize(),
        currentWindow.scaleFactor(),
      ]);
      const logicalSize = size.toLogical(scaleFactor);
      const clampedSize = clampCompanionSize(logicalSize.width, logicalSize.height);
      await invoke("update_settings", {
        updates: {
          companion_window_x: Math.round(position.x),
          companion_window_y: Math.round(position.y),
          companion_window_width: clampedSize.width,
          companion_window_height: clampedSize.height,
        },
      });
    } catch {
      // Window persistence is best-effort; interaction should never fail because saving failed.
    }
  };
  const scheduleCompanionWindowPersist = () => {
    if (!companionWindowVisible || restoringCompanionWindow) return;
    if (persistWindowTimer !== null) {
      window.clearTimeout(persistWindowTimer);
    }
    persistWindowTimer = window.setTimeout(() => {
      persistWindowTimer = null;
      persistCompanionWindowBounds();
    }, 500);
  };
  let avatarSpeakTimer: number | null = null;
  const stopAvatarSpeechTimer = () => {
    if (avatarSpeakTimer !== null) {
      window.clearTimeout(avatarSpeakTimer);
      avatarSpeakTimer = null;
    }
  };
  const dispatchAvatarEvent = (event: CompanionAvatarEvent) => {
    avatarAdapter?.onEvent?.(event.type, event.data);
    if (event.type === "speak_start") {
      stopAvatarSpeechTimer();
      const durationMs = Math.max(600, Math.min(8000, event.data?.duration_ms ?? 1800));
      avatarSpeakTimer = window.setTimeout(() => {
        avatarSpeakTimer = null;
        dispatchAvatarEvent({ type: "speak_stop" });
      }, durationMs);
      return;
    }
    if (event.type === "speak_stop" || event.type === "idle") {
      stopAvatarSpeechTimer();
    }
  };
  listen<AvatarConfig>("avatar-config-changed", (event) => {
    cfg.app.avatar = event.payload;
    stopAvatarSpeechTimer();
    mountCompanionAvatar(event.payload);
    dispatchAvatarEvent({ type: "idle" });
  }).catch(() => {});
  const speechDurationForText = (text: string) => Math.max(1200, Math.min(8000, text.length * 70));
  let controlsHideTimer: number | null = null;
  const hideControls = () => {
    if (controlsHideTimer !== null) {
      window.clearTimeout(controlsHideTimer);
      controlsHideTimer = null;
    }
    shell.classList.remove("controls-visible");
  };
  const showControls = (autoHide = true) => {
    shell.classList.add("controls-visible");
    if (controlsHideTimer !== null) {
      window.clearTimeout(controlsHideTimer);
      controlsHideTimer = null;
    }
    if (autoHide) {
      controlsHideTimer = window.setTimeout(hideControls, 1200);
    }
  };

  shell.addEventListener("pointermove", (event) => {
    showControls();
    const rect = avatar.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    const x = ((event.clientX - rect.left) / rect.width - 0.5) * 2;
    const y = ((event.clientY - rect.top) / rect.height - 0.5) * 2;
    dispatchAvatarEvent({ type: "look_at", data: { x, y } });
  });
  shell.addEventListener("pointerenter", () => showControls());
  shell.addEventListener("pointerleave", hideControls);
  shell.addEventListener("pointerout", (event) => {
    if (!event.relatedTarget || !shell.contains(event.relatedTarget as Node)) hideControls();
  });
  controls.addEventListener("pointerenter", () => showControls(false));
  controls.addEventListener("pointermove", () => showControls(false));
  controls.addEventListener("pointerleave", hideControls);
  controls.addEventListener("pointerdown", (event) => {
    event.stopPropagation();
    showControls(false);
  });
  controls.addEventListener("click", (event) => {
    event.stopPropagation();
  });
  window.addEventListener("mouseout", (event) => {
    if (!event.relatedTarget) hideControls();
  });
  window.addEventListener("pointerout", (event) => {
    if (!event.relatedTarget) hideControls();
  });
  document.addEventListener("mouseleave", hideControls);
  document.documentElement.addEventListener("mouseleave", hideControls);
  window.addEventListener("blur", hideControls);
  shell.addEventListener("dragstart", (event) => {
    event.preventDefault();
  });
  shell.addEventListener("selectstart", (event) => {
    event.preventDefault();
  });

  const updateControlState = () => {
    pinBtn.classList.toggle("active", alwaysOnTop);
    proactiveBtn.classList.toggle("active", proactiveEnabled);
    dialogBtn.classList.toggle("active", dialogVisible);
  };
  const setDialogVisibleState = (visible: boolean) => {
    dialogVisible = visible && companionWindowVisible;
    updateControlState();
  };

  const updateDialogPlacement = async () => {
    const dialog = await getDialogWindow();
    if (!dialog) return;
    try {
      const [monitor, position, size] = await Promise.all([
        currentMonitor(),
        currentWindow.outerPosition(),
        currentWindow.outerSize(),
      ]);
      const workArea = monitor?.workArea;
      if (!workArea) return;
      const dialogSize = await dialog.outerSize().catch(() => ({ width: 320, height: 220 }));
      const margin = 10;
      const workLeft = workArea.position.x;
      const workTop = workArea.position.y;
      const workRight = workLeft + workArea.size.width;
      const workBottom = workTop + workArea.size.height;
      const centerX = position.x + Math.round((size.width - dialogSize.width) / 2);
      const centerY = position.y + Math.round((size.height - dialogSize.height) / 2);
      const spaces = {
        top: position.y - workTop - margin,
        bottom: workBottom - (position.y + size.height) - margin,
        right: workRight - (position.x + size.width) - margin,
        left: position.x - workLeft - margin,
      };
      let placement: "top" | "bottom" | "right" | "left" = "top";
      if (spaces.top >= dialogSize.height) {
        placement = "top";
      } else if (spaces.bottom >= dialogSize.height) {
        placement = "bottom";
      } else if (spaces.right >= dialogSize.width) {
        placement = "right";
      } else if (spaces.left >= dialogSize.width) {
        placement = "left";
      } else {
        placement = Object.entries(spaces).sort((a, b) => b[1] - a[1])[0][0] as typeof placement;
      }
      const x =
        placement === "right"
          ? position.x + size.width + margin
          : placement === "left"
            ? position.x - dialogSize.width - margin
            : Math.max(workLeft + margin, Math.min(centerX, workRight - dialogSize.width - margin));
      const y =
        placement === "top"
          ? position.y - dialogSize.height - margin
          : placement === "bottom"
            ? position.y + size.height + margin
            : Math.max(workTop + margin, Math.min(centerY, workBottom - dialogSize.height - margin));
      await dialog.setPosition(new PhysicalPosition(x, y));
      currentWindow.emitTo("companion_dialog", "dialog-placement", placement).catch(() => {});
    } catch {
      // Placement is best-effort; the dialog remains usable at its current position.
    }
  };

  const showDialog = async (text?: string) => {
    if (!companionWindowVisible) return;
    setDialogVisibleState(true);
    await invoke("set_companion_dialog_visible", { visible: true }).catch(() => dialogWindow?.show().catch(() => {}));
    const dialog = await getDialogWindow();
    await dialog?.setAlwaysOnTop(alwaysOnTop).catch(() => {});
    updateDialogPlacement();
    if (text?.trim()) {
      const content = text.trim();
      window.setTimeout(() => {
        currentWindow.emitTo("companion_dialog", "companion-message", content).catch(() => {});
      }, 50);
      dispatchAvatarEvent({ type: "speak_start", data: { duration_ms: speechDurationForText(content) } });
      void emitAutonomousLive2DReaction(cfg.app.avatar, "", content, dispatchAvatarEvent);
    }
  };

  const hideDialog = async () => {
    setDialogVisibleState(false);
    await invoke("set_companion_dialog_visible", { visible: false }).catch(() => dialogWindow?.hide().catch(() => {}));
  };

  const requestCompanionInitiative = async () => {
    if (!companionWindowVisible || !proactiveEnabled || inFlight) return;
    inFlight = true;
    dispatchAvatarEvent({ type: "expression", data: { name: "thinking" } });
    try {
      const raw = await invoke<string>("request_initiative_message", {
        history: [],
        trigger: "companion_timer",
      });
      const response = JSON.parse(raw) as InitiativeResponse;
      if (response.allowed && response.content?.trim()) {
        showDialog(response.content.trim());
      } else {
        dispatchAvatarEvent({ type: "idle" });
      }
    } catch {
      // Timer-triggered companion checks stay silent on backend/model errors.
      dispatchAvatarEvent({ type: "idle" });
    } finally {
      inFlight = false;
    }
  };

  pinBtn.addEventListener("click", async () => {
    alwaysOnTop = !alwaysOnTop;
    updateControlState();
    try {
      await invoke("set_companion_always_on_top", { enabled: alwaysOnTop });
      await dialogWindow?.setAlwaysOnTop(alwaysOnTop).catch(() => {});
    } catch {
      alwaysOnTop = !alwaysOnTop;
      updateControlState();
    }
  });
  proactiveBtn.addEventListener("click", async () => {
    proactiveEnabled = !proactiveEnabled;
    updateControlState();
    try {
      await invoke("update_settings", { updates: { initiative_enabled: proactiveEnabled } });
    } catch {
      proactiveEnabled = !proactiveEnabled;
      updateControlState();
    }
  });
  openMainBtn.addEventListener("click", () => {
    invoke("show_main_window").catch(() => {});
  });
  dialogBtn.addEventListener("click", () => {
    if (dialogVisible) {
      hideDialog();
    } else {
      showDialog();
    }
  });
  closeBtn.addEventListener("click", () => {
    companionWindowVisible = false;
    setDialogVisibleState(false);
    dispatchAvatarEvent({ type: "idle" });
    updateControlState();
    hideControls();
    invoke("set_companion_visible", { visible: false }).catch(() => {});
  });

  resizeHandle.addEventListener("pointerdown", async (event) => {
    event.preventDefault();
    event.stopPropagation();
    const pointerId = event.pointerId;
    resizeHandle.setPointerCapture(pointerId);
    shell.classList.add("is-resizing");
    const startX = event.clientX;
    const startY = event.clientY;
    let active = true;
    let startLogical: { width: number; height: number } | null = null;
    let onMove: (moveEvent: PointerEvent) => void = () => {};
    const onDone = () => {
      if (!active) return;
      active = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onDone);
      window.removeEventListener("pointercancel", onDone);
      window.removeEventListener("blur", onDone);
      resizeHandle.removeEventListener("lostpointercapture", onDone);
      shell.classList.remove("is-resizing");
      if (resizeHandle.hasPointerCapture(pointerId)) {
        resizeHandle.releasePointerCapture(pointerId);
      }
      scheduleCompanionWindowPersist();
    };
    onMove = (moveEvent: PointerEvent) => {
      if (!active || !startLogical) return;
      if (moveEvent.buttons === 0) {
        onDone();
        return;
      }
      const width = Math.max(
        COMPANION_MIN_SIZE.width,
        Math.min(COMPANION_MAX_SIZE.width, startLogical.width + moveEvent.clientX - startX),
      );
      const height = Math.max(
        COMPANION_MIN_SIZE.height,
        Math.min(COMPANION_MAX_SIZE.height, startLogical.height + moveEvent.clientY - startY),
      );
      currentWindow.setSize(new LogicalSize(width, height)).then(updateDialogPlacement).catch(() => {});
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onDone);
    window.addEventListener("pointercancel", onDone);
    window.addEventListener("blur", onDone, { once: true });
    resizeHandle.addEventListener("lostpointercapture", onDone);
    try {
      const startSize = await currentWindow.innerSize();
      const factor = await currentWindow.scaleFactor();
      startLogical = startSize.toLogical(factor);
    } catch {
      onDone();
    }
  });

  await restoreCompanionWindowBounds();
  currentWindow.onMoved(() => {
    updateDialogPlacement();
    scheduleCompanionWindowPersist();
  }).catch(() => {});
  currentWindow.onResized(() => {
    updateDialogPlacement();
    scheduleCompanionWindowPersist();
  }).catch(() => {});
  currentWindow.onFocusChanged(({ payload }) => {
    if (!payload && alwaysOnTop) {
      invoke("set_companion_always_on_top", { enabled: true }).catch(() => {});
    }
  }).catch(() => {});
  window.setInterval(() => {
    if (companionWindowVisible && alwaysOnTop) {
      invoke("set_companion_always_on_top", { enabled: true }).catch(() => {});
    }
  }, 5000);
  listen<boolean>("companion-visible-changed", (event) => {
    companionWindowVisible = event.payload;
    if (!companionWindowVisible) {
      if (persistWindowTimer !== null) {
        window.clearTimeout(persistWindowTimer);
        persistWindowTimer = null;
      }
      setDialogVisibleState(false);
      inFlight = false;
      dispatchAvatarEvent({ type: "idle" });
      hideControls();
      updateControlState();
    }
  }).catch(() => {});
  listen<boolean>("companion-dialog-visible-changed", (event) => {
    setDialogVisibleState(event.payload);
    if (event.payload && companionWindowVisible) {
      updateDialogPlacement();
    } else {
      dispatchAvatarEvent({ type: "idle" });
    }
  }).catch(() => {});
  listen<CompanionAvatarEvent>("companion-avatar-event", (event) => {
    dispatchAvatarEvent(event.payload);
  }).catch(() => {});
  updateControlState();
  dispatchAvatarEvent({ type: "idle" });
  updateDialogPlacement();
  window.setInterval(requestCompanionInitiative, 120000);
}

async function buildCompanionDialogView() {
  const cfg = await loadConfig();
  applyTheme(cfg.app.theme.mode);
  watchSystemTheme();

  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";
  document.body.classList.add("companion-body", "companion-dialog-body");

  const companionHistory: HistoryEntry[] = [];
  let inFlight = false;
  let requestGeneration = 0;
  const panel = el("section", { class: "companion-dialog-panel", "aria-live": "polite" });
  const messages = el("div", { class: "companion-dialogue-messages" });
  const input = el("textarea", {
    class: "companion-dialogue-input",
    rows: "2",
    placeholder: "Say something...",
  }) as HTMLTextAreaElement;
  const sendBtn = el("button", { class: "companion-dialogue-send", type: "button", title: "Send", "aria-label": "Send" }, icon("send", 15)) as HTMLButtonElement;

  const appendDialogMessage = (role: string, text: string) => {
    const message = el("div", { class: `companion-dialogue-message ${role}` }, text);
    messages.append(message);
    messages.scrollTop = messages.scrollHeight;
  };
  const emitAvatarEvent = (event: CompanionAvatarEvent) => {
    getCurrentWindow().emitTo("companion", "companion-avatar-event", event).catch(() => {});
  };
  const speechDurationForText = (text: string) => Math.max(1200, Math.min(8000, text.length * 70));
  const sendMessage = async () => {
    const text = input.value.trim();
    if (!text || inFlight) return;
    input.value = "";
    appendDialogMessage("user", text);
    companionHistory.push({ role: "user", content: text });
    inFlight = true;
    emitAvatarEvent({ type: "expression", data: { name: "thinking" } });
    const generation = ++requestGeneration;
    input.disabled = true;
    sendBtn.disabled = true;
    try {
      const raw = await invoke<string>("send_chat_message", { message: text, history: companionHistory });
      if (generation !== requestGeneration) return;
      const response = JSON.parse(raw) as ChatResponse;
      const content = response.content || "";
      appendDialogMessage("assistant", content);
      companionHistory.push({ role: "assistant", content });
      emitAvatarEvent({ type: "speak_start", data: { duration_ms: speechDurationForText(content) } });
      void emitAutonomousLive2DReaction(cfg.app.avatar, text, content, emitAvatarEvent);
    } catch (error) {
      if (generation !== requestGeneration) return;
      appendDialogMessage("error", String(error));
      emitAvatarEvent({ type: "expression", data: { name: "confused" } });
    } finally {
      if (generation !== requestGeneration) return;
      inFlight = false;
      input.disabled = false;
      sendBtn.disabled = false;
      input.focus();
    }
  };

  sendBtn.addEventListener("click", sendMessage);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  });
  messages.setAttribute("data-tauri-drag-region", "");
  panel.addEventListener("dragstart", (event) => {
    event.preventDefault();
  });
  listen<string>("companion-message", (event) => {
    appendDialogMessage("assistant", event.payload);
    companionHistory.push({ role: "assistant", content: event.payload });
    invoke("set_companion_dialog_visible", { visible: true }).catch(() => getCurrentWindow().show().catch(() => {}));
  }).catch(() => {});
  listen<boolean>("companion-visible-changed", (event) => {
    if (!event.payload) {
      requestGeneration++;
      input.disabled = false;
      sendBtn.disabled = false;
      inFlight = false;
      emitAvatarEvent({ type: "idle" });
    }
  }).catch(() => {});
  listen<boolean>("companion-dialog-visible-changed", (event) => {
    if (!event.payload) {
      requestGeneration++;
      input.disabled = false;
      sendBtn.disabled = false;
      inFlight = false;
      emitAvatarEvent({ type: "idle" });
    }
  }).catch(() => {});
  listen<string>("dialog-placement", (event) => {
    panel.dataset.placement = event.payload;
  }).catch(() => {});
  listen<AvatarConfig>("avatar-config-changed", (event) => {
    cfg.app.avatar = event.payload;
  }).catch(() => {});

  panel.append(messages, el("div", { class: "companion-dialogue-input-row" }, input, sendBtn));
  app.append(panel);
}

function addMessage(container: HTMLElement, role: string, text: string) {
  const message = el("div", { class: `message ${role}` }, text);
  container.append(message);
  container.scrollTop = container.scrollHeight;
  return message;
}

async function appendGeneratedImages(message: HTMLElement, imagePaths: string[]) {
  if (imagePaths.length === 0) return;
  const gallery = el("div", { class: "message-images" });
  message.append(gallery);
  for (const path of imagePaths) {
    const figure = el("figure", { class: "generated-image" });
    const img = el("img", { alt: "Generated image" });
    const caption = el("figcaption", {}, path);
    figure.append(img, caption);
    gallery.append(figure);
    try {
      img.setAttribute("src", await invoke<string>("read_image_artifact", { path }));
    } catch (error) {
      img.replaceWith(el("div", { class: "artifact-path" }, `Preview failed: ${String(error)}`));
    }
  }
}

async function handleVisionUpload(input: HTMLTextAreaElement, buttons: HTMLButtonElement[], messages: HTMLElement) {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
  });
  if (!selected || typeof selected !== "string") return;

  const prompt = input.value.trim();
  const userText = prompt ? `[image] ${selected}\n${prompt}` : `[image] ${selected}`;
  addMessage(messages, "user", userText);
  input.disabled = true;
  buttons.forEach((button) => {
    button.disabled = true;
  });
  const loadingMessage = addMessage(messages, "loading", "Recognizing image...");

  try {
    const raw = await invoke<string>("recognize_image", { path: selected, prompt: prompt || null });
    const response = JSON.parse(raw) as VisionResponse;
    loadingMessage.remove();
    const content = response.content || "No visual description returned.";
    const messageElement = addMessage(messages, "assistant", content);
    messageElement.setAttribute("data-vision", "true");
    if (response.model) {
      messageElement.setAttribute("title", response.model);
    }
    chatHistory.push({ role: "user", content: userText });
    chatHistory.push({ role: "assistant", content });
  } catch (error) {
    loadingMessage.remove();
    addMessage(messages, "error", String(error));
  } finally {
    input.disabled = false;
    buttons.forEach((button) => {
      button.disabled = false;
    });
    input.focus();
  }
}

function initiativeReasonText(decision: InitiativeDecision): string {
  if (decision.reasons.length === 0) return "allowed";
  const detail = decision.reasons.join(", ");
  const cooldown = decision.cooldown_remaining_ms > 0 ? `, cooldown ${Math.ceil(decision.cooldown_remaining_ms / 1000)}s` : "";
  return `${detail}${cooldown}`;
}

async function handleInitiative(
  input: HTMLTextAreaElement,
  buttons: HTMLButtonElement[],
  messages: HTMLElement,
) {
  if (input.disabled) return;
  input.disabled = true;
  buttons.forEach((button) => {
    button.disabled = true;
  });
  const loadingMessage = addMessage(messages, "loading", "Checking initiative...");

  try {
    const raw = await invoke<string>("request_initiative_message", {
      history: chatHistory,
      trigger: "manual",
    });
    const response = JSON.parse(raw) as InitiativeResponse;
    loadingMessage.remove();
    if (!response.allowed) {
      addMessage(messages, "assistant", t("initiative.notAllowed", { reason: initiativeReasonText(response.decision) }));
      return;
    }
    const content = response.content || "";
    const messageElement = addMessage(messages, "assistant", content);
    messageElement.setAttribute("data-initiative", "true");
    messageElement.title = `initiative score ${response.decision.score.toFixed(2)}`;
    chatHistory.push({ role: "assistant", content });
  } catch (error) {
    loadingMessage.remove();
    addMessage(messages, "error", String(error));
  } finally {
    input.disabled = false;
    buttons.forEach((button) => {
      button.disabled = false;
    });
    input.focus();
  }
}

async function handleImageReferenceGeneration(
  input: HTMLTextAreaElement,
  buttons: HTMLButtonElement[],
  messages: HTMLElement,
) {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
  });
  if (!selected || typeof selected !== "string") return;

  const prompt = input.value.trim();
  const outgoingText = prompt;
  addMessage(messages, "user", prompt ? `[image] ${selected}\n${prompt}` : `[image] ${selected}`);
  input.value = "";
  input.disabled = true;
  buttons.forEach((button) => {
    button.disabled = true;
  });
  const loadingMessage = addMessage(messages, "loading", "Generating image...");

  try {
    const raw = await invoke<string>("send_chat_message", {
      message: outgoingText,
      history: chatHistory,
      inputImagePath: selected,
      imageMode: null,
      denoise: null,
    });
    const response = JSON.parse(raw) as ChatResponse;
    const content = response.content || "";
    loadingMessage.remove();
    const messageElement = addMessage(messages, "assistant", content);
    messageElement.setAttribute("data-generated-image", "true");
    if (response.image_prompt || response.input_image_path) {
      messageElement.setAttribute("title", [response.mode, response.input_image_path, response.image_prompt].filter(Boolean).join(" | "));
    }
    await appendGeneratedImages(messageElement, response.images ?? []);
    chatHistory.push({ role: "user", content: outgoingText });
    chatHistory.push({ role: "assistant", content });
  } catch (error) {
    loadingMessage.remove();
    addMessage(messages, "error", String(error));
  } finally {
    input.disabled = false;
    buttons.forEach((button) => {
      button.disabled = false;
    });
    input.focus();
  }
}

async function invokeStreamingChat(
  outgoingText: string,
  loadingMessage: HTMLElement,
  messages: HTMLElement,
): Promise<ChatResponse> {
  const requestId = createRequestId("chat");
  let content = "";
  let messageElement: HTMLElement | undefined;
  let renderFrame: number | undefined;
  const ensureMessageElement = () => {
    if (!messageElement) {
      loadingMessage.remove();
      messageElement = addMessage(messages, "assistant", "");
    }
    return messageElement;
  };
  const renderContent = () => {
    renderFrame = undefined;
    ensureMessageElement().textContent = content;
    messages.scrollTop = messages.scrollHeight;
  };
  const unlisten = await listen<ChatStreamDelta>("chat-stream-delta", (event) => {
    if (event.payload.request_id !== requestId) return;
    content += event.payload.delta;
    if (renderFrame === undefined) {
      renderFrame = requestAnimationFrame(renderContent);
    }
  });

  try {
    const raw = await invoke<string>("send_chat_message_stream", {
      requestId,
      message: outgoingText,
      history: chatHistory,
    });
    const response = JSON.parse(raw) as ChatResponse;
    content = response.content || content;
    if (renderFrame !== undefined) {
      cancelAnimationFrame(renderFrame);
      renderFrame = undefined;
    }
    const finalMessageElement = ensureMessageElement();
    finalMessageElement.textContent = content;
    if (response.generated_image) {
      finalMessageElement.setAttribute("data-generated-image", "true");
      if (response.image_prompt) {
        finalMessageElement.setAttribute("title", response.image_prompt);
      }
      await appendGeneratedImages(finalMessageElement, response.images ?? []);
    }
    messages.scrollTop = messages.scrollHeight;
    return { ...response, content };
  } finally {
    if (renderFrame !== undefined) {
      cancelAnimationFrame(renderFrame);
      renderContent();
    }
    unlisten();
  }
}

async function handleSend(input: HTMLTextAreaElement, buttons: HTMLButtonElement[], messages: HTMLElement, mode: "chat" | "image") {
  const text = input.value.trim();
  if (!text) return;
  const outgoingText = mode === "image" ? `\\image ${text}` : text;
  const loadingText = mode === "image" || text.startsWith("\\image") || text.startsWith("/image") ? "Generating image..." : "Thinking...";
  const useStreaming = mode === "chat" && !text.startsWith("\\image") && !text.startsWith("/image");

  addMessage(messages, "user", outgoingText);
  input.value = "";
  input.disabled = true;
  buttons.forEach((button) => {
    button.disabled = true;
  });
  const loadingMessage = addMessage(messages, "loading", loadingText);

  try {
    if (useStreaming) {
      const response = await invokeStreamingChat(outgoingText, loadingMessage, messages);
      const content = response.content || "";
      chatHistory.push({ role: "user", content: outgoingText });
      chatHistory.push({ role: "assistant", content });
    } else {
      const raw = await invoke<string>("send_chat_message", {
        message: outgoingText,
        history: chatHistory,
        inputImagePath: null,
        imageMode: mode === "image" ? "text_to_image" : null,
        denoise: null,
      });
      const response = JSON.parse(raw) as ChatResponse;
      const content = response.content || "";
      loadingMessage.remove();
      const messageElement = addMessage(messages, "assistant", content);
      if (response.rewritten) {
        messageElement.setAttribute("data-rewritten", "true");
        messageElement.title = "Rewritten by local LLM";
      }
      if (response.generated_image) {
        messageElement.setAttribute("data-generated-image", "true");
        if (response.image_prompt) {
          messageElement.setAttribute("title", response.image_prompt);
        }
        await appendGeneratedImages(messageElement, response.images ?? []);
      }
      chatHistory.push({ role: "user", content: outgoingText });
      chatHistory.push({ role: "assistant", content });
    }
  } catch (error) {
    loadingMessage.remove();
    addMessage(messages, "error", String(error));
  } finally {
    input.disabled = false;
    buttons.forEach((button) => {
      button.disabled = false;
    });
    input.focus();
  }
}

const windowLabel = getCurrentWindow().label;
if (windowLabel === "companion") {
  buildCompanionView();
} else if (windowLabel === "companion_dialog") {
  buildCompanionDialogView();
} else {
  buildApp();
}
