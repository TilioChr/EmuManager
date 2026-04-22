import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ControllerMappingPanel from "./components/ControllerMappingPanel";
import LibraryPanel from "./components/LibraryPanel";
import RommConnectionCard from "./components/RommConnectionCard";
import { resolveGameDownloadUrl, type RommGame, type RommSession } from "./lib/romm";
import { buildPortablePaths } from "./lib/portableConfig";
import type {
  AppConfig,
  ConfigureResult,
  ControllerProfile,
  ControllerWriteResult,
  DownloadResult,
  EmulatorEntry,
  GameLaunchResult,
  InstallResult,
  LaunchResult,
  PortablePaths
} from "./types";

const fallbackPaths = buildPortablePaths("C:\\Users\\Tilio\\Documents\\EmuManager");

const initialPaths: PortablePaths = {
  ...fallbackPaths,
  config: `${fallbackPaths.root}\\Config`,
  data: `${fallbackPaths.root}\\Data`
};

export default function App() {
  const [paths, setPaths] = useState<PortablePaths>(initialPaths);
  const [emulators, setEmulators] = useState<EmulatorEntry[]>([]);
  const [controllerProfiles, setControllerProfiles] = useState<ControllerProfile[]>([]);
  const [selectedEmulatorId, setSelectedEmulatorId] = useState<string | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [config, setConfig] = useState<AppConfig>({ installedEmulators: [] });
  const [rommSession, setRommSession] = useState<RommSession | null>(null);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [configuringId, setConfiguringId] = useState<string | null>(null);
  const [downloadingGameId, setDownloadingGameId] = useState<string | null>(null);
  const [applyingProfileId, setApplyingProfileId] = useState<string | null>(null);
  const [lastDownloadedRomPath, setLastDownloadedRomPath] = useState<string | null>(null);

  useEffect(() => {
    const bootstrap = async () => {
      try {
        const portablePaths = await invoke<PortablePaths>("init_portable_layout", {
          root: fallbackPaths.root
        });
        setPaths(portablePaths);

        const savedConfig = await invoke<AppConfig>("load_app_config", {
          root: portablePaths.root
        });
        setConfig(savedConfig);

        const savedProfiles = await invoke<ControllerProfile[]>("load_controller_profiles_command", {
          root: portablePaths.root
        });
        setControllerProfiles(savedProfiles);

        const builtin = await invoke<Array<Omit<EmulatorEntry, "status" | "version">>>(
          "get_builtin_emulators"
        );

        const installedIds: string[] = [];
        for (const emu of builtin) {
          const isInstalled = await invoke<boolean>("check_emulator_installed", {
            root: portablePaths.root,
            emulatorId: emu.id
          });

          if (isInstalled) {
            installedIds.push(emu.id);
          }
        }

        const mergedInstalledIds = Array.from(
          new Set([...savedConfig.installedEmulators, ...installedIds])
        );

        const nextConfig: AppConfig = {
          ...savedConfig,
          installedEmulators: mergedInstalledIds
        };

        if (mergedInstalledIds.length !== savedConfig.installedEmulators.length) {
          await invoke("save_app_config", {
            root: portablePaths.root,
            config: nextConfig
          });
        }

        setConfig(nextConfig);

        const nextEmulators = builtin.map((emu) => ({
          ...emu,
          status: mergedInstalledIds.includes(emu.id) ? "installed" : "not_installed"
        }));

        setEmulators(nextEmulators);

        const firstInstalled = nextEmulators.find((entry) => entry.status === "installed");
        setSelectedEmulatorId(firstInstalled?.id ?? null);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      } finally {
        setLoading(false);
      }
    };

    void bootstrap();
  }, []);

  const installedCount = useMemo(
    () => emulators.filter((emu) => emu.status === "installed").length,
    [emulators]
  );

  const selectedEmulator =
    emulators.find((entry) => entry.id === selectedEmulatorId) ??
    emulators.find((entry) => entry.status === "installed") ??
    null;

  const persistConfig = async (nextConfig: AppConfig) => {
    await invoke("save_app_config", {
      root: paths.root,
      config: nextConfig
    });
    setConfig(nextConfig);
  };

  const persistControllerProfiles = async (profiles: ControllerProfile[]) => {
    await invoke("save_controller_profiles_command", {
      root: paths.root,
      profiles
    });
    setControllerProfiles(profiles);
  };

  const saveControllerProfile = async (profile: ControllerProfile) => {
    const nextProfiles = controllerProfiles.some((entry) => entry.id === profile.id)
      ? controllerProfiles.map((entry) => (entry.id === profile.id ? profile : entry))
      : [...controllerProfiles, profile];

    await persistControllerProfiles(nextProfiles);

    try {
      setApplyingProfileId(profile.id);
      const result = await invoke<ControllerWriteResult>("apply_controller_profile_command", {
        root: paths.root,
        profile
      });
      setActionMessage(`Profil appliqué à Dolphin : ${result.profilePath}`);
    } catch (reason) {
      setActionMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setApplyingProfileId(null);
    }
  };

  const removeInstalledFlag = async (id: string) => {
    const nextInstalledIds = config.installedEmulators.filter((entry) => entry !== id);
    const nextConfig: AppConfig = {
      ...config,
      installedEmulators: nextInstalledIds
    };

    await persistConfig(nextConfig);

    const nextEmulators = emulators.map((emu) =>
      emu.id === id
        ? {
            ...emu,
            status: "not_installed" as const,
            version: undefined
          }
        : emu
    );

    setEmulators(nextEmulators);

    if (selectedEmulatorId === id) {
      const replacement = nextEmulators.find((entry) => entry.status === "installed");
      setSelectedEmulatorId(replacement?.id ?? null);
    }

    setActionMessage(`Marquage retiré pour ${id}. Les fichiers installés n'ont pas été supprimés.`);
  };

  const configureSelectedEmulator = async (id: string) => {
    try {
      setConfiguringId(id);
      const result = await invoke<ConfigureResult>("configure_emulator_command", {
        root: paths.root,
        emulatorId: id
      });
      setActionMessage(`Configuration portable prête : ${result.userDirectory}`);
    } catch (reason) {
      setActionMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setConfiguringId(null);
    }
  };

  const installSelectedEmulator = async (id: string) => {
    if (id !== "dolphin") {
      setActionMessage(`${id} n'a pas encore d'installateur réel.`);
      return;
    }

    try {
      setInstallingId(id);
      setActionMessage("Téléchargement et extraction de Dolphin en cours...");

      const result = await invoke<InstallResult>("install_emulator_command", {
        root: paths.root,
        emulatorId: id
      });

      const nextInstalledIds = Array.from(new Set([...config.installedEmulators, id]));
      const nextConfig: AppConfig = {
        ...config,
        installedEmulators: nextInstalledIds
      };

      await persistConfig(nextConfig);

      const nextEmulators = emulators.map((emu) =>
        emu.id === id
          ? {
              ...emu,
              status: "installed" as const,
              version: "2603a"
            }
          : emu
      );

      setEmulators(nextEmulators);
      setSelectedEmulatorId(id);

      const configResult = await invoke<ConfigureResult>("configure_emulator_command", {
        root: paths.root,
        emulatorId: id
      });

      setActionMessage(
        `Dolphin installé dans ${result.installPath} et configuré en mode portable dans ${configResult.userDirectory}`
      );
    } catch (reason) {
      setActionMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setInstallingId(null);
    }
  };

  const launchSelectedEmulator = async (id: string) => {
    try {
      setLaunchingId(id);
      const result = await invoke<LaunchResult>("launch_emulator_command", {
        root: paths.root,
        emulatorId: id
      });
      setActionMessage(`Émulateur lancé depuis ${result.executablePath}`);
    } catch (reason) {
      setActionMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLaunchingId(null);
    }
  };

  const handleRommConnected = async (session: RommSession, username: string) => {
    setRommSession(session);

    const nextConfig: AppConfig = {
      ...config,
      romm: {
        baseUrl: session.baseUrl,
        username
      }
    };

    await persistConfig(nextConfig);
  };

  const handleDownloadGame = async (game: RommGame) => {
    if (!rommSession) {
      setActionMessage("Connexion RomM requise.");
      return;
    }

    if (selectedEmulatorId !== "dolphin") {
      setActionMessage("Pour l'instant, le téléchargement/lancement rapide vise Dolphin.");
      return;
    }

    const downloadUrl = resolveGameDownloadUrl(rommSession, game);
    if (!downloadUrl) {
      setActionMessage("URL de téléchargement RomM introuvable pour ce jeu.");
      return;
    }

    const targetFileName = game.file_name || `${game.name}.iso`;

    try {
      setDownloadingGameId(String(game.id));
      const result = await invoke<DownloadResult>("download_rom_command", {
        root: paths.root,
        url: downloadUrl,
        fileName: targetFileName
      });

      setLastDownloadedRomPath(result.filePath);
      setActionMessage(`ROM téléchargée dans ${result.filePath}`);
    } catch (reason) {
      setActionMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setDownloadingGameId(null);
    }
  };

  const launchLastDownloadedGame = async () => {
    if (!selectedEmulatorId || !lastDownloadedRomPath) {
      setActionMessage("Aucune ROM téléchargée prête à être lancée.");
      return;
    }

    try {
      const result = await invoke<GameLaunchResult>("launch_game_command", {
        root: paths.root,
        emulatorId: selectedEmulatorId,
        romPath: lastDownloadedRomPath
      });
      setActionMessage(`Jeu lancé avec ${result.emulatorId} : ${result.romPath}`);
    } catch (reason) {
      setActionMessage(reason instanceof Error ? reason.message : String(reason));
    }
  };

  if (loading) {
    return (
      <div className="center-screen">
        <div className="panel loading-panel">
          <p className="eyebrow">Initialisation</p>
          <h2>Préparation de l'environnement portable</h2>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="center-screen">
        <div className="panel loading-panel">
          <p className="eyebrow">Erreur</p>
          <h2>Impossible d'initialiser EmuManager</h2>
          <p>{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div>
          <p className="eyebrow">EmuManager</p>
          <h1>Émulateurs</h1>
          <p className="muted">{installedCount} installés</p>
        </div>

        <button className="primary-button" onClick={() => setShowPicker(true)}>
          Gérer la liste
        </button>

        <nav className="emulator-list">
          {emulators
            .filter((emu) => emu.status === "installed")
            .map((emu) => (
              <button
                key={emu.id}
                className={`emulator-item ${selectedEmulatorId === emu.id ? "emulator-item-active" : ""}`}
                onClick={() => setSelectedEmulatorId(emu.id)}
              >
                <span>{emu.name}</span>
                <small>{emu.platformLabel}</small>
              </button>
            ))}

          {installedCount === 0 && (
            <div className="empty-state">
              <p>Aucun émulateur installé</p>
              <small>Ajoute tes premiers émulateurs avec le bouton ci-dessus.</small>
            </div>
          )}
        </nav>
      </aside>

      <main className="content">
        <section className="panel hero-panel">
          <p className="eyebrow">Configuration portable</p>
          <h2>Dossiers racine</h2>
          <div className="path-grid">
            <PathCard label="Root" value={paths.root} />
            <PathCard label="Emu" value={paths.emu} />
            <PathCard label="Roms" value={paths.roms} />
            <PathCard label="Saves" value={paths.saves} />
            <PathCard label="Firmware" value={paths.firmware} />
            <PathCard label="Config" value={paths.config} />
            <PathCard label="Data" value={paths.data} />
          </div>
        </section>

        <section className="panel">
          <p className="eyebrow">Émulateur sélectionné</p>
          <h2>{selectedEmulator?.name ?? "Aucun émulateur"}</h2>
          {selectedEmulator ? (
            <div className="selected-emulator-grid">
              <StatusCard label="Plateforme" value={selectedEmulator.platformLabel} />
              <StatusCard label="Statut" value={selectedEmulator.status} />
              <StatusCard label="Version" value={selectedEmulator.version ?? "inconnue"} />
            </div>
          ) : (
            <p className="muted">Installe Dolphin pour commencer les tests réels.</p>
          )}
          <div className="selected-actions">
            <button
              className="primary-button"
              disabled={!selectedEmulator || launchingId === selectedEmulator.id}
              onClick={() => selectedEmulator && void launchSelectedEmulator(selectedEmulator.id)}
            >
              {selectedEmulator && launchingId === selectedEmulator.id
                ? "Lancement..."
                : "Ouvrir l'émulateur"}
            </button>
            <button
              className="primary-button compact-button"
              disabled={!selectedEmulator || configuringId === selectedEmulator.id}
              onClick={() => selectedEmulator && void configureSelectedEmulator(selectedEmulator.id)}
            >
              {selectedEmulator && configuringId === selectedEmulator.id
                ? "Configuration..."
                : "Configurer l'émulateur"}
            </button>
            <button
              className="primary-button compact-button"
              disabled={!selectedEmulator || !lastDownloadedRomPath}
              onClick={() => void launchLastDownloadedGame()}
            >
              Lancer la dernière ROM
            </button>
          </div>
          {lastDownloadedRomPath && <p className="muted last-rom">Dernière ROM : {lastDownloadedRomPath}</p>}
        </section>

        <ControllerMappingPanel
          selectedEmulator={selectedEmulator}
          profiles={controllerProfiles}
          onSaveProfile={saveControllerProfile}
        />
        {applyingProfileId && (
          <section className="panel">
            <p className="eyebrow">Manette</p>
            <h2>Application du profil</h2>
            <p className="muted">Écriture du profil {applyingProfileId} dans la configuration Dolphin...</p>
          </section>
        )}

        <RommConnectionCard
          defaultBaseUrl={config.romm?.baseUrl}
          defaultUsername={config.romm?.username}
          onConnected={handleRommConnected}
        />

        <LibraryPanel
          session={rommSession}
          onDownloadGame={handleDownloadGame}
          downloadingGameId={downloadingGameId}
        />

        <section className="panel">
          <p className="eyebrow">État</p>
          <h2>Statut actuel</h2>
          <div className="status-grid">
            <StatusCard
              label="RomM"
              value={rommSession ? "Connecté" : config.romm ? "Config enregistré" : "Non configuré"}
            />
            <StatusCard label="Émulateurs" value={`${installedCount} installés`} />
            <StatusCard label="Profils manette" value={`${controllerProfiles.length} enregistrés`} />
            <StatusCard label="Mode" value="Portable" />
          </div>
          {actionMessage && <p className="form-message success-message status-message">{actionMessage}</p>}
        </section>
      </main>

      {showPicker && (
        <div className="modal-backdrop" onClick={() => setShowPicker(false)}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <p className="eyebrow">Installation</p>
                <h3>Choisir les émulateurs</h3>
              </div>
              <button className="ghost-button" onClick={() => setShowPicker(false)}>
                Fermer
              </button>
            </div>

            <div className="picker-list">
              {emulators.map((emu) => {
                const isInstalling = installingId === emu.id;
                const isInstalled = emu.status === "installed";

                return (
                  <div key={emu.id} className="picker-item">
                    <div>
                      <strong>{emu.name}</strong>
                      <p>{emu.platformLabel}</p>
                    </div>
                    <button
                      className="primary-button"
                      disabled={isInstalling}
                      onClick={() =>
                        void (isInstalled ? removeInstalledFlag(emu.id) : installSelectedEmulator(emu.id))
                      }
                    >
                      {isInstalling
                        ? "Installation..."
                        : isInstalled
                          ? "Retirer"
                          : emu.id === "dolphin"
                            ? "Installer"
                            : "Bientôt"}
                    </button>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface PathCardProps {
  label: string;
  value: string;
}

function PathCard({ label, value }: PathCardProps) {
  return (
    <div className="path-card">
      <small>{label}</small>
      <code>{value}</code>
    </div>
  );
}

interface StatusCardProps {
  label: string;
  value: string;
}

function StatusCard({ label, value }: StatusCardProps) {
  return (
    <div className="path-card">
      <small>{label}</small>
      <strong>{value}</strong>
    </div>
  );
}