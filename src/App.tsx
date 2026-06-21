import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";

const LANGS: { code: string; name: string }[] = [
  { code: "vi", name: "Vietnamese" },
  { code: "en", name: "English" },
  { code: "ja", name: "Japanese" },
  { code: "ko", name: "Korean" },
  { code: "zh-CN", name: "Chinese (Simp.)" },
  { code: "zh-TW", name: "Chinese (Trad.)" },
  { code: "fr", name: "French" },
  { code: "de", name: "German" },
  { code: "es", name: "Spanish" },
  { code: "ru", name: "Russian" },
  { code: "th", name: "Thai" },
];

const PROVIDERS = {
  openai: {
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-4o-mini"
  },
  deepseek: {
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com",
    defaultModel: "deepseek-chat"
  },
  mistral: {
    name: "Mistral AI",
    baseUrl: "https://api.mistral.ai/v1",
    defaultModel: "open-mistral-7b"
  },
  openrouter: {
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "meta-llama/llama-3-8b-instruct:free"
  },
  groq: {
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "llama3-8b-8192"
  },
  google: {
    name: "Google Gemini",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    defaultModel: "gemini-1.5-flash"
  },
  custom: {
    name: "Custom (OpenAI-compatible)",
    baseUrl: "",
    defaultModel: ""
  }
};

const formatKeyLabel = (code: string) => {
  if (!code) return "NONE";
  if (code.startsWith("Key")) {
    return code.substring(3);
  }
  if (code.startsWith("Digit")) {
    return code.substring(5);
  }
  if (code === "Space") return "SPACE";
  if (code === "Enter") return "ENTER";
  if (code === "Tab") return "TAB";
  return code.toUpperCase();
};

interface AISettings {
  provider: keyof typeof PROVIDERS;
  baseUrl: string;
  apiKey: string;
  model: string;
  systemPrompt: string;
  userPrompt: string;
}

interface HotkeyConfig {
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  win: boolean;
  key: string;
}

interface AppSettings {
  autoPaste: boolean;
  engine: "google" | "ai";
  ai: AISettings;
  shortcut: HotkeyConfig;
  tooltipShortcut: HotkeyConfig;
  tooltipDuration: number;
  tooltipTarget: string;
}

const getDefaultSettings = (): AppSettings => {
  return {
    autoPaste: false,
    engine: "google",
    ai: {
      provider: "openai",
      baseUrl: "https://api.openai.com/v1",
      apiKey: "",
      model: "gpt-4o-mini",
      systemPrompt: "You are a professional translator. Translate the text accurately to {target}. Output only the translation, no extra explanations.",
      userPrompt: "{text}"
    },
    shortcut: {
      ctrl: false,
      shift: false,
      alt: true,
      win: false,
      key: "KeyT"
    },
    tooltipShortcut: {
      ctrl: false,
      shift: true,
      alt: true,
      win: false,
      key: "KeyT"
    },
    tooltipDuration: 10,
    tooltipTarget: "vi"
  };
};

