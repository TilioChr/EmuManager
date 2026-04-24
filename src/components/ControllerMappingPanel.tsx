import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ControllerBinding,
  ControllerDolphinSettings,
  ControllerProfile,
  ControllerProfileSaveResult,
  EmulatorEntry
} from "../types";
import CollapsiblePanel from "./CollapsiblePanel";

interface ControllerMappingPanelProps {
  selectedEmulator: EmulatorEntry | null;
  profiles: ControllerProfile[];
  onSaveProfile: (profile: ControllerProfile) => Promise<ControllerProfileSaveResult>;
}

type PhysicalDeviceType = "keyboard" | "gamepad" | "saved";

interface PhysicalDevice {
  id: string;
  label: string;
  type: PhysicalDeviceType;
  index?: number;
}

interface EmulatedInputDefinition {
  id: string;
  label: string;
  shortLabel: string;
  x: number;
  y: number;
}

interface EmulatedControllerDefinition {
  id: string;
  label: string;
  description: string;
  inputs: EmulatedInputDefinition[];
}

const keyboardDevice: PhysicalDevice = {
  id: "keyboard",
  label: "Clavier / souris",
  type: "keyboard"
};

const defaultDolphinSettings: ControllerDolphinSettings = {
  irAutoHide: true,
  irRelativeInput: true
};

const standardGamepadButtons = [
  "A",
  "B",
  "X",
  "Y",
  "LB",
  "RB",
  "Left Trigger",
  "Right Trigger",
  "Select",
  "Start",
  "Left Stick",
  "Right Stick",
  "DPad Up",
  "DPad Down",
  "DPad Left",
  "DPad Right",
  "Home"
];

const gamepadAxes = [
  { axis: 0, negative: "Left Stick Left", positive: "Left Stick Right" },
  { axis: 1, negative: "Left Stick Up", positive: "Left Stick Down" },
  { axis: 2, negative: "Right Stick Left", positive: "Right Stick Right" },
  { axis: 3, negative: "Right Stick Up", positive: "Right Stick Down" }
];

const gamecubeInputs: EmulatedInputDefinition[] = [
  input("gc_stick_up", "Stick Haut", "L^", 24, 28),
  input("gc_stick_left", "Stick Gauche", "L<", 16, 42),
  input("gc_stick_right", "Stick Droite", "L>", 32, 42),
  input("gc_stick_down", "Stick Bas", "Lv", 24, 56),
  input("gc_dpad_up", "Croix Haut", "D^", 18, 68),
  input("gc_dpad_left", "Croix Gauche", "D<", 10, 80),
  input("gc_dpad_right", "Croix Droite", "D>", 26, 80),
  input("gc_dpad_down", "Croix Bas", "Dv", 18, 92),
  input("gc_start", "Start", "S", 50, 48),
  input("gc_z", "Z", "Z", 68, 18),
  input("gc_l", "L", "L", 24, 13),
  input("gc_r", "R", "R", 76, 13),
  input("gc_y", "Bouton Y", "Y", 75, 34),
  input("gc_x", "Bouton X", "X", 88, 40),
  input("gc_b", "Bouton B", "B", 70, 54),
  input("gc_a", "Bouton A", "A", 82, 60),
  input("gc_c_up", "C Haut", "C^", 60, 64),
  input("gc_c_left", "C Gauche", "C<", 52, 76),
  input("gc_c_right", "C Droite", "C>", 68, 76),
  input("gc_c_down", "C Bas", "Cv", 60, 88)
];

const wiimoteInputs: EmulatedInputDefinition[] = [
  input("wm_dpad_up", "Croix Haut", "D^", 24, 20),
  input("wm_dpad_left", "Croix Gauche", "D<", 16, 31),
  input("wm_dpad_right", "Croix Droite", "D>", 32, 31),
  input("wm_dpad_down", "Croix Bas", "Dv", 24, 42),
  input("wm_a", "Bouton A", "A", 50, 36),
  input("wm_b", "Bouton B", "B", 50, 62),
  input("wm_minus", "Minus", "-", 39, 50),
  input("wm_plus", "Plus", "+", 61, 50),
  input("wm_home", "Home", "H", 50, 50),
  input("wm_one", "Bouton 1", "1", 44, 78),
  input("wm_two", "Bouton 2", "2", 56, 78),
  input("wm_ir_up", "IR Haut", "I^", 76, 20),
  input("wm_ir_left", "IR Gauche", "I<", 68, 32),
  input("wm_ir_right", "IR Droite", "I>", 84, 32),
  input("wm_ir_down", "IR Bas", "Iv", 76, 44),
  input("wm_ir_recenter", "IR Recentrer", "IR", 86, 56),
  input("wm_shake_x", "Secouer X", "SX", 76, 70),
  input("wm_shake_y", "Secouer Y", "SY", 84, 82),
  input("wm_shake_z", "Secouer Z", "SZ", 68, 82)
];

