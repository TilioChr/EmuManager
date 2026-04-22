import { FormEvent, useEffect, useMemo, useState } from "react";
import type { ControllerBinding, ControllerProfile, EmulatorEntry } from "../types";
import CollapsiblePanel from "./CollapsiblePanel";

interface ControllerMappingPanelProps {
  selectedEmulator: EmulatorEntry | null;
  profiles: ControllerProfile[];
  onSaveProfile: (profile: ControllerProfile) => Promise<void>;
}

const defaultBindings: ControllerBinding[] = [
  { physicalInput: "A", emulatedInput: "Bouton A" },
  { physicalInput: "B", emulatedInput: "Bouton B" },
  { physicalInput: "X", emulatedInput: "Bouton X" },
  { physicalInput: "Y", emulatedInput: "Bouton Y" },
  { physicalInput: "LB", emulatedInput: "Z" },
  { physicalInput: "Start", emulatedInput: "Start" }
];

export default function ControllerMappingPanel({
  selectedEmulator,
  profiles,
  onSaveProfile
}: ControllerMappingPanelProps) {
  const existingProfile = useMemo(() => {
    if (!selectedEmulator) {
      return null;
    }

    return profiles.find((profile) => profile.emulatorId === selectedEmulator.id) ?? null;
  }, [profiles, selectedEmulator]);

  const [profileName, setProfileName] = useState("");
  const [physicalDeviceLabel, setPhysicalDeviceLabel] = useState("Manette Xbox");
  const [emulatedDeviceLabel, setEmulatedDeviceLabel] = useState("Manette Wii + Nunchuk");
  const [bindings, setBindings] = useState<ControllerBinding[]>(defaultBindings);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedEmulator) {
      return;
    }

    if (existingProfile) {
      setProfileName(existingProfile.name);
      setPhysicalDeviceLabel(existingProfile.physicalDeviceLabel);
      setEmulatedDeviceLabel(existingProfile.emulatedDeviceLabel);
      setBindings(existingProfile.bindings);
      setMessage(null);
      return;
    }

    setProfileName(`${selectedEmulator.name} - Profil 1`);
    setPhysicalDeviceLabel("Manette Xbox");
    setEmulatedDeviceLabel(
      selectedEmulator.id === "dolphin" ? "Manette Wii + Nunchuk" : "Manette émulée"
    );
    setBindings(defaultBindings);
    setMessage(null);
  }, [existingProfile, selectedEmulator]);

  const updateBinding = (index: number, key: keyof ControllerBinding, value: string) => {
    setBindings((current) =>
      current.map((binding, bindingIndex) =>
        bindingIndex === index
          ? {
              ...binding,
              [key]: value
            }
          : binding
      )
    );
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedEmulator) {
      return;
    }

    try {
      setSaving(true);
      setMessage(null);

      const profile: ControllerProfile = {
        id: existingProfile?.id ?? `${selectedEmulator.id}-default`,
        name: profileName,
        emulatorId: selectedEmulator.id,
        platformLabel: selectedEmulator.platformLabel,
        physicalDeviceLabel,
        emulatedDeviceLabel,
        bindings
      };

      await onSaveProfile(profile);
      setMessage("Profil manette enregistré.");
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : "Impossible d'enregistrer le profil.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <CollapsiblePanel eyebrow="Manette" title="Mapping simplifié" defaultCollapsed>
      {!selectedEmulator && <p className="muted">Sélectionne un émulateur pour préparer un profil.</p>}

      {selectedEmulator && (
        <form className="mapping-form" onSubmit={handleSubmit}>
          <div className="mapping-grid">
            <label className="field">
              <span>Nom du profil</span>
              <input value={profileName} onChange={(event) => setProfileName(event.target.value)} />
            </label>
            <label className="field">
              <span>Manette physique</span>
              <input
                value={physicalDeviceLabel}
                onChange={(event) => setPhysicalDeviceLabel(event.target.value)}
              />
            </label>
            <label className="field field-full">
              <span>Manette émulée</span>
              <input
                value={emulatedDeviceLabel}
                onChange={(event) => setEmulatedDeviceLabel(event.target.value)}
              />
            </label>
          </div>

          <div className="mapping-columns">
            <div className="mapping-visual-card">
              <small>Manette physique</small>
              <div className="controller-visual">🎮 {physicalDeviceLabel}</div>
            </div>
            <div className="mapping-visual-card">
              <small>Manette émulée</small>
              <div className="controller-visual">🕹️ {emulatedDeviceLabel}</div>
            </div>
          </div>

          <div className="bindings-list">
            {bindings.map((binding, index) => (
              <div key={`${binding.physicalInput}-${index}`} className="binding-row">
                <input
                  value={binding.physicalInput}
                  onChange={(event) => updateBinding(index, "physicalInput", event.target.value)}
                />
                <span className="binding-arrow">→</span>
                <input
                  value={binding.emulatedInput}
                  onChange={(event) => updateBinding(index, "emulatedInput", event.target.value)}
                />
              </div>
            ))}
          </div>

          <div className="selected-actions">
            <button className="primary-button compact-button" type="submit" disabled={saving}>
              {saving ? "Enregistrement..." : "Enregistrer le profil"}
            </button>
          </div>

          {message && <p className="form-message success-message status-message">{message}</p>}
        </form>
      )}
    </CollapsiblePanel>
  );
}