import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import type {
  EmulatorEntry,
  GraphicsPreset,
  GraphicsProfile,
  GraphicsProfileSaveResult
} from "../types";
import CollapsiblePanel from "./CollapsiblePanel";

interface GraphicsSettingsPanelProps {
  selectedEmulator: EmulatorEntry | null;
  profiles: GraphicsProfile[];
  onSaveProfile: (profile: GraphicsProfile) => Promise<GraphicsProfileSaveResult>;
}

interface GraphicsOption {
  id: string;
  label: string;
}

interface GraphicsCapabilities {
  graphicsApis: GraphicsOption[];
  maxResolutionScale: number;
  aspectRatios: GraphicsOption[];
  aspectRatioLabel: string;
  antiAliasing: GraphicsOption[];
  anisotropicFiltering: GraphicsOption[];
  textureFiltering: GraphicsOption[];
  showWidescreenHack: boolean;
  showIntegerScaling: boolean;
}

interface PresetDefinition {
  id: GraphicsPreset;
  label: string;
  description: string;
  resolutionScale: number;
  vsync: boolean;
  antiAliasing: string;
  anisotropicFiltering: string;
  textureFiltering: string;
  shaderCache: boolean;
  widescreenHack: boolean;
}

const graphicsApis = {
  vulkan: option("vulkan", "Vulkan"),
  opengl: option("opengl", "OpenGL"),
  direct3d11: option("direct3d11", "DirectX 11"),
  direct3d12: option("direct3d12", "DirectX 12"),
  software: option("software", "Software")
};

const aspectRatios = [
  option("auto", "Auto"),
  option("4:3", "4:3"),
  option("16:9", "16:9"),
  option("stretch", "Stretched")
];

const antiAliasingOptions = [
  option("off", "Off"),
  option("2x", "2x"),
  option("4x", "4x"),
  option("8x", "8x")
];

const shaderAntiAliasingOptions = [
  option("off", "Off"),
  option("fxaa", "FXAA"),
  option("smaa", "SMAA")
];

const anisotropicOptions = [
  option("off", "Off"),
  option("2x", "2x"),
  option("4x", "4x"),
  option("8x", "8x"),
  option("16x", "16x")
];

const textureFilteringOptions = [
  option("nearest", "Nearest"),
  option("linear", "Linear")
];

const pcsx2TextureFilteringOptions = [
  option("nearest", "Nearest"),
  option("linear", "PS2 / Linear"),
  option("forced", "Forced")
];

const edenScalingFilters = [
  option("nearest", "Nearest"),
  option("linear", "Bilinear"),
  option("bicubic", "Bicubic"),
  option("gaussian", "Gaussian"),
  option("lanczos", "Lanczos"),
  option("scaleforce", "ScaleForce"),
  option("fsr", "FSR")
];

const azaharTextureFilters = [
  option("linear", "Linear / Default"),
  option("anime4k", "Anime4K"),
  option("bicubic", "Bicubic"),
  option("scaleforce", "ScaleForce"),
  option("xbrz", "xBRZ / Enhanced"),
  option("mmpx", "MMPX")
];

const switchAspectRatios = [
  option("16:9", "16:9"),
  option("4:3", "4:3"),
  option("21:9", "21:9"),
  option("16:10", "16:10"),
  option("stretch", "Stretched")
];

const azaharLayouts = [
  option("auto", "Default"),
  option("single", "Single screen"),
  option("large", "Large screen"),
  option("side", "Side by side"),
  option("hybrid", "Hybrid")
];

const melondsAspectRatios = [
  option("4:3", "Native 4:3"),
  option("16:9", "16:9"),
  option("stretch", "Window stretch")
];

const presetCatalog: PresetDefinition[] = [
  {
    id: "native",
    label: "Native",
    description: "Original resolution, maximum compatibility.",
    resolutionScale: 1,
    vsync: false,
    antiAliasing: "off",
    anisotropicFiltering: "off",
    textureFiltering: "nearest",
    shaderCache: true,
    widescreenHack: false
  },
  {
    id: "greater",
    label: "Greater",
    description: "Clean upgrade with a modest performance cost.",
    resolutionScale: 2,
    vsync: true,
    antiAliasing: "off",
    anisotropicFiltering: "4x",
    textureFiltering: "linear",
    shaderCache: true,
    widescreenHack: false
  },
  {
    id: "beautiful",
    label: "Beautiful",
    description: "Sharper image and smoother edges.",
    resolutionScale: 3,
    vsync: true,
    antiAliasing: "2x",
    anisotropicFiltering: "8x",
    textureFiltering: "linear",
    shaderCache: true,
    widescreenHack: true
  },
  {
    id: "epic",
    label: "Epic",
    description: "High-end preset for powerful hardware.",
    resolutionScale: 4,
    vsync: true,
    antiAliasing: "4x",
    anisotropicFiltering: "16x",
    textureFiltering: "xbrz",
    shaderCache: true,
    widescreenHack: true
  }
];

const capabilityCatalog: Record<string, GraphicsCapabilities> = {
  dolphin: capabilities({
    graphicsApis: [graphicsApis.vulkan, graphicsApis.opengl, graphicsApis.direct3d12, graphicsApis.direct3d11],
    maxResolutionScale: 8,
    textureFiltering: textureFilteringOptions,
    showWidescreenHack: true
  }),
  melonds: capabilities({
    graphicsApis: [graphicsApis.opengl, graphicsApis.software],
    maxResolutionScale: 4,
    aspectRatios: melondsAspectRatios,
    antiAliasing: [antiAliasingOptions[0]],
    anisotropicFiltering: [anisotropicOptions[0]],
    textureFiltering: textureFilteringOptions,
    showIntegerScaling: true,
    showWidescreenHack: false
  }),
  azahar: capabilities({
    graphicsApis: [graphicsApis.vulkan, graphicsApis.opengl, graphicsApis.software],
    maxResolutionScale: 4,
    aspectRatios: azaharLayouts,
    aspectRatioLabel: "Screen layout",
    antiAliasing: [antiAliasingOptions[0]],
    anisotropicFiltering: [anisotropicOptions[0]],
    textureFiltering: azaharTextureFilters,
    showWidescreenHack: false
  }),
  eden: capabilities({
    graphicsApis: [graphicsApis.vulkan, graphicsApis.opengl],
    maxResolutionScale: 8,
    aspectRatios: switchAspectRatios,
    antiAliasing: shaderAntiAliasingOptions,
    textureFiltering: edenScalingFilters,
    showWidescreenHack: false
  }),
  pcsx2: capabilities({
    graphicsApis: [
      graphicsApis.vulkan,
      graphicsApis.direct3d12,
      graphicsApis.direct3d11,
      graphicsApis.opengl,
      graphicsApis.software
    ],
    maxResolutionScale: 8,
    antiAliasing: [option("off", "Off"), option("fxaa", "FXAA")],
    textureFiltering: pcsx2TextureFilteringOptions,
    showWidescreenHack: true
  })
};

const fallbackCapabilities = capabilities({
  graphicsApis: [graphicsApis.vulkan, graphicsApis.opengl],
  maxResolutionScale: 4
});

