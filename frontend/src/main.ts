import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { currentMonitor, getCurrentWindow, Window } from "@tauri-apps/api/window";
import "./style.css";

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
    avatar: { enabled: boolean; image_path: string; model_type: string };
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
      output_dir: string;
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

type HistoryEntry = { role: string; content: string };

const chatHistory: HistoryEntry[] = [];

interface ChatResponse {
  content: string;
  rewritten?: boolean;
  generated_image?: boolean;
  image_prompt?: string;
  images?: string[];
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

let lastActivityReport = 0;

const iconPaths = {
  brush:
    "M12 20h9 M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z",
  check:
    "M20 6 9 17l-5-5",
  companion:
    "M4 10l2-5 4 3h4l4-3 2 5v5a6 6 0 0 1-6 6h-4a6 6 0 0 1-6-6v-5z M9 14h.01 M15 14h.01 M10 17c1.3.8 2.7.8 4 0",
  eye:
    "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  gear:
    "M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.5a2 2 0 0 1-1 1.8l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.8v-.5a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  image:
    "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M21 9V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v4 M3 15l5-5 4 4 2-2 7 7 M14 8h.01",
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
}

function createAvatarAdapter(_modelType: string, imagePath: string): AvatarAdapter {
  return {
    type: "placeholder",
    mount(container: HTMLElement) {
      container.append(el("img", { src: `/${imagePath}`, alt: "Hestia companion" }));
    },
    unmount() {},
  };
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
      setStatus(status, true, `Saved ${profile}.json. The next message will use it.`);
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
    textarea,
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
      });
      const response = JSON.parse(raw) as { prompt_id: string; images: string[]; workflow_path: string };
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
    ),
    result,
    el("div", { class: "settings-actions" }, runBtn, closeBtn),
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
    placeholder: cfg.remote_api.has_api_key ? "Existing key is set" : "sk-...",
  }) as HTMLInputElement;
  const baseUrlInput = el("input", { type: "text", value: cfg.remote_api.base_url }) as HTMLInputElement;
  const modelInput = el("input", { type: "text", value: cfg.remote_api.model }) as HTMLInputElement;

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
  const rewriteToggle = el("input", { type: "checkbox" }) as HTMLInputElement;
  rewriteToggle.checked = cfg.persona_rewrite.enabled;
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
  const modelHint = el("span", { class: "hint" }, "Scan the configured models_dir for .gguf files.");

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
  const comfyOutputDir = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.output_dir,
    placeholder: "data/artifacts/images",
  }) as HTMLInputElement;
  const comfyLaunchCommand = el("input", {
    type: "text",
    value: cfg.multimodal.comfyui.launch_command,
    placeholder: "Use {python} main.py --listen {host} --port {port}",
  }) as HTMLInputElement;
  const visionEnabled = el("input", { type: "checkbox" }) as HTMLInputElement;
  visionEnabled.checked = cfg.multimodal.vision.enabled;
  const visionApiKey = el("input", {
    type: "password",
    placeholder: cfg.multimodal.vision.has_api_key ? "Existing key is set" : "MOONSHOT_API_KEY",
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

  const browseBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Browse") as HTMLButtonElement;
  const scanBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Scan") as HTMLButtonElement;

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
        modelHint.textContent = "No .gguf models found in models_dir.";
        return;
      }
      modelHint.textContent = models
        .slice(0, 4)
        .map((model) => `${model.manufacturer}/${model.model_name}`)
        .join(", ");
      if (models.length > 4) {
        modelHint.textContent += `, +${models.length - 4} more`;
      }
    } catch (error) {
      modelHint.textContent = `Scan failed: ${String(error)}`;
    }
  });

  const browseComfyRoot = el("button", { class: "btn btn-secondary", type: "button" }, "Browse");
  browseComfyRoot.addEventListener("click", async () => {
    const selected = await open({ multiple: false, directory: true });
    if (selected && typeof selected === "string") comfyRootDir.value = selected;
  });

  const browseComfyPython = el("button", { class: "btn btn-secondary", type: "button" }, "Browse");
  browseComfyPython.addEventListener("click", async () => {
    const selected = await open({ multiple: false });
    if (selected && typeof selected === "string") comfyPython.value = selected;
  });

  const browseWorkflow = el("button", { class: "btn btn-secondary", type: "button" }, "Browse");
  browseWorkflow.addEventListener("click", async () => {
    const selected = await open({ multiple: false, filters: [{ name: "JSON Workflow", extensions: ["json"] }] });
    if (selected && typeof selected === "string") comfyWorkflow.value = selected;
  });

  const browseOutputDir = el("button", { class: "btn btn-secondary", type: "button" }, "Browse");
  browseOutputDir.addEventListener("click", async () => {
    const selected = await open({ multiple: false, directory: true });
    if (selected && typeof selected === "string") comfyOutputDir.value = selected;
  });

  const themeSelect = el("select") as HTMLSelectElement;
  ["system", "dark", "light"].forEach((value) => {
    themeSelect.append(option(value, value.charAt(0).toUpperCase() + value.slice(1), cfg.app.theme.mode));
  });

  const saveBtn = el("button", { class: "btn btn-primary", type: "button" }, icon("check", 16), "Save");
  const closeBtn = el("button", { class: "btn btn-secondary", type: "button" }, "Close");

  saveBtn.addEventListener("click", async () => {
    const updates: Record<string, string | boolean | number> = {};
    const apiKey = apiKeyInput.value.trim();
    const localModel = localModelInput.value.trim();
    const localUrl = localUrlInput.value.trim();
    const loadCommand = loadCmdInput.value.trim();
    const unloadCommand = unloadCmdInput.value.trim();

    if (apiKey) updates.api_key = apiKey;
    if (baseUrlInput.value !== cfg.remote_api.base_url) updates.base_url = baseUrlInput.value;
    if (modelInput.value !== cfg.remote_api.model) updates.model = modelInput.value;
    if (themeSelect.value !== cfg.app.theme.mode) updates.theme_mode = themeSelect.value;
    if (backendSelect.value !== cfg.local_llm.backend) updates.local_llm_backend = backendSelect.value;
    if (localUrl !== cfg.local_llm.base_url) updates.local_llm_base_url = localUrl;
    if (localModel !== cfg.local_llm.model) updates.local_llm_model = localModel;
    if (llmToggle.checked !== cfg.local_llm.enabled) updates.local_llm_enabled = llmToggle.checked;
    if (autoLoadToggle.checked !== cfg.local_llm.auto_load) updates.local_llm_auto_load = autoLoadToggle.checked;
    if (rewriteToggle.checked !== cfg.persona_rewrite.enabled) {
      updates.persona_rewrite_enabled = rewriteToggle.checked;
    }
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
    if (comfyOutputDir.value.trim() !== cfg.multimodal.comfyui.output_dir) {
      updates.comfyui_output_dir = comfyOutputDir.value.trim();
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
      setStatus(status, true, "No changes.");
      return;
    }

    try {
      await invoke("update_settings", { updates });
      if (updates.theme_mode) {
        applyTheme(String(updates.theme_mode));
      }
      setStatus(status, true, "Saved. Restart backend-affecting settings to apply.");
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
  const comfyRootControls = el("div", { class: "inline-controls two" }, comfyRootDir, browseComfyRoot);
  const comfyPythonControls = el("div", { class: "inline-controls two" }, comfyPython, browseComfyPython);
  const comfyWorkflowControls = el("div", { class: "inline-controls two" }, comfyWorkflow, browseWorkflow);
  const comfyOutputControls = el("div", { class: "inline-controls two" }, comfyOutputDir, browseOutputDir);

  panel.append(
    el("h2", {}, "Settings"),
    status,
    el(
      "div",
      { class: "settings-section" },
      el("h3", {}, "Remote API"),
      fieldRow("API key", apiKeyInput, "Leave blank to keep the current key."),
      fieldRow("Base URL", baseUrlInput),
      fieldRow("Model", modelInput),
    ),
    el(
      "div",
      { class: "settings-section" },
      el("h3", {}, "Local LLM"),
      fieldRow("Backend", backendSelect),
      fieldRow("Base URL", localUrlInput),
      fieldRow("Enable", llmToggle),
      fieldRow("Auto-load", autoLoadToggle, "Starts the inference server on app launch when supported."),
      fieldRow("Rewrite", rewriteToggle, "Restyles remote responses through the local model."),
      fieldRow("Model", modelControls, "Use manufacturer/model_name for discovered GGUF files."),
      modelHint,
      fieldRow("Load command", loadCmdInput, "Placeholders: {model_path}, {port}, {host}."),
      fieldRow("Unload command", unloadCmdInput, "Empty means terminate the spawned process."),
    ),
    el(
      "div",
      { class: "settings-section" },
      el("h3", {}, "ComfyUI"),
      fieldRow("Enable", comfyEnabled),
      fieldRow("On-demand start", comfyAutoStart, "Starts ComfyUI only when an image is requested, then stops the managed process after completion."),
      fieldRow("Base URL", comfyBaseUrl),
      fieldRow("Root dir", comfyRootControls),
      fieldRow("Python", comfyPythonControls, "Use the Python executable inside conda or venv."),
      fieldRow("Env type", comfyEnv),
      fieldRow("Workflow", comfyWorkflowControls),
      fieldRow("Output dir", comfyOutputControls),
      fieldRow("Launch command", comfyLaunchCommand, "Optional. Placeholders: {python}, {root_dir}, {host}, {port}."),
    ),
    el(
      "div",
      { class: "settings-section" },
      el("h3", {}, "Kimi Vision"),
      fieldRow("Enable", visionEnabled),
      fieldRow("API key", visionApiKey, `Leave blank to keep current key. Env fallback: ${cfg.multimodal.vision.api_key_env}.`),
      fieldRow("Base URL", visionBaseUrl),
      fieldRow("Model", visionModel),
      fieldRow("Default prompt", visionDefaultPrompt),
      fieldRow("Max bytes", visionMaxBytes, "Applies to local uploads and future screenshots."),
    ),
    el(
      "div",
      { class: "settings-section" },
      el("h3", {}, "Initiative"),
      fieldRow("Enable", initiativeEnabled),
      fieldRow("Level", initiativeLevel, "Higher levels reduce the required idle time."),
      initiativeLevelHint,
      fieldRow("Cooldown ms", initiativeCooldown),
    ),
    el(
      "div",
      { class: "settings-section" },
      el("h3", {}, "Appearance"),
      fieldRow("Theme", themeSelect),
    ),
    el("div", { class: "settings-actions" }, saveBtn, closeBtn),
  );
  overlay.append(panel);
  return overlay;
}

function fallbackConfig(): ConfigSnapshot {
  return {
    app: {
      name: "Hestia",
      theme: { mode: "system" },
      avatar: { enabled: true, image_path: "companion-cat-placeholder.png", model_type: "placeholder" },
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
        output_dir: "data/artifacts/images",
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
    return JSON.parse(await invoke("get_config_snapshot"));
  } catch {
    return fallbackConfig();
  }
}

function companionImagePath(cfg: ConfigSnapshot): string {
  return cfg.app.avatar.image_path || "companion-cat-placeholder.png";
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
  watchSystemTheme();

  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";

  const layout = el("div", { class: "app-layout" });
  const sidebar = el("aside", { class: "sidebar" });
  const avatar = el("div", { class: "avatar-container", id: "avatar-container" });
  if (cfg.app.avatar.enabled) {
    createAvatarAdapter(cfg.app.avatar.model_type, companionImagePath(cfg)).mount(avatar);
  }

  const themeSelect = el("select", { id: "theme-select", "aria-label": "Theme" }) as HTMLSelectElement;
  ["system", "dark", "light"].forEach((value) => {
    themeSelect.append(option(value, value.charAt(0).toUpperCase() + value.slice(1), cfg.app.theme.mode));
  });
  themeSelect.addEventListener("change", () => applyTheme(themeSelect.value));

  const personaBtn = el("button", { class: "sidebar-btn", type: "button" }, icon("brush", 16), "Persona");
  personaBtn.addEventListener("click", () => document.body.append(buildPersonaEditor(cfg)));

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
        "Rewrite",
        cfg.persona_rewrite.enabled && cfg.local_llm.available,
        cfg.persona_rewrite.enabled ? "enabled" : "off",
      ),
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
    el("div", { class: "sidebar-actions" }, personaBtn, imageBtn, companionBtn, settingsBtn),
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
  const sendBtn = el("button", { id: "send-btn", type: "button", title: "Send" }, icon("send", 17), "Send") as HTMLButtonElement;
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !sendBtn.disabled) {
      event.preventDefault();
      sendBtn.click();
    }
  });
  const chatButtons = [sendBtn, imageSendBtn, visionBtn, initiativeBtn];
  sendBtn.addEventListener("click", () => handleSend(input, chatButtons, messages, "chat"));
  imageSendBtn.addEventListener("click", () => handleSend(input, chatButtons, messages, "image"));
  visionBtn.addEventListener("click", () => handleVisionUpload(input, chatButtons, messages));
  initiativeBtn.addEventListener("click", () => handleInitiative(input, chatButtons, messages));
  input.addEventListener("input", reportUserActivity);

  chatContainer.append(
    header,
    messages,
    el("div", { class: "chat-input-area" }, input, visionBtn, imageSendBtn, sendBtn),
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
}