const wiimoteNunchukInputs: EmulatedInputDefinition[] = [
  ...wiimoteInputs,
  input("nunchuk_up", "Nunchuk Haut", "N^", 78, 70),
  input("nunchuk_left", "Nunchuk Gauche", "N<", 70, 84),
  input("nunchuk_right", "Nunchuk Droite", "N>", 86, 84),
  input("nunchuk_down", "Nunchuk Bas", "Nv", 78, 96),
  input("nunchuk_c", "Nunchuk C", "C", 91, 20),
  input("nunchuk_z", "Nunchuk Z", "Z", 91, 38)
];

const classicInputs: EmulatedInputDefinition[] = [
  input("cc_l", "L", "L", 24, 14),
  input("cc_r", "R", "R", 76, 14),
  input("cc_zl", "ZL", "ZL", 12, 20),
  input("cc_zr", "ZR", "ZR", 88, 20),
  input("cc_dpad_up", "Croix Haut", "D^", 23, 39),
  input("cc_dpad_left", "Croix Gauche", "D<", 15, 51),
  input("cc_dpad_right", "Croix Droite", "D>", 31, 51),
  input("cc_dpad_down", "Croix Bas", "Dv", 23, 63),
  input("cc_stick_up", "Stick Haut", "L^", 39, 65),
  input("cc_stick_left", "Stick Gauche", "L<", 31, 77),
  input("cc_stick_right", "Stick Droite", "L>", 47, 77),
  input("cc_stick_down", "Stick Bas", "Lv", 39, 89),
  input("cc_minus", "Minus", "-", 43, 47),
  input("cc_plus", "Plus", "+", 57, 47),
  input("cc_home", "Home", "H", 50, 56),
  input("cc_y", "Bouton Y", "Y", 72, 40),
  input("cc_x", "Bouton X", "X", 84, 45),
  input("cc_b", "Bouton B", "B", 70, 58),
  input("cc_a", "Bouton A", "A", 82, 64),
  input("cc_rs_up", "Stick Droit Haut", "R^", 62, 68),
  input("cc_rs_left", "Stick Droit Gauche", "R<", 54, 80),
  input("cc_rs_right", "Stick Droit Droite", "R>", 70, 80),
  input("cc_rs_down", "Stick Droit Bas", "Rv", 62, 92)
];

const switchInputs: EmulatedInputDefinition[] = [
  input("sw_l", "L", "L", 20, 14),
  input("sw_r", "R", "R", 80, 14),
  input("sw_zl", "ZL", "ZL", 11, 22),
  input("sw_zr", "ZR", "ZR", 89, 22),
  input("sw_left_up", "Stick Haut", "L^", 24, 35),
  input("sw_left_left", "Stick Gauche", "L<", 16, 47),
  input("sw_left_right", "Stick Droite", "L>", 32, 47),
  input("sw_left_down", "Stick Bas", "Lv", 24, 59),
  input("sw_dpad_up", "Croix Haut", "D^", 33, 68),
  input("sw_dpad_left", "Croix Gauche", "D<", 25, 80),
  input("sw_dpad_right", "Croix Droite", "D>", 41, 80),
  input("sw_dpad_down", "Croix Bas", "Dv", 33, 92),
  input("sw_minus", "Minus", "-", 42, 45),
  input("sw_plus", "Plus", "+", 58, 45),
  input("sw_home", "Home", "H", 58, 57),
  input("sw_capture", "Capture", "C", 42, 57),
  input("sw_y", "Bouton Y", "Y", 70, 34),
  input("sw_x", "Bouton X", "X", 82, 40),
  input("sw_b", "Bouton B", "B", 70, 54),
  input("sw_a", "Bouton A", "A", 82, 60),
  input("sw_right_up", "Stick Droit Haut", "R^", 66, 68),
  input("sw_right_left", "Stick Droit Gauche", "R<", 58, 80),
  input("sw_right_right", "Stick Droit Droite", "R>", 74, 80),
  input("sw_right_down", "Stick Droit Bas", "Rv", 66, 92)
];

const dualshockInputs: EmulatedInputDefinition[] = [
  input("ps_l1", "L1", "L1", 22, 14),
  input("ps_r1", "R1", "R1", 78, 14),
  input("ps_l2", "L2", "L2", 12, 21),
  input("ps_r2", "R2", "R2", 88, 21),
  input("ps_left_up", "Stick Haut", "L^", 27, 39),
  input("ps_left_left", "Stick Gauche", "L<", 19, 51),
  input("ps_left_right", "Stick Droite", "L>", 35, 51),
  input("ps_left_down", "Stick Bas", "Lv", 27, 63),
  input("ps_dpad_up", "Croix Haut", "D^", 19, 65),
  input("ps_dpad_left", "Croix Gauche", "D<", 11, 77),
  input("ps_dpad_right", "Croix Droite", "D>", 27, 77),
  input("ps_dpad_down", "Croix Bas", "Dv", 19, 89),
  input("ps_select", "Select", "Se", 42, 48),
  input("ps_start", "Start", "St", 58, 48),
  input("ps_square", "Carre", "Sq", 71, 41),
  input("ps_triangle", "Triangle", "Tr", 83, 35),
  input("ps_cross", "Croix", "X", 83, 61),
  input("ps_circle", "Rond", "O", 93, 50),
  input("ps_right_up", "Stick Droit Haut", "R^", 62, 66),
  input("ps_right_left", "Stick Droit Gauche", "R<", 54, 78),
  input("ps_right_right", "Stick Droit Droite", "R>", 70, 78),
  input("ps_right_down", "Stick Droit Bas", "Rv", 62, 90)
];

const compactNintendoInputs: EmulatedInputDefinition[] = [
  input("nds_dpad_up", "Croix Haut", "D^", 22, 34),
  input("nds_dpad_left", "Croix Gauche", "D<", 14, 46),
  input("nds_dpad_right", "Croix Droite", "D>", 30, 46),
  input("nds_dpad_down", "Croix Bas", "Dv", 22, 58),
  input("nds_l", "L", "L", 22, 16),
  input("nds_r", "R", "R", 78, 16),
  input("nds_select", "Select", "Se", 42, 49),
  input("nds_start", "Start", "St", 58, 49),
  input("nds_y", "Bouton Y", "Y", 72, 38),
  input("nds_x", "Bouton X", "X", 84, 44),
  input("nds_b", "Bouton B", "B", 72, 58),
  input("nds_a", "Bouton A", "A", 84, 64)
];

const azaharInputs: EmulatedInputDefinition[] = [
  input("3ds_circle_up", "Stick Haut", "L^", 24, 31),
  input("3ds_circle_left", "Stick Gauche", "L<", 16, 43),
  input("3ds_circle_right", "Stick Droite", "L>", 32, 43),
  input("3ds_circle_down", "Stick Bas", "Lv", 24, 55),
  input("3ds_dpad_up", "Croix Haut", "D^", 22, 66),
  input("3ds_dpad_left", "Croix Gauche", "D<", 14, 78),
  input("3ds_dpad_right", "Croix Droite", "D>", 30, 78),
  input("3ds_dpad_down", "Croix Bas", "Dv", 22, 90),
  input("3ds_l", "L", "L", 22, 15),
  input("3ds_r", "R", "R", 78, 15),
  input("3ds_zl", "ZL", "ZL", 12, 22),
  input("3ds_zr", "ZR", "ZR", 88, 22),
  input("3ds_select", "Select", "Se", 42, 50),
  input("3ds_start", "Start", "St", 58, 50),
  input("3ds_home", "Home", "H", 50, 63),
  input("3ds_y", "Bouton Y", "Y", 72, 38),
  input("3ds_x", "Bouton X", "X", 84, 44),
  input("3ds_b", "Bouton B", "B", 72, 58),
  input("3ds_a", "Bouton A", "A", 84, 64),
  input("3ds_c_up", "C-Stick Haut", "C^", 64, 69),
  input("3ds_c_left", "C-Stick Gauche", "C<", 56, 81),
  input("3ds_c_right", "C-Stick Droite", "C>", 72, 81),
  input("3ds_c_down", "C-Stick Bas", "Cv", 64, 93)
];

const controllerCatalog: Record<string, EmulatedControllerDefinition[]> = {
  dolphin: [
    controller("gamecube", "GameCube controller", "Ports manette GameCube de Dolphin", gamecubeInputs),
    controller("wiimote", "Wiimote", "Wiimote seule", wiimoteInputs),
    controller("wiimote_nunchuk", "Wiimote + Nunchuk", "Wiimote avec extension Nunchuk", wiimoteNunchukInputs),
    controller("classic_controller", "Wii Classic Controller", "Extension Classic Controller", classicInputs)
  ],
  eden: [
    controller("switch_pro", "Pro Controller", "Manette Switch Pro", switchInputs),
    controller("joycon_pair", "Joy-Con pair", "Deux Joy-Con en mode horizontal", switchInputs)
  ],
  pcsx2: [
    controller("dualshock2", "DualShock 2", "Manette PlayStation 2", dualshockInputs)
  ],
  melonds: [
    controller("nds", "Nintendo DS controls", "Boutons Nintendo DS", compactNintendoInputs)
  ],
  azahar: [
    controller("3ds", "Nintendo 3DS controls", "Boutons Nintendo 3DS", azaharInputs)
  ]
};