function App() {
  const [original, setOriginal] = useState("");
  const [target, setTarget] = useState("en");
  const [translation, setTranslation] = useState("");
  const [status, setStatus] = useState<"idle" | "loading" | "error">("idle");
  const [error, setError] = useState("");
  
  // Settings States
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<AppSettings>(getDefaultSettings);

  const appWindow = getCurrentWindow();
  const isTooltip = appWindow.label === "tooltip";

  useEffect(() => {
    if (isTooltip) {
      document.body.classList.add("transparent-bg");
    } else {
      document.body.classList.remove("transparent-bg");
    }
  }, [isTooltip]);

  // Load settings on startup from data.json next to executable
  useEffect(() => {
    async function loadStartupData() {
      try {
        const data = await invoke<any>("load_settings_rust");
        if (data && data !== null) {
          if (data.target) {
            setTarget(data.target);
          }
          if (data.settings) {
            setSettings({
              ...getDefaultSettings(),
              ...data.settings,
              shortcut: {
                ...getDefaultSettings().shortcut,
                ...(data.settings.shortcut || {})
              },
              tooltipShortcut: {
                ...getDefaultSettings().tooltipShortcut,
                ...(data.settings.tooltipShortcut || {})
              },
              tooltipDuration: data.settings.tooltipDuration !== undefined ? data.settings.tooltipDuration : 10,
              tooltipTarget: data.settings.tooltipTarget !== undefined ? data.settings.tooltipTarget : "vi"
            });
          }
        }
      } catch (e) {
        console.error("Failed to load settings from Rust", e);
      }
    }
    loadStartupData();
  }, []);

  async function saveAllData(newTarget: string, newSettings: AppSettings) {
    try {
      await invoke("save_settings", {
        settings: {
          target: newTarget,
          settings: newSettings
        }
      });
    } catch (e) {
      console.error("Failed to save settings to Rust", e);
    }
  }
  
  // Form States (for settings panel)
  const [formEngine, setFormEngine] = useState<"google" | "ai">("google");
  const [formAi, setFormAi] = useState<AISettings>({
    provider: "openai",
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    model: "gpt-4o-mini",
    systemPrompt: "",
    userPrompt: ""
  });
  const [formAutoPaste, setFormAutoPaste] = useState(false);
  const [formShortcut, setFormShortcut] = useState<HotkeyConfig>({
    ctrl: false,
    shift: false,
    alt: true,
    win: false,
    key: "KeyT"
  });
  const [formTooltipShortcut, setFormTooltipShortcut] = useState<HotkeyConfig>({
    ctrl: false,
    shift: true,
    alt: true,
    win: false,
    key: "KeyT"
  });
  const [formTooltipDuration, setFormTooltipDuration] = useState(10);
  const [formTooltipTarget, setFormTooltipTarget] = useState("vi");
  
  const [modelList, setModelList] = useState<string[]>([]);
  const [fetchingModels, setFetchingModels] = useState(false);
  
  // Bump on each request so a stale response can't overwrite a newer one.
  const reqId = useRef(0);

  async function runTranslate(text: string, tl: string, currentSettings: AppSettings = settings, shouldPaste: boolean = false) {
    const id = ++reqId.current;
    if (!text.trim()) {
      setTranslation("");
      setStatus("idle");
      return;
    }
    setStatus("loading");
    setError("");

    let detectedTl = tl;
    let cleanText = text;
    const match = text.match(/<([a-zA-Z]{2}(-[a-zA-Z]{2})?)\b/);
    if (match) {
      const code = match[1].toLowerCase();
      const foundLang = LANGS.find((l) => l.code.toLowerCase() === code);
      if (foundLang) {
        detectedTl = foundLang.code;
        cleanText = text.replace(match[0], "").replace(/[ \t]+/g, " ").trim();
      }
    }

    if (!cleanText.trim()) {
      setTranslation("");
      setStatus("idle");
      return;
    }

    try {
      let out = "";
      if (currentSettings.engine === "google") {
        out = await invoke<string>("translate", { text: cleanText, target: detectedTl });
      } else {
        const ai = currentSettings.ai;
        const targetLangName = LANGS.find((l) => l.code === detectedTl)?.name || detectedTl;
        
        // String substitutions without regex safety concerns
        const sysPrompt = ai.systemPrompt
          .split("{source}").join("auto-detect")
          .split("{target}").join(targetLangName)
          .split("{text}").join(cleanText);
          
        const userPrompt = ai.userPrompt
          .split("{source}").join("auto-detect")
          .split("{target}").join(targetLangName)
          .split("{text}").join(cleanText);

        const headers: Record<string, string> = {
          "Content-Type": "application/json",
        };
        if (ai.apiKey) {
          headers["Authorization"] = `Bearer ${ai.apiKey}`;
        }
        
        const body = {
          model: ai.model,
          messages: [
            { role: "system", content: sysPrompt },
            { role: "user", content: userPrompt }
          ],
          temperature: 0.3,
        };

        const url = `${ai.baseUrl}/chat/completions`;
        const responseJson = await invoke<any>("request_post", { url, headers, body });
        
        out = responseJson?.choices?.[0]?.message?.content || "";
        if (!out) {
          throw new Error("Empty response from AI provider");
        }
      }
      
      if (id !== reqId.current) return; // superseded
      const trimmedOut = out.trim();
      setTranslation(trimmedOut);
      setStatus("idle");
      if (shouldPaste && !isTooltip && currentSettings.autoPaste && trimmedOut) {
        await invoke("overwrite", { text: trimmedOut });
      }
    } catch (e) {
      if (id !== reqId.current) return;
      setError(String(e));
      setStatus("error");
    }
  }

  const [isListening, setIsListening] = useState(false);

  useEffect(() => {
    if (!isListening) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const code = e.code;
      if (
        [
          "ControlLeft", "ControlRight",
          "ShiftLeft", "ShiftRight",
          "AltLeft", "AltRight",
          "MetaLeft", "MetaRight"
        ].includes(code)
      ) {
        return;
      }

      setFormShortcut((prev) => ({ ...prev, key: code }));
      setIsListening(false);
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [isListening]);

  const [isListeningTooltip, setIsListeningTooltip] = useState(false);

  useEffect(() => {
    if (!isListeningTooltip) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const code = e.code;
      if (
        [
          "ControlLeft", "ControlRight",
          "ShiftLeft", "ShiftRight",
          "AltLeft", "AltRight",
          "MetaLeft", "MetaRight"
        ].includes(code)
      ) {
        return;
      }

      setFormTooltipShortcut((prev) => ({ ...prev, key: code }));
      setIsListeningTooltip(false);
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [isListeningTooltip]);

  // Hotkey capture event from Rust.
  useEffect(() => {
    const un = listen<string>("captured", (e) => {
      const text = e.payload || "";
      setOriginal(text);
      setTranslation("");
      runTranslate(text, target, settings, true);
    });
    return () => {
      un.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target, settings]);

  const timeoutRef = useRef<any>(null);

  // Hotkey capture event for tooltip.
  useEffect(() => {
    if (!isTooltip) return;
    const un = listen<string>("captured-tooltip", (e) => {
      const text = e.payload || "";
      setOriginal(text);
      setTranslation("");
      runTranslate(text, settings.tooltipTarget, settings);

      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      const duration = settings.tooltipDuration !== undefined ? settings.tooltipDuration : 10;
      if (duration > 0) {
        timeoutRef.current = setTimeout(() => {
          invoke("hide_window");
        }, duration * 1000);
      }
    });
    return () => {
      un.then((f) => f());
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.tooltipTarget, settings, isTooltip]);

  // Esc hides the window. Ctrl+V pastes and hides.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        invoke("hide_window");
      } else if (e.ctrlKey && (e.key === "v" || e.key === "V")) {
        const active = document.activeElement;
        const isInputFocused = active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA");
        if (isSettingsOpen || isInputFocused) {
          return; // Let standard paste happen in inputs or settings
        }
        e.preventDefault();
        onOverwrite();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [translation, isSettingsOpen]);

  function onTargetChange(code: string) {
    setTarget(code);
    saveAllData(code, settings);
    runTranslate(original, code, settings);
  }

  async function onOverwrite() {
    if (!translation) return;
    await invoke("overwrite", { text: translation });
  }

  async function onCopy() {
    if (!translation) return;
    await invoke("copy_text", { text: translation });
  }

  // Settings Handlers
  function handleOpenSettings() {
    const s = settings;
    setFormEngine(s.engine);
    setFormAi({ ...s.ai });
    setFormAutoPaste(s.autoPaste);
    setFormShortcut({
      ...getDefaultSettings().shortcut,
      ...(s.shortcut || {})
    });
    setFormTooltipShortcut({
      ...getDefaultSettings().tooltipShortcut,
      ...(s.tooltipShortcut || {})
    });
    setFormTooltipDuration(s.tooltipDuration !== undefined ? s.tooltipDuration : 10);
    setFormTooltipTarget(s.tooltipTarget !== undefined ? s.tooltipTarget : "vi");
    setModelList([]);
    setIsSettingsOpen(true);
  }

  function handleSave() {
    const newSettings: AppSettings = {
      autoPaste: formAutoPaste,
      engine: formEngine,
      ai: formAi,
      shortcut: formShortcut,
      tooltipShortcut: formTooltipShortcut,
      tooltipDuration: formTooltipDuration,
      tooltipTarget: formTooltipTarget
    };
    setSettings(newSettings);
    saveAllData(target, newSettings);
    setIsSettingsOpen(false);
    // Re-translate with new settings
    runTranslate(original, target, newSettings);
  }

  function handleCancel() {
    setIsSettingsOpen(false);
  }

  function handleProviderChange(provider: keyof typeof PROVIDERS) {
    const defaults = PROVIDERS[provider];
    setFormAi({
      ...formAi,
      provider,
      baseUrl: defaults.baseUrl,
      model: defaults.defaultModel
    });
    setModelList([]);
  }

  async function fetchModelList(baseUrl: string, apiKey: string) {
    if (!baseUrl) {
      alert("Base URL is required");
      return;
    }
    setFetchingModels(true);
    try {
      const headers: Record<string, string> = {};
      if (apiKey) {
        headers["Authorization"] = `Bearer ${apiKey}`;
      }
      const url = `${baseUrl}/models`;
      const responseJson = await invoke<any>("request_get", { url, headers });
      const models: any[] = responseJson?.data || [];
      const ids = models.map((m) => m.id).filter((id) => typeof id === "string");
      if (ids.length > 0) {
        setModelList(ids);
        alert(`Successfully fetched ${ids.length} models!`);
      } else {
        alert("No models found in the API response.");
      }
    } catch (e) {
      alert("Failed to fetch models: " + String(e));
    } finally {
      setFetchingModels(false);
    }
  }

  if (isSettingsOpen) {
    return (
      <main className="app">
        <div className="settings-view">
          <header className="settings-header">
            <button className="icon-btn" onClick={handleCancel} title="Back">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="19" y1="12" x2="5" y2="12"></line>
                <polyline points="12 19 5 12 12 5"></polyline>
              </svg>
            </button>
            <h2 className="settings-title">QuickTran Settings</h2>
          </header>

          <div className="settings-content">
            <div className="form-group checkbox-group">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={formAutoPaste}
                  onChange={(e) => setFormAutoPaste(e.target.checked)}
                />
                Auto-paste translation back
              </label>
            </div>

            <div className="form-group">
              <label>Global Hotkey (Main Window)</label>
              <div className="hotkey-selector">
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formShortcut.ctrl}
                    onChange={(e) => setFormShortcut({ ...formShortcut, ctrl: e.target.checked })}
                  />
                  Ctrl
                </label>
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formShortcut.shift}
                    onChange={(e) => setFormShortcut({ ...formShortcut, shift: e.target.checked })}
                  />
                  Shift
                </label>
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formShortcut.alt}
                    onChange={(e) => setFormShortcut({ ...formShortcut, alt: e.target.checked })}
                  />
                  Alt
                </label>
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formShortcut.win}
                    onChange={(e) => setFormShortcut({ ...formShortcut, win: e.target.checked })}
                  />
                  Win
                </label>
                <input
                  type="text"
                  readOnly
                  value={isListening ? "Press a key..." : formatKeyLabel(formShortcut.key)}
                  onFocus={() => setIsListening(true)}
                  onBlur={() => setIsListening(false)}
                  className="hotkey-key-input"
                  placeholder="Click to set"
                />
              </div>
            </div>

            <div className="form-group">
              <label>Global Hotkey (Tooltip Window)</label>
              <div className="hotkey-selector">
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formTooltipShortcut.ctrl}
                    onChange={(e) => setFormTooltipShortcut({ ...formTooltipShortcut, ctrl: e.target.checked })}
                  />
                  Ctrl
                </label>
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formTooltipShortcut.shift}
                    onChange={(e) => setFormTooltipShortcut({ ...formTooltipShortcut, shift: e.target.checked })}
                  />
                  Shift
                </label>
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formTooltipShortcut.alt}
                    onChange={(e) => setFormTooltipShortcut({ ...formTooltipShortcut, alt: e.target.checked })}
                  />
                  Alt
                </label>
                <label className="hotkey-checkbox">
                  <input
                    type="checkbox"
                    checked={formTooltipShortcut.win}
                    onChange={(e) => setFormTooltipShortcut({ ...formTooltipShortcut, win: e.target.checked })}
                  />
                  Win
                </label>
                <input
                  type="text"
                  readOnly
                  value={isListeningTooltip ? "Press a key..." : formatKeyLabel(formTooltipShortcut.key)}
                  onFocus={() => setIsListeningTooltip(true)}
                  onBlur={() => setIsListeningTooltip(false)}
                  className="hotkey-key-input"
                  placeholder="Click to set"
                />
              </div>
            </div>

            <div className="form-group">
              <label>Tooltip Auto-hide Duration (seconds, 0 to disable)</label>
              <input
                type="number"
                min="0"
                max="300"
                value={formTooltipDuration}
                onChange={(e) => setFormTooltipDuration(parseInt(e.target.value) || 0)}
              />
            </div>

            <div className="form-group">
              <label>Tooltip Target Language</label>
              <select
                value={formTooltipTarget}
                onChange={(e) => setFormTooltipTarget(e.target.value)}
              >
                {LANGS.map((l) => (
                  <option key={l.code} value={l.code}>
                    {l.name}
                  </option>
                ))}
              </select>
            </div>

            <div className="form-group">
              <label>Translation Engine</label>
              <select value={formEngine} onChange={(e) => setFormEngine(e.target.value as "google" | "ai")}>
                <option value="google">Google Translate (Free)</option>
                <option value="ai">AI (OpenAI-compatible)</option>
              </select>
            </div>

            {formEngine === "ai" && (
              <>
                <div className="form-group">
                  <label>Provider</label>
                  <select value={formAi.provider} onChange={(e) => handleProviderChange(e.target.value as keyof typeof PROVIDERS)}>
                    {Object.entries(PROVIDERS).map(([key, val]) => (
                      <option key={key} value={key as keyof typeof PROVIDERS}>
                        {val.name}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="form-group">
                  <label>API Key</label>
                  <input
                    type="password"
                    value={formAi.apiKey}
                    onChange={(e) => setFormAi({ ...formAi, apiKey: e.target.value })}
                    placeholder="Enter your API Key"
                  />
                </div>

                <div className="form-group">
                  <label>Base URL</label>
                  <input
                    type="text"
                    value={formAi.baseUrl}
                    onChange={(e) => setFormAi({ ...formAi, baseUrl: e.target.value })}
                    placeholder="e.g. https://api.openai.com/v1"
                  />
                </div>

                <div className="form-group">
                  <label>Model</label>
                  <div className="input-row">
                    {modelList.length === 0 ? (
                      <input
                        type="text"
                        value={formAi.model}
                        onChange={(e) => setFormAi({ ...formAi, model: e.target.value })}
                        placeholder="e.g. gpt-4o-mini"
                      />
                    ) : (
                      <select
                        value={formAi.model}
                        onChange={(e) => {
                          if (e.target.value === "__custom__") {
                            setModelList([]);
                          } else {
                            setFormAi({ ...formAi, model: e.target.value });
                          }
                        }}
                      >
                        {!modelList.includes(formAi.model) && (
                          <option value={formAi.model}>{formAi.model}</option>
                        )}
                        {modelList.map((m) => (
                          <option key={m} value={m}>
                            {m}
                          </option>
                        ))}
                        <option value="__custom__">✍️ Custom / Type manually...</option>
                      </select>
                    )}
                    <button onClick={() => fetchModelList(formAi.baseUrl, formAi.apiKey)} disabled={fetchingModels}>
                      {fetchingModels ? "..." : "Fetch"}
                    </button>
                  </div>
                </div>

                <div className="form-group">
                  <label>System Prompt</label>
                  <textarea
                    value={formAi.systemPrompt}
                    onChange={(e) => setFormAi({ ...formAi, systemPrompt: e.target.value })}
                    placeholder="System prompt instructions..."
                    rows={2}
                  />
                </div>

                <div className="form-group">
                  <label>User Prompt</label>
                  <textarea
                    value={formAi.userPrompt}
                    onChange={(e) => setFormAi({ ...formAi, userPrompt: e.target.value })}
                    placeholder="User prompt template..."
                    rows={2}
                  />
                </div>
              </>
            )}
          </div>

          <footer className="settings-footer">
            <button className="primary" onClick={handleSave}>Save</button>
            <button onClick={handleCancel}>Cancel</button>
          </footer>
        </div>
      </main>
    );
  }

  if (isTooltip) {
    return (
      <div className="tooltip-container">
        <header className="tooltip-header">
          <span className="tooltip-title">QuickTran</span>
          <button className="tooltip-close" onClick={() => invoke("hide_window")} title="Close">
            ✕
          </button>
        </header>
        <div className="tooltip-body">
          {status === "loading" ? (
            <div className="tooltip-loading">Translating…</div>
          ) : status === "error" ? (
            <div className="tooltip-error">{error}</div>
          ) : (
            <div className="tooltip-text">{translation || "—"}</div>
          )}
        </div>
        <div className="tooltip-footer">
          <span className="spark-icon">✦</span>
        </div>
      </div>
    );
  }

  return (
    <main className="app">
      <header className="bar">
        <span className="logo">⇄ QuickTran</span>
        <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
          <select
            value={target}
            onChange={(e) => onTargetChange(e.target.value)}
            title="Target language"
          >
            {LANGS.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
          <button className="icon-btn" onClick={handleOpenSettings} title="Settings">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3"></circle>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
            </svg>
          </button>
        </div>
      </header>

      <label className="lbl">Source</label>
      <textarea
        className="box"
        value={original}
        placeholder="Select text anywhere, then press Alt+T…"
        onChange={(e) => {
          setOriginal(e.target.value);
          runTranslate(e.target.value, target, settings);
        }}
      />

      <label className="lbl">
        Translation {status === "loading" && <span className="spin">…</span>}
      </label>
      <textarea
        className="box"
        value={status === "error" ? "" : translation}
        readOnly
        placeholder={status === "error" ? error : "—"}
      />

      <footer className="actions">
        <button className="primary" onClick={onOverwrite} disabled={!translation}>
          Overwrite
        </button>
        <button onClick={onCopy} disabled={!translation}>
          Copy
        </button>
        <button onClick={() => invoke("hide_window")}>Close</button>
      </footer>
    </main>
  );
}

export default App;