async function buildCompanionView() {
  const cfg = await loadConfig();
  applyTheme(cfg.app.theme.mode);
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
  let companionWindowVisible = true;
  let restoringCompanionWindow = true;
  let persistWindowTimer: number | null = null;

  const shell = el("main", { class: "companion-surface" });
  const controls = el("div", { class: "companion-controls" });
  const pinBtn = el("button", { class: "companion-control active", type: "button", title: "Always on top", "aria-label": "Always on top" }, "Top");
  const proactiveBtn = el("button", { class: proactiveEnabled ? "companion-control active" : "companion-control", type: "button", title: "Proactive speech", "aria-label": "Proactive speech" }, "Talk");
  const openMainBtn = el("button", { class: "companion-control", type: "button", title: "Open chat window", "aria-label": "Open chat window" }, "Chat");
  const dialogBtn = el("button", { class: "companion-control", type: "button", title: "Dialogue", "aria-label": "Dialogue" }, "Bubble");
  const closeBtn = el("button", { class: "companion-control", type: "button", title: "Close companion", "aria-label": "Close companion" }, "Close");
  controls.append(pinBtn, proactiveBtn, openMainBtn, dialogBtn, closeBtn);

  const avatar = el("div", {
    class: "companion-avatar",
    title: "Drag companion",
    "aria-label": "Hestia companion",
    "data-tauri-drag-region": "",
  });
  const resizeHandle = el("button", { class: "companion-resize", type: "button", title: "Resize", "aria-label": "Resize companion" });
  avatar.append(el("img", { src: `/${companionImagePath(cfg)}`, alt: "Hestia companion", draggable: "false" }));
  shell.append(controls, avatar, resizeHandle);
  app.append(shell);

  const dialogWindow = await Window.getByLabel("companion_dialog");
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
  const restoreCompanionWindowBounds = async () => {
    const size = clampCompanionSize(companionWindowConfig.width, companionWindowConfig.height);
    await currentWindow.setSize(new LogicalSize(size.width, size.height)).catch(() => {});
    if (Number.isFinite(companionWindowConfig.x) && Number.isFinite(companionWindowConfig.y)) {
      await currentWindow
        .setPosition(new PhysicalPosition(Math.round(companionWindowConfig.x ?? 0), Math.round(companionWindowConfig.y ?? 0)))
        .catch(() => {});
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

  shell.addEventListener("pointermove", () => showControls());
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

  const updateDialogPlacement = async () => {
    if (!dialogWindow) return;
    try {
      const [monitor, position, size] = await Promise.all([
        currentMonitor(),
        currentWindow.outerPosition(),
        currentWindow.outerSize(),
      ]);
      const workArea = monitor?.workArea;
      if (!workArea) return;
      const dialogSize = await dialogWindow.outerSize().catch(() => ({ width: 320, height: 220 }));
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
      await dialogWindow.setPosition(new PhysicalPosition(x, y));
      currentWindow.emitTo("companion_dialog", "dialog-placement", placement).catch(() => {});
    } catch {
      // Placement is best-effort; the dialog remains usable at its current position.
    }
  };

  const showDialog = async (text?: string) => {
    dialogVisible = true;
    updateControlState();
    await updateDialogPlacement();
    if (dialogWindow) {
      await dialogWindow
        .show()
        .catch(() => invoke("set_companion_dialog_visible", { visible: true }).catch(() => {}));
      await dialogWindow.setAlwaysOnTop(alwaysOnTop).catch(() => {});
    } else {
      await invoke("set_companion_dialog_visible", { visible: true }).catch(() => {});
    }
    updateDialogPlacement();
    if (text?.trim()) {
      currentWindow.emitTo("companion_dialog", "companion-message", text.trim()).catch(() => {});
    }
  };

  const hideDialog = async () => {
    dialogVisible = false;
    updateControlState();
    if (dialogWindow) {
      await dialogWindow
        .hide()
        .catch(() => invoke("set_companion_dialog_visible", { visible: false }).catch(() => {}));
    } else {
      await invoke("set_companion_dialog_visible", { visible: false }).catch(() => {});
    }
  };

  const requestCompanionInitiative = async () => {
    if (!companionWindowVisible || !proactiveEnabled || inFlight) return;
    inFlight = true;
    try {
      const raw = await invoke<string>("request_initiative_message", {
        history: [],
        trigger: "companion_timer",
      });
      const response = JSON.parse(raw) as InitiativeResponse;
      if (response.allowed && response.content?.trim()) {
        showDialog(response.content.trim());
      }
    } catch {
      // Timer-triggered companion checks stay silent on backend/model errors.
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
    dialogVisible = false;
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
      dialogVisible = false;
      inFlight = false;
      hideControls();
      updateControlState();
    }
  }).catch(() => {});
  updateControlState();
  updateDialogPlacement();
  window.setInterval(requestCompanionInitiative, 120000);
}

async function buildCompanionDialogView() {
  applyTheme((await loadConfig()).app.theme.mode);
  watchSystemTheme();

  const app = document.getElementById("app");
  if (!app) return;
  app.innerHTML = "";
  document.body.classList.add("companion-body", "companion-dialog-body");

  const companionHistory: HistoryEntry[] = [];
  let inFlight = false;
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
  const sendMessage = async () => {
    const text = input.value.trim();
    if (!text || inFlight) return;
    input.value = "";
    appendDialogMessage("user", text);
    companionHistory.push({ role: "user", content: text });
    inFlight = true;
    input.disabled = true;
    sendBtn.disabled = true;
    try {
      const raw = await invoke<string>("send_chat_message", { message: text, history: companionHistory });
      const response = JSON.parse(raw) as ChatResponse;
      const content = response.content || "";
      appendDialogMessage("assistant", content);
      companionHistory.push({ role: "assistant", content });
    } catch (error) {
      appendDialogMessage("error", String(error));
    } finally {
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
    getCurrentWindow().show().catch(() => {});
  }).catch(() => {});
  listen<string>("dialog-placement", (event) => {
    panel.dataset.placement = event.payload;
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
      addMessage(messages, "assistant", `暂时不主动发言: ${initiativeReasonText(response.decision)}`);
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

async function handleSend(input: HTMLTextAreaElement, buttons: HTMLButtonElement[], messages: HTMLElement, mode: "chat" | "image") {
  const text = input.value.trim();
  if (!text) return;
  const outgoingText = mode === "image" ? `\\image ${text}` : text;
  const loadingText = mode === "image" || text.startsWith("\\image") || text.startsWith("/image") ? "Generating image..." : "Thinking...";

  addMessage(messages, "user", outgoingText);
  input.value = "";
  input.disabled = true;
  buttons.forEach((button) => {
    button.disabled = true;
  });
  const loadingMessage = addMessage(messages, "loading", loadingText);

  try {
    const raw = await invoke<string>("send_chat_message", { message: outgoingText, history: chatHistory });
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