export default function ControllerMappingPanel({
  selectedEmulator,
  profiles,
  onSaveProfile
}: ControllerMappingPanelProps) {
  const [physicalDevices, setPhysicalDevices] = useState<PhysicalDevice[]>([keyboardDevice]);
  const [selectedPhysicalDeviceId, setSelectedPhysicalDeviceId] = useState(keyboardDevice.id);
  const [savedPhysicalDeviceLabel, setSavedPhysicalDeviceLabel] = useState(keyboardDevice.label);
  const [emulatedControllerId, setEmulatedControllerId] = useState("");
  const [profileName, setProfileName] = useState("");
  const [bindings, setBindings] = useState<ControllerBinding[]>([]);
  const [dolphinSettings, setDolphinSettings] =
    useState<ControllerDolphinSettings>(defaultDolphinSettings);
  const [listeningInputId, setListeningInputId] = useState<string | null>(null);
  const [scanningDevices, setScanningDevices] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const baselineSignalsRef = useRef<Set<string>>(new Set());

  const compatibleControllers = useMemo(
    () => (selectedEmulator ? getCompatibleControllers(selectedEmulator.id) : []),
    [selectedEmulator]
  );

  const selectedController =
    compatibleControllers.find((entry) => entry.id === emulatedControllerId) ??
    compatibleControllers[0] ??
    null;

  const visiblePhysicalDevices = useMemo(() => {
    const hasSelectedDevice = physicalDevices.some((device) => device.id === selectedPhysicalDeviceId);
    if (hasSelectedDevice || selectedPhysicalDeviceId === keyboardDevice.id) {
      return physicalDevices;
    }

    return [
      ...physicalDevices,
      {
        id: selectedPhysicalDeviceId,
        label: savedPhysicalDeviceLabel,
        type: "saved" as const
      }
    ];
  }, [physicalDevices, savedPhysicalDeviceLabel, selectedPhysicalDeviceId]);

  const selectedPhysicalDevice =
    visiblePhysicalDevices.find((device) => device.id === selectedPhysicalDeviceId) ??
    visiblePhysicalDevices[0] ??
    keyboardDevice;
  const supportsMouseBinding = selectedEmulator?.id === "dolphin";

  const selectedPhysicalDeviceRef = useRef(selectedPhysicalDevice);

  useEffect(() => {
    selectedPhysicalDeviceRef.current = selectedPhysicalDevice;
  }, [selectedPhysicalDevice]);

  const refreshPhysicalDevices = useCallback(() => {
    const gamepads = readConnectedGamepads();
    const nextDevices = [
      keyboardDevice,
      ...gamepads.map((gamepad) => ({
        id: `gamepad:${gamepad.index}`,
        label: gamepad.id || `Manette ${gamepad.index + 1}`,
        type: "gamepad" as const,
        index: gamepad.index
      }))
    ];

    setPhysicalDevices(nextDevices);
  }, []);

  const handleRescanPhysicalDevices = async () => {
    setScanningDevices(true);
    await new Promise((resolve) => window.setTimeout(resolve, 250));
    refreshPhysicalDevices();
    setScanningDevices(false);
  };

  useEffect(() => {
    refreshPhysicalDevices();

    const handleGamepadChange = () => refreshPhysicalDevices();
    window.addEventListener("gamepadconnected", handleGamepadChange);
    window.addEventListener("gamepaddisconnected", handleGamepadChange);

    return () => {
      window.removeEventListener("gamepadconnected", handleGamepadChange);
      window.removeEventListener("gamepaddisconnected", handleGamepadChange);
    };
  }, [refreshPhysicalDevices]);

  useEffect(() => {
    if (!selectedEmulator) {
      return;
    }

    const nextControllers = getCompatibleControllers(selectedEmulator.id);
    const storedProfile = profiles.find((profile) => profile.emulatorId === selectedEmulator.id) ?? null;
    const nextController =
      resolveControllerFromProfile(storedProfile, nextControllers) ?? nextControllers[0] ?? null;

    if (!nextController) {
      return;
    }

    setEmulatedControllerId(nextController.id);

    if (storedProfile) {
      setProfileName(storedProfile.name);
      setSelectedPhysicalDeviceId(storedProfile.physicalDeviceId ?? keyboardDevice.id);
      setSavedPhysicalDeviceLabel(storedProfile.physicalDeviceLabel);
      setDolphinSettings(storedProfile.dolphinSettings ?? defaultDolphinSettings);
      setBindings(mergeBindingsForController(nextController, storedProfile.bindings));
    } else {
      setProfileName(`${selectedEmulator.name} - ${nextController.label}`);
      setSelectedPhysicalDeviceId(keyboardDevice.id);
      setSavedPhysicalDeviceLabel(keyboardDevice.label);
      setDolphinSettings(defaultDolphinSettings);
      setBindings(createBindings(nextController));
    }

    setListeningInputId(null);
    setMessage(null);
  }, [profiles, selectedEmulator]);

  const completion = useMemo(() => {
    if (!selectedController) {
      return { done: 0, total: 0, percent: 0 };
    }

    const done = selectedController.inputs.filter((inputDefinition) =>
      Boolean(getBindingForInput(bindings, inputDefinition.label))
    ).length;

    return {
      done,
      total: selectedController.inputs.length,
      percent: Math.round((done / selectedController.inputs.length) * 100)
    };
  }, [bindings, selectedController]);

  const applyPhysicalInput = useCallback(
    (physicalInput: string) => {
      if (!selectedController || !listeningInputId) {
        return;
      }

      const targetInput = selectedController.inputs.find((entry) => entry.id === listeningInputId);
      if (!targetInput) {
        return;
      }

      setBindings((current) => upsertBinding(current, targetInput.label, physicalInput));
      setListeningInputId(null);
      setMessage(`${targetInput.label} associe a ${physicalInput}.`);
    },
    [listeningInputId, selectedController]
  );

  useEffect(() => {
    if (!listeningInputId) {
      return;
    }

    const device = selectedPhysicalDeviceRef.current;

    if (device.type === "keyboard") {
      const targetInput = selectedController?.inputs.find((entry) => entry.id === listeningInputId);
      const listensForIr = supportsMouseBinding && targetInput ? targetInput.label.startsWith("IR ") : false;
      const handleKeyDown = (event: KeyboardEvent) => {
        if (event.repeat) {
          return;
        }

        event.preventDefault();
        applyPhysicalInput(keyboardEventToInput(event));
      };
      const handleMouseDown = (event: MouseEvent) => {
        event.preventDefault();
        applyPhysicalInput(mouseEventToInput(event));
      };
      const handleMouseMove = (event: MouseEvent) => {
        if (!listensForIr) {
          return;
        }

        const input = mouseMoveEventToInput(event);
        if (input) {
          event.preventDefault();
          applyPhysicalInput(input);
        }
      };
      const preventContextMenu = (event: MouseEvent) => event.preventDefault();

      window.addEventListener("keydown", handleKeyDown);
      if (supportsMouseBinding) {
        window.addEventListener("mousedown", handleMouseDown, true);
        window.addEventListener("mousemove", handleMouseMove, true);
        window.addEventListener("contextmenu", preventContextMenu, true);
      }
      return () => {
        window.removeEventListener("keydown", handleKeyDown);
        if (supportsMouseBinding) {
          window.removeEventListener("mousedown", handleMouseDown, true);
          window.removeEventListener("mousemove", handleMouseMove, true);
          window.removeEventListener("contextmenu", preventContextMenu, true);
        }
      };
    }

    let frame = 0;
    let cancelled = false;

    const pollGamepad = () => {
      const nextInput = readNextGamepadInput(
        selectedPhysicalDeviceRef.current,
        baselineSignalsRef.current
      );

      if (nextInput) {
        applyPhysicalInput(nextInput);
        return;
      }

      if (!cancelled) {
        frame = window.requestAnimationFrame(pollGamepad);
      }
    };

    frame = window.requestAnimationFrame(pollGamepad);

    return () => {
      cancelled = true;
      window.cancelAnimationFrame(frame);
    };
  }, [applyPhysicalInput, listeningInputId, selectedController, supportsMouseBinding]);

  const handleControllerChange = (controllerId: string) => {
    if (!selectedEmulator) {
      return;
    }

    const nextController = compatibleControllers.find((entry) => entry.id === controllerId);
    if (!nextController) {
      return;
    }

    const matchingProfile = findMatchingProfile(
      profiles,
      selectedEmulator.id,
      controllerId,
      selectedPhysicalDeviceId
    );

    setEmulatedControllerId(controllerId);
    setProfileName(matchingProfile?.name ?? `${selectedEmulator.name} - ${nextController.label}`);
    setDolphinSettings(matchingProfile?.dolphinSettings ?? defaultDolphinSettings);
    setBindings(
      matchingProfile
        ? mergeBindingsForController(nextController, matchingProfile.bindings)
        : createBindings(nextController)
    );
    setMessage(null);
  };

  const handlePhysicalDeviceChange = (deviceId: string) => {
    if (!selectedEmulator || !selectedController) {
      setSelectedPhysicalDeviceId(deviceId);
      return;
    }

    const device = visiblePhysicalDevices.find((entry) => entry.id === deviceId);
    const matchingProfile = findMatchingProfile(
      profiles,
      selectedEmulator.id,
      selectedController.id,
      deviceId
    );

    setSelectedPhysicalDeviceId(deviceId);
    setSavedPhysicalDeviceLabel(device?.label ?? keyboardDevice.label);

    if (matchingProfile) {
      setProfileName(matchingProfile.name);
      setDolphinSettings(matchingProfile.dolphinSettings ?? defaultDolphinSettings);
      setBindings(mergeBindingsForController(selectedController, matchingProfile.bindings));
    } else {
      setProfileName(`${selectedEmulator.name} - ${selectedController.label}`);
      setDolphinSettings(defaultDolphinSettings);
      setBindings(createBindings(selectedController));
    }
    setListeningInputId(null);
    setMessage(null);
  };

  const startListening = (inputId: string) => {
    const device = selectedPhysicalDeviceRef.current;
    if (device.type === "saved") {
      setListeningInputId(null);
      setMessage("Rebranche cette manette puis relance un scan pour modifier ce profil.");
      return;
    }

    baselineSignalsRef.current =
      device.type === "gamepad" ? new Set(readGamepadSignals(device)) : new Set();
    setListeningInputId(inputId);
    setMessage(
      device.type === "gamepad"
        ? "Appuie sur le bouton ou bouge le stick a associer."
        : supportsMouseBinding
          ? "Appuie sur une touche, clique, ou bouge la souris pour l'IR."
          : "Appuie sur une touche a associer."
    );
  };

  const clearCurrentControllerBindings = () => {
    if (!selectedController) {
      return;
    }

    setBindings(createBindings(selectedController));
    setListeningInputId(null);
    setMessage("Mapping vide pour cette manette emulee.");
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedEmulator || !selectedController) {
      return;
    }

    try {
      setSaving(true);
      setMessage(null);

      const matchingProfile = findMatchingProfile(
        profiles,
        selectedEmulator.id,
        selectedController.id,
        selectedPhysicalDevice.id
      );
      const profile: ControllerProfile = {
        id:
          matchingProfile?.id ??
          createProfileId(selectedEmulator.id, selectedController.id, selectedPhysicalDevice.id),
        name: profileName.trim() || `${selectedEmulator.name} - ${selectedController.label}`,
        emulatorId: selectedEmulator.id,
        platformLabel: selectedEmulator.platformLabel,
        physicalDeviceId: selectedPhysicalDevice.id,
        physicalDeviceLabel: selectedPhysicalDevice.label,
        emulatedControllerId: selectedController.id,
        emulatedDeviceLabel: selectedController.label,
        dolphinSettings: selectedEmulator.id === "dolphin" ? dolphinSettings : undefined,
        bindings
      };

      const result = await onSaveProfile(profile);
      setMessage(
        result.warning ??
          `Profil enregistre et applique pour ${selectedController.label}.`
      );
    } catch (reason) {
      setMessage(reason instanceof Error ? reason.message : "Impossible d'enregistrer le profil.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <CollapsiblePanel eyebrow="Manettes" title="Mapping visuel unifie">
      {!selectedEmulator && <p className="muted">Selectionne un emulateur pour preparer un profil.</p>}

      {selectedEmulator && selectedController && (
        <form className="mapping-form" onSubmit={handleSubmit}>
          <div className="mapping-grid">
            <label className="field">
              <span>Profil</span>
              <input value={profileName} onChange={(event) => setProfileName(event.target.value)} />
            </label>

            <label className="field">
              <span>Manette physique</span>
              <select
                value={selectedPhysicalDevice.id}
                onChange={(event) => handlePhysicalDeviceChange(event.target.value)}
              >
                {visiblePhysicalDevices.map((device) => (
                  <option key={device.id} value={device.id}>
                    {device.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span>Manette emulee</span>
              <select
                value={selectedController.id}
                onChange={(event) => handleControllerChange(event.target.value)}
              >
                {compatibleControllers.map((controllerDefinition) => (
                  <option key={controllerDefinition.id} value={controllerDefinition.id}>
                    {controllerDefinition.label}
                  </option>
                ))}
              </select>
            </label>

            <div className="mapping-scan-actions">
              <button
                className="ghost-button compact-button scan-button"
                type="button"
                onClick={() => void handleRescanPhysicalDevices()}
                disabled={scanningDevices}
              >
                {scanningDevices ? <span className="button-spinner" aria-hidden="true" /> : null}
                {scanningDevices ? "Scan..." : "Rescanner"}
              </button>
            </div>
          </div>

          {selectedEmulator.id === "dolphin" && selectedController.id !== "gamecube" ? (
            <div className="mapping-options">
              <label className="toggle-field">
                <input
                  type="checkbox"
                  checked={dolphinSettings.irAutoHide}
                  onChange={(event) =>
                    setDolphinSettings((current) => ({
                      ...current,
                      irAutoHide: event.target.checked
                    }))
                  }
                />
                <span>IR auto hide</span>
              </label>
              <label className="toggle-field">
                <input
                  type="checkbox"
                  checked={dolphinSettings.irRelativeInput}
                  onChange={(event) =>
                    setDolphinSettings((current) => ({
                      ...current,
                      irRelativeInput: event.target.checked
                    }))
                  }
                />
                <span>IR relative input</span>
              </label>
            </div>
          ) : null}

          <div className="mapping-designer">
            <div className="mapping-stage-wrap">
              <div className="mapping-stage-heading">
                <div>
                  <strong>{selectedController.label}</strong>
                  <span>{selectedController.description}</span>
                </div>
                <small>{completion.done}/{completion.total} binds</small>
              </div>

              <div className="controller-stage" aria-label={`Mapping ${selectedController.label}`}>
                <div className="controller-shell" />
                {selectedController.inputs.map((inputDefinition) => {
                  const boundValue = getBindingForInput(bindings, inputDefinition.label);
                  const isListening = listeningInputId === inputDefinition.id;

                  return (
                    <button
                      key={inputDefinition.id}
                      type="button"
                      className={`mapping-hotspot ${isListening ? "mapping-hotspot-listening" : ""} ${
                        boundValue ? "mapping-hotspot-bound" : ""
                      }`}
                      style={{
                        left: `${inputDefinition.x}%`,
                        top: `${inputDefinition.y}%`
                      }}
                      onClick={() => startListening(inputDefinition.id)}
                      title={`${inputDefinition.label}${boundValue ? ` -> ${boundValue}` : ""}`}
                    >
                      <span>{inputDefinition.shortLabel}</span>
                      <small>{boundValue || "..."}</small>
                    </button>
                  );
                })}
              </div>
            </div>

            <aside className="mapping-side">
              <div>
                <small>Progression</small>
                <strong>{completion.percent}%</strong>
                <div className="mapping-progress-track">
                  <span style={{ width: `${completion.percent}%` }} />
                </div>
              </div>

              <div>
                <small>Input actif</small>
                <strong>{listeningInputId ? "Ecoute..." : selectedPhysicalDevice.label}</strong>
              </div>

              <div className="mapping-side-actions">
                <button className="ghost-button compact-button" type="button" onClick={clearCurrentControllerBindings}>
                  Effacer
                </button>
                <button className="primary-button compact-button" type="submit" disabled={saving}>
                  {saving ? "Enregistrement..." : "Enregistrer"}
                </button>
              </div>
            </aside>
          </div>

          {message && (
            <p
              className={`form-message status-message ${
                message.includes("Impossible") ? "error-message" : "success-message"
              }`}
            >
              {message}
            </p>
          )}
        </form>
      )}
    </CollapsiblePanel>
  );
}

function input(
  id: string,
  label: string,
  shortLabel: string,
  x: number,
  y: number
): EmulatedInputDefinition {
  return { id, label, shortLabel, x, y };
}

function controller(
  id: string,
  label: string,
  description: string,
  inputs: EmulatedInputDefinition[]
): EmulatedControllerDefinition {
  return { id, label, description, inputs };
}

function getCompatibleControllers(emulatorId: string) {
  return controllerCatalog[emulatorId] ?? [];
}

function resolveControllerFromProfile(
  profile: ControllerProfile | null,
  controllers: EmulatedControllerDefinition[]
) {
  if (!profile) {
    return null;
  }

  return (
    controllers.find((controllerDefinition) => controllerDefinition.id === profile.emulatedControllerId) ??
    controllers.find((controllerDefinition) => controllerDefinition.label === profile.emulatedDeviceLabel) ??
    null
  );
}

function createBindings(controllerDefinition: EmulatedControllerDefinition) {
  return controllerDefinition.inputs.map((inputDefinition) => ({
    physicalInput: "",
    emulatedInput: inputDefinition.label
  }));
}

function mergeBindingsForController(
  controllerDefinition: EmulatedControllerDefinition,
  storedBindings: ControllerBinding[]
) {
  const storedByInput = new Map(
    storedBindings.map((binding) => [binding.emulatedInput.toLocaleLowerCase(), binding.physicalInput])
  );

  return controllerDefinition.inputs.map((inputDefinition) => ({
    emulatedInput: inputDefinition.label,
    physicalInput: storedByInput.get(inputDefinition.label.toLocaleLowerCase()) ?? ""
  }));
}

function getBindingForInput(bindings: ControllerBinding[], emulatedInput: string) {
  return (
    bindings.find((binding) => binding.emulatedInput.toLocaleLowerCase() === emulatedInput.toLocaleLowerCase())
      ?.physicalInput ?? ""
  );
}

function upsertBinding(bindings: ControllerBinding[], emulatedInput: string, physicalInput: string) {
  const hasBinding = bindings.some(
    (binding) => binding.emulatedInput.toLocaleLowerCase() === emulatedInput.toLocaleLowerCase()
  );

  if (!hasBinding) {
    return [...bindings, { emulatedInput, physicalInput }];
  }

  return bindings.map((binding) =>
    binding.emulatedInput.toLocaleLowerCase() === emulatedInput.toLocaleLowerCase()
      ? { ...binding, physicalInput }
      : binding
  );
}

function findMatchingProfile(
  profiles: ControllerProfile[],
  emulatorId: string,
  controllerId: string,
  physicalDeviceId: string
) {
  return (
    profiles.find(
      (profile) =>
        profile.emulatorId === emulatorId &&
        profile.emulatedControllerId === controllerId &&
        profile.physicalDeviceId === physicalDeviceId
    ) ??
    profiles.find(
      (profile) => profile.emulatorId === emulatorId && profile.emulatedControllerId === controllerId
    ) ??
    null
  );
}

function createProfileId(emulatorId: string, controllerId: string, physicalDeviceId: string) {
  return `${emulatorId}-${controllerId}-${physicalDeviceId}`.replace(/[^a-z0-9_-]+/gi, "-").toLowerCase();
}

function readConnectedGamepads() {
  if (!("getGamepads" in navigator)) {
    return [];
  }

  return Array.from(navigator.getGamepads()).filter((gamepad): gamepad is Gamepad => Boolean(gamepad));
}

function readGamepadSignals(device: PhysicalDevice) {
  if (device.type !== "gamepad" || typeof device.index !== "number" || !("getGamepads" in navigator)) {
    return [];
  }

  const gamepad = navigator.getGamepads()[device.index];
  if (!gamepad) {
    return [];
  }

  const signals: string[] = [];

  gamepad.buttons.forEach((button, index) => {
    if (button.value > 0.55 || button.pressed) {
      signals.push(standardGamepadButtons[index] ?? `Button ${index}`);
    }
  });

  gamepadAxes.forEach((axisDefinition) => {
    const value = gamepad.axes[axisDefinition.axis] ?? 0;
    if (value < -0.65) {
      signals.push(axisDefinition.negative);
    }
    if (value > 0.65) {
      signals.push(axisDefinition.positive);
    }
  });

  return signals;
}

function readNextGamepadInput(device: PhysicalDevice, baselineSignals: Set<string>) {
  const signals = readGamepadSignals(device);
  return signals.find((signal) => !baselineSignals.has(signal)) ?? null;
}

function keyboardEventToInput(event: KeyboardEvent) {
  if (event.code === "NumpadEnter") {
    return "Numpad Enter";
  }
  if (event.code === "NumpadMultiply") {
    return "Numpad Multiply";
  }
  if (event.code === "NumpadAdd") {
    return "Numpad Add";
  }
  if (event.code === "NumpadSubtract") {
    return "Numpad Subtract";
  }
  if (event.code === "NumpadDecimal") {
    return "Numpad Decimal";
  }
  if (event.code === "NumpadDivide") {
    return "Numpad Divide";
  }
  if (/^Numpad[0-9]$/.test(event.code)) {
    return `Numpad ${event.code.slice(-1)}`;
  }
  if (event.code === "ArrowUp") {
    return "DPad Up";
  }
  if (event.code === "ArrowDown") {
    return "DPad Down";
  }
  if (event.code === "ArrowLeft") {
    return "DPad Left";
  }
  if (event.code === "ArrowRight") {
    return "DPad Right";
  }
  if (event.code === "Space") {
    return "Space";
  }
  if (event.code === "Enter") {
    return "Enter";
  }

  return event.key.length === 1 ? event.key.toUpperCase() : event.key;
}

function mouseEventToInput(event: MouseEvent) {
  switch (event.button) {
    case 0:
      return "Mouse Left";
    case 1:
      return "Mouse Middle";
    case 2:
      return "Mouse Right";
    case 3:
      return "Mouse Back";
    case 4:
      return "Mouse Forward";
    default:
      return `Mouse Button ${event.button}`;
  }
}

function mouseMoveEventToInput(event: MouseEvent) {
  const minimumDelta = 8;
  const deltaX = event.movementX;
  const deltaY = event.movementY;

  if (Math.abs(deltaX) < minimumDelta && Math.abs(deltaY) < minimumDelta) {
    return null;
  }

  if (Math.abs(deltaX) > Math.abs(deltaY)) {
    return deltaX < 0 ? "Mouse Left Move" : "Mouse Right Move";
  }

  return deltaY < 0 ? "Mouse Up Move" : "Mouse Down Move";
}