export default function GraphicsSettingsPanel({
  selectedEmulator,
  profiles,
  onSaveProfile
}: GraphicsSettingsPanelProps) {
  const capabilities = useMemo(
    () => (selectedEmulator ? getCapabilities(selectedEmulator.id) : fallbackCapabilities),
    [selectedEmulator]
  );
  const panelRef = useRef<HTMLDivElement | null>(null);
  const [draft, setDraft] = useState<GraphicsProfile | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const pendingScrollClampRef = useRef(false);

  useEffect(() => {
    if (!pendingScrollClampRef.current) {
      return;
    }

    pendingScrollClampRef.current = false;

    const frameId = window.requestAnimationFrame(() => {
      stabilizeAppScroll(panelRef.current);
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [draft?.mode]);

  useEffect(() => {
    if (!selectedEmulator) {
      setDraft(null);
      return;
    }

    const existing = profiles.find((profile) => profile.emulatorId === selectedEmulator.id);
    setDraft(existing ? normalizeProfile(existing, selectedEmulator, capabilities) : createDefaultProfile(selectedEmulator, capabilities));
    setMessage(null);
  }, [capabilities, profiles, selectedEmulator]);

  const handlePresetChange = (presetId: GraphicsPreset) => {
    if (!draft || !selectedEmulator) {
      return;
    }

    setDraft(applyPresetToProfile(draft, presetId, selectedEmulator, capabilities));
  };

  const handleAdvancedToggle = (advanced: boolean) => {
    if (!draft || !selectedEmulator) {
      return;
    }

    pendingScrollClampRef.current = true;

    setDraft((current) => {
      if (!current) {
        return current;
      }

      return advanced
        ? { ...current, mode: "advanced" }
        : applyPresetToProfile({ ...current, mode: "simple" }, current.preset, selectedEmulator, capabilities);
    });
  };

  const updateDraft = <K extends keyof GraphicsProfile>(key: K, value: GraphicsProfile[K]) => {
    setDraft((current) => (current ? { ...current, [key]: value } : current));
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!draft || !selectedEmulator) {
      return;
    }

    try {
      setSaving(true);
      setMessage(null);
      const profile = normalizeProfile(draft, selectedEmulator, capabilities);
      const result = await onSaveProfile(profile);
      setMessage(result.warning ?? "Profil graphique enregistre et applique.");
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : "Impossible d'enregistrer le profil graphique.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="graphics-panel-anchor" ref={panelRef}>
      <CollapsiblePanel eyebrow="Graphismes" defaultCollapsed>
        {!selectedEmulator || !draft ? (
          <p className="muted">Selectionne un emulateur pour preparer ses graphismes.</p>
        ) : (
          <form className="graphics-form" onSubmit={handleSubmit}>
            <div className="graphics-mode-row">
              <div>
                <strong>{selectedEmulator.name}</strong>
                <span>{selectedEmulator.platformLabel}</span>
              </div>
              <label className="mode-switch">
                <input
                  type="checkbox"
                  checked={draft.mode === "advanced"}
                  onChange={(event) => handleAdvancedToggle(event.target.checked)}
                />
                <span className="mode-switch-track" aria-hidden="true" />
                <span>Mode avance</span>
              </label>
            </div>

            {draft.mode === "simple" ? (
              <div className="graphics-preset-grid">
                {presetCatalog.map((preset) => (
                  <label
                    key={preset.id}
                    className={`graphics-preset-card ${
                      draft.preset === preset.id ? "graphics-preset-card-active" : ""
                    }`}
                  >
                    <input
                      type="checkbox"
                      checked={draft.preset === preset.id}
                      onChange={() => handlePresetChange(preset.id)}
                    />
                    <span>
                      <strong>{preset.label}</strong>
                      <small>{preset.description}</small>
                    </span>
                  </label>
                ))}
              </div>
            ) : (
              <div className="graphics-advanced-grid">
                <label className="field">
                  <span>Resolution</span>
                  <select
                    value={draft.resolutionScale}
                    onChange={(event) => updateDraft("resolutionScale", Number(event.target.value))}
                  >
                    {Array.from({ length: capabilities.maxResolutionScale }, (_, index) => index + 1).map((scale) => (
                      <option key={scale} value={scale}>
                        {scale === 1 ? "Native" : `x${scale}`}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span>Graphics API</span>
                  <select
                    value={draft.graphicsApi}
                    onChange={(event) => updateDraft("graphicsApi", event.target.value)}
                  >
                    {capabilities.graphicsApis.map((api) => (
                      <option key={api.id} value={api.id}>
                        {api.label}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span>{capabilities.aspectRatioLabel}</span>
                  <select
                    value={draft.aspectRatio}
                    onChange={(event) => updateDraft("aspectRatio", event.target.value)}
                  >
                    {capabilities.aspectRatios.map((ratio) => (
                      <option key={ratio.id} value={ratio.id}>
                        {ratio.label}
                      </option>
                    ))}
                  </select>
                </label>

                {capabilities.antiAliasing.length > 1 ? (
                  <label className="field">
                    <span>Anti-aliasing</span>
                    <select
                      value={draft.antiAliasing}
                      onChange={(event) => updateDraft("antiAliasing", event.target.value)}
                    >
                      {capabilities.antiAliasing.map((optionValue) => (
                        <option key={optionValue.id} value={optionValue.id}>
                          {optionValue.label}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : null}

                {capabilities.anisotropicFiltering.length > 1 ? (
                  <label className="field">
                    <span>Anisotropic filtering</span>
                    <select
                      value={draft.anisotropicFiltering}
                      onChange={(event) => updateDraft("anisotropicFiltering", event.target.value)}
                    >
                      {capabilities.anisotropicFiltering.map((optionValue) => (
                        <option key={optionValue.id} value={optionValue.id}>
                          {optionValue.label}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : null}

                <label className="field">
                  <span>Texture filtering</span>
                  <select
                    value={draft.textureFiltering}
                    onChange={(event) => updateDraft("textureFiltering", event.target.value)}
                  >
                    {capabilities.textureFiltering.map((optionValue) => (
                      <option key={optionValue.id} value={optionValue.id}>
                        {optionValue.label}
                      </option>
                    ))}
                  </select>
                </label>

                <div className="graphics-toggle-grid">
                  <label className="toggle-field">
                    <input
                      type="checkbox"
                      checked={draft.fullscreen}
                      onChange={(event) => updateDraft("fullscreen", event.target.checked)}
                    />
                    <span>Fullscreen on launch</span>
                  </label>
                  <label className="toggle-field">
                    <input
                      type="checkbox"
                      checked={draft.vsync}
                      onChange={(event) => updateDraft("vsync", event.target.checked)}
                    />
                    <span>VSync</span>
                  </label>
                  <label className="toggle-field">
                    <input
                      type="checkbox"
                      checked={draft.shaderCache}
                      onChange={(event) => updateDraft("shaderCache", event.target.checked)}
                    />
                    <span>Shader cache</span>
                  </label>
                  {capabilities.showWidescreenHack ? (
                    <label className="toggle-field">
                      <input
                        type="checkbox"
                        checked={draft.widescreenHack}
                        onChange={(event) => updateDraft("widescreenHack", event.target.checked)}
                      />
                      <span>Widescreen hack</span>
                    </label>
                  ) : null}
                  {capabilities.showIntegerScaling ? (
                    <label className="toggle-field">
                      <input
                        type="checkbox"
                        checked={draft.integerScaling}
                        onChange={(event) => updateDraft("integerScaling", event.target.checked)}
                      />
                      <span>Integer scaling</span>
                    </label>
                  ) : null}
                </div>
              </div>
            )}

            <div className="graphics-actions">
              <button className="primary-button compact-button" type="submit" disabled={saving}>
                {saving ? "Enregistrement..." : "Enregistrer les graphismes"}
              </button>
            </div>

            {message ? (
              <p
                className={`form-message status-message ${
                  message.includes("Impossible") || message.includes("non applique")
                    ? "error-message"
                    : "success-message"
                }`}
              >
                {message}
              </p>
            ) : null}
          </form>
        )}
      </CollapsiblePanel>
    </div>
  );

}

function createDefaultProfile(emulator: EmulatorEntry, capabilities: GraphicsCapabilities): GraphicsProfile {
  return applyPresetToProfile(
    {
      id: createGraphicsProfileId(emulator.id),
      emulatorId: emulator.id,
      platformLabel: emulator.platformLabel,
      mode: "simple",
      preset: "greater",
      resolutionScale: 2,
      graphicsApi: capabilities.graphicsApis[0]?.id ?? "vulkan",
      fullscreen: false,
      vsync: true,
      aspectRatio: "auto",
      antiAliasing: "off",
      anisotropicFiltering: "4x",
      textureFiltering: "linear",
      shaderCache: true,
      widescreenHack: false,
      integerScaling: false
    },
    "greater",
    emulator,
    capabilities
  );
}

function applyPresetToProfile(
  profile: GraphicsProfile,
  presetId: GraphicsPreset,
  emulator: EmulatorEntry,
  capabilities: GraphicsCapabilities
): GraphicsProfile {
  const preset = presetCatalog.find((entry) => entry.id === presetId) ?? presetCatalog[1];

  return normalizeProfile(
    {
      ...profile,
      id: createGraphicsProfileId(emulator.id),
      emulatorId: emulator.id,
      platformLabel: emulator.platformLabel,
      preset: preset.id,
      resolutionScale: preset.resolutionScale,
      vsync: preset.vsync,
      antiAliasing: preset.antiAliasing,
      anisotropicFiltering: preset.anisotropicFiltering,
      textureFiltering: preset.textureFiltering,
      shaderCache: preset.shaderCache,
      widescreenHack: preset.widescreenHack
    },
    emulator,
    capabilities
  );
}

function normalizeProfile(
  profile: GraphicsProfile,
  emulator: EmulatorEntry,
  capabilities: GraphicsCapabilities
): GraphicsProfile {
  return {
    ...profile,
    id: createGraphicsProfileId(emulator.id),
    emulatorId: emulator.id,
    platformLabel: emulator.platformLabel,
    resolutionScale: clamp(profile.resolutionScale, 1, capabilities.maxResolutionScale),
    graphicsApi: optionExists(capabilities.graphicsApis, profile.graphicsApi)
      ? profile.graphicsApi
      : capabilities.graphicsApis[0]?.id ?? "vulkan",
    aspectRatio: optionExists(capabilities.aspectRatios, profile.aspectRatio)
      ? profile.aspectRatio
      : capabilities.aspectRatios[0]?.id ?? "auto",
    antiAliasing: resolveAntiAliasing(profile.antiAliasing, capabilities.antiAliasing),
    anisotropicFiltering: optionExists(capabilities.anisotropicFiltering, profile.anisotropicFiltering)
      ? profile.anisotropicFiltering
      : "off",
    textureFiltering: resolveTextureFiltering(profile.textureFiltering, capabilities.textureFiltering),
    widescreenHack: capabilities.showWidescreenHack ? profile.widescreenHack : false,
    integerScaling: capabilities.showIntegerScaling ? profile.integerScaling : false
  };
}

function getCapabilities(emulatorId: string) {
  return capabilityCatalog[emulatorId] ?? fallbackCapabilities;
}

function capabilities(overrides: Partial<GraphicsCapabilities>): GraphicsCapabilities {
  return {
    graphicsApis: [graphicsApis.vulkan, graphicsApis.opengl],
    maxResolutionScale: 4,
    aspectRatios,
    aspectRatioLabel: "Aspect ratio",
    antiAliasing: antiAliasingOptions,
    anisotropicFiltering: anisotropicOptions,
    textureFiltering: textureFilteringOptions,
    showWidescreenHack: true,
    showIntegerScaling: false,
    ...overrides
  };
}

function option(id: string, label: string): GraphicsOption {
  return { id, label };
}

function optionExists(options: GraphicsOption[], value: string) {
  return options.some((optionValue) => optionValue.id === value);
}

function stabilizeAppScroll(panel: HTMLElement | null) {
  const container = panel?.closest(".window-content");
  if (container instanceof HTMLElement) {
    clampScrollTop(container);
  }

  window.scrollTo(0, 0);
  document.documentElement.scrollTop = 0;
  document.body.scrollTop = 0;
}

function clampScrollTop(container: HTMLElement) {
  const maxScrollTop = Math.max(0, container.scrollHeight - container.clientHeight);
  if (container.scrollTop > maxScrollTop) {
    container.scrollTop = maxScrollTop;
  }
}

function resolveAntiAliasing(value: string, options: GraphicsOption[]) {
  if (optionExists(options, value)) {
    return value;
  }

  if (value !== "off" && optionExists(options, "fxaa")) {
    return "fxaa";
  }

  return options[0]?.id ?? "off";
}

function resolveTextureFiltering(value: string, options: GraphicsOption[]) {
  if (optionExists(options, value)) {
    return value;
  }

  for (const fallback of ["linear", "fsr", "bicubic", "nearest"]) {
    if (optionExists(options, fallback)) {
      return fallback;
    }
  }

  return options[0]?.id ?? "linear";
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? value : min));
}

function createGraphicsProfileId(emulatorId: string) {
  return `graphics-${emulatorId}`.replace(/[^a-z0-9_-]+/gi, "-").toLowerCase();
}
