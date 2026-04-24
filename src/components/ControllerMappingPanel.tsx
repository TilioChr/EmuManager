import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import controller3dsImage from "../assets/controllers/3DS.png";
import classicWiiImage from "../assets/controllers/CLASSICWII.png";
import dsImage from "../assets/controllers/DS.png";
import gamecubeImage from "../assets/controllers/GAMECUBE.png";
import ps2Image from "../assets/controllers/PS2.png";
import switchProImage from "../assets/controllers/SWITCHPRO.png";
import wiiNunchukImage from "../assets/controllers/WIINUNCHUK.png";
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
  imageSrc: string;
  stageVariant?: "default" | "vertical";
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
  input("gc_stick_up", "Stick Haut", "L^", 28, 28),
  input("gc_stick_left", "Stick Gauche", "L<", 20, 39),
  input("gc_stick_right", "Stick Droite", "L>", 35, 39),
  input("gc_stick_down", "Stick Bas", "Lv", 28, 49),
  input("gc_dpad_up", "Croix Haut", "D^", 38, 57),
  input("gc_dpad_left", "Croix Gauche", "D<", 32, 66),
  input("gc_dpad_right", "Croix Droite", "D>", 44, 66),
  input("gc_dpad_down", "Croix Bas", "Dv", 38, 74),
  input("gc_start", "Start", "S", 50, 38),
  input("gc_z", "Z", "Z", 80, 18),
  input("gc_l", "L", "L", 24, 11),
  input("gc_r", "R", "R", 76, 11),
  input("gc_y", "Bouton Y", "Y", 70, 26),
  input("gc_x", "Bouton X", "X", 80, 37),
  input("gc_b", "Bouton B", "B", 64, 46),
  input("gc_a", "Bouton A", "A", 72, 39),
  input("gc_c_up", "C Haut", "C^", 62, 57),
  input("gc_c_left", "C Gauche", "C<", 56, 66),
  input("gc_c_right", "C Droite", "C>", 68, 66),
  input("gc_c_down", "C Bas", "Cv", 62, 74)
];

const wiimoteInputs: EmulatedInputDefinition[] = [
  input("wm_dpad_up", "Croix Haut", "D^", 67, 13),
  input("wm_dpad_left", "Croix Gauche", "D<", 62, 19),
  input("wm_dpad_right", "Croix Droite", "D>", 71.5, 19),
  input("wm_dpad_down", "Croix Bas", "Dv", 67, 25),
  input("wm_a", "Bouton A", "A", 67, 33),
  input("wm_b", "Bouton B", "B", 77, 33),
  input("wm_minus", "Minus", "-", 61, 48.5),
  input("wm_plus", "Plus", "+", 73, 48.5),
  input("wm_home", "Home", "H", 67, 48.5),
  input("wm_one", "Bouton 1", "1", 67, 72),
  input("wm_two", "Bouton 2", "2", 67, 80),
  input("wm_ir_up", "IR Haut", "I^", 51, 4),
  input("wm_ir_left", "IR Gauche", "I<", 46, 9),
  input("wm_ir_right", "IR Droite", "I>", 56, 9),
  input("wm_ir_down", "IR Bas", "Iv", 51, 14),
  input("wm_ir_recenter", "IR Recentrer", "IR", 38, 6),
  input("wm_shake_x", "Secouer X", "SX", 85, 60),
  input("wm_shake_y", "Secouer Y", "SY", 95, 60),
  input("wm_shake_z", "Secouer Z", "SZ", 90, 54)
];

const wiimoteNunchukInputs: EmulatedInputDefinition[] = [
  ...wiimoteInputs,
  input("nunchuk_up", "Nunchuk Haut", "N^", 42, 20),
  input("nunchuk_left", "Nunchuk Gauche", "N<", 36, 27),
  input("nunchuk_right", "Nunchuk Droite", "N>", 48, 27),
  input("nunchuk_down", "Nunchuk Bas", "Nv", 42, 35),
  input("nunchuk_c", "Nunchuk C", "C", 30, 18),
  input("nunchuk_z", "Nunchuk Z", "Z", 26, 25)
];

const classicInputs: EmulatedInputDefinition[] = [
  input("cc_l", "L", "L", 20, 12),
  input("cc_r", "R", "R", 80, 12),
  input("cc_zl", "ZL", "ZL", 37, 14),
  input("cc_zr", "ZR", "ZR", 63, 14),
  input("cc_dpad_up", "Croix Haut", "D^", 20.5, 35),
  input("cc_dpad_left", "Croix Gauche", "D<", 13, 47.5),
  input("cc_dpad_right", "Croix Droite", "D>", 28, 47.5),
  input("cc_dpad_down", "Croix Bas", "Dv", 20.5, 61),
  input("cc_stick_up", "Stick Haut", "L^", 36.5, 64),
  input("cc_stick_left", "Stick Gauche", "L<", 30, 75),
  input("cc_stick_right", "Stick Droite", "L>", 43.5, 75),
  input("cc_stick_down", "Stick Bas", "Lv", 36.5, 86),
  input("cc_minus", "Minus", "-", 43, 47),
  input("cc_plus", "Plus", "+", 57, 47),
  input("cc_home", "Home", "H", 50, 47),
  input("cc_y", "Bouton Y", "Y", 70, 47),
  input("cc_x", "Bouton X", "X", 80, 37),
  input("cc_b", "Bouton B", "B", 80, 58),
  input("cc_a", "Bouton A", "A", 89, 47),
  input("cc_rs_up", "Stick Droit Haut", "R^", 63.5, 64),
  input("cc_rs_left", "Stick Droit Gauche", "R<", 57, 75),
  input("cc_rs_right", "Stick Droit Droite", "R>", 70, 75),
  input("cc_rs_down", "Stick Droit Bas", "Rv", 63.5, 86)
];

const switchInputs: EmulatedInputDefinition[] = [
  input("sw_l", "L", "L", 30, 17),
  input("sw_r", "R", "R", 70, 17),
  input("sw_zl", "ZL", "ZL", 35, 12),
  input("sw_zr", "ZR", "ZR", 65, 12),
  input("sw_left_up", "Stick Haut", "L^", 32, 26),
  input("sw_left_left", "Stick Gauche", "L<", 26.5, 35),
  input("sw_left_right", "Stick Droite", "L>", 38, 35),
  input("sw_left_down", "Stick Bas", "Lv", 32, 43),
  input("sw_dpad_up", "Croix Haut", "D^", 40.5, 43),
  input("sw_dpad_left", "Croix Gauche", "D<", 35, 50),
  input("sw_dpad_right", "Croix Droite", "D>", 46, 50),
  input("sw_dpad_down", "Croix Bas", "Dv", 40.5, 57),
  input("sw_minus", "Minus", "-", 41.5, 26),
  input("sw_plus", "Plus", "+", 59, 26),
  input("sw_home", "Home", "H", 55, 35),
  input("sw_capture", "Capture", "C", 45.5, 35),
  input("sw_y", "Bouton Y", "Y", 63, 34.5),
  input("sw_x", "Bouton X", "X", 68, 27),
  input("sw_b", "Bouton B", "B", 68, 42),
  input("sw_a", "Bouton A", "A", 72.5, 34.5),
  input("sw_right_up", "Stick Droit Haut", "R^", 59, 43),
  input("sw_right_left", "Stick Droit Gauche", "R<", 53.5, 50),
  input("sw_right_right", "Stick Droit Droite", "R>", 64.5, 50),
  input("sw_right_down", "Stick Droit Bas", "Rv", 59, 57)
];

const dualshockInputs: EmulatedInputDefinition[] = [
  input("ps_l1", "L1", "L1", 26, 12),
  input("ps_r1", "R1", "R1", 74, 12),
  input("ps_l2", "L2", "L2", 26, 5),
  input("ps_r2", "R2", "R2", 74, 5),
  input("ps_left_up", "Stick Haut", "L^", 38.5, 48),
  input("ps_left_left", "Stick Gauche", "L<", 32, 58),
  input("ps_left_right", "Stick Droite", "L>", 45, 58),
  input("ps_left_down", "Stick Bas", "Lv", 38.5, 69),
  input("ps_dpad_up", "Croix Haut", "D^", 26, 25),
  input("ps_dpad_left", "Croix Gauche", "D<", 19.5, 36),
  input("ps_dpad_right", "Croix Droite", "D>", 32, 36),
  input("ps_dpad_down", "Croix Bas", "Dv", 26, 48),
  input("ps_select", "Select", "Se", 42.5, 37),
  input("ps_start", "Start", "St", 57.5, 37),
  input("ps_square", "Carre", "Sq", 67, 36),
  input("ps_triangle", "Triangle", "Tr", 74, 24),
  input("ps_cross", "Croix", "X", 74, 48),
  input("ps_circle", "Rond", "O", 81, 36),
  input("ps_right_up", "Stick Droit Haut", "R^", 61.5, 48),
  input("ps_right_left", "Stick Droit Gauche", "R<", 55, 58),
  input("ps_right_right", "Stick Droit Droite", "R>", 68, 58),
  input("ps_right_down", "Stick Droit Bas", "Rv", 61.5, 69)
];

const compactNintendoInputs: EmulatedInputDefinition[] = [
  input("nds_dpad_up", "Croix Haut", "D^", 12.75, 34),
  input("nds_dpad_left", "Croix Gauche", "D<", 5, 46),
  input("nds_dpad_right", "Croix Droite", "D>", 20.5, 46),
  input("nds_dpad_down", "Croix Bas", "Dv", 12.75, 58),
  input("nds_l", "L", "L", 8, 14),
  input("nds_r", "R", "R", 92, 14),
  input("nds_select", "Select", "Se", 80, 82.5),
  input("nds_start", "Start", "St", 80, 73),
  input("nds_y", "Bouton Y", "Y", 81.5, 43),
  input("nds_x", "Bouton X", "X", 87, 33),
  input("nds_b", "Bouton B", "B", 87, 52),
  input("nds_a", "Bouton A", "A", 92.5, 43)
];

const azaharInputs: EmulatedInputDefinition[] = [
  input("3ds_circle_up", "Stick Haut", "L^", 14.5, 23),
  input("3ds_circle_left", "Stick Gauche", "L<", 9, 32),
  input("3ds_circle_right", "Stick Droite", "L>", 20, 32),
  input("3ds_circle_down", "Stick Bas", "Lv", 14.5, 40),
  input("3ds_dpad_up", "Croix Haut", "D^", 14.5, 48),
  input("3ds_dpad_left", "Croix Gauche", "D<", 9, 56.5),
  input("3ds_dpad_right", "Croix Droite", "D>", 20, 56.5),
  input("3ds_dpad_down", "Croix Bas", "Dv", 14.5, 65),
  input("3ds_l", "L", "L", 12, 10),
  input("3ds_r", "R", "R", 88, 10),
  input("3ds_zl", "ZL", "ZL", 20, 10),
  input("3ds_zr", "ZR", "ZR", 80, 10),
  input("3ds_select", "Select", "Se", 79, 78),
  input("3ds_start", "Start", "St", 79, 67),
  input("3ds_home", "Home", "H", 50, 91),
  input("3ds_y", "Bouton Y", "Y", 80.5, 39),
  input("3ds_x", "Bouton X", "X", 85, 31),
  input("3ds_b", "Bouton B", "B", 85, 47),
  input("3ds_a", "Bouton A", "A", 90, 39),
  input("3ds_c_up", "C-Stick Haut", "C^", 80, 18),
  input("3ds_c_left", "C-Stick Gauche", "C<", 76, 22),
  input("3ds_c_right", "C-Stick Droite", "C>", 84, 22),
  input("3ds_c_down", "C-Stick Bas", "Cv", 80, 25)
];

const controllerCatalog: Record<string, EmulatedControllerDefinition[]> = {
  dolphin: [
    controller("gamecube", "GameCube controller", "Ports manette GameCube de Dolphin", gamecubeImage, gamecubeInputs),
    controller("wiimote", "Wiimote", "Wiimote seule", wiiNunchukImage, wiimoteInputs, "vertical"),
    controller("wiimote_nunchuk", "Wiimote + Nunchuk", "Wiimote avec extension Nunchuk", wiiNunchukImage, wiimoteNunchukInputs, "vertical"),
    controller("classic_controller", "Wii Classic Controller", "Extension Classic Controller", classicWiiImage, classicInputs)
  ],
  eden: [
    controller("switch_pro", "Pro Controller", "Manette Switch Pro", switchProImage, switchInputs),
    controller("joycon_pair", "Joy-Con pair", "Deux Joy-Con en mode horizontal", switchProImage, switchInputs)
  ],
  pcsx2: [
    controller("dualshock2", "DualShock 2", "Manette PlayStation 2", ps2Image, dualshockInputs)
  ],
  melonds: [
    controller("nds", "Nintendo DS controls", "Boutons Nintendo DS", dsImage, compactNintendoInputs)
  ],
  azahar: [
    controller("3ds", "Nintendo 3DS controls", "Boutons Nintendo 3DS", controller3dsImage, azaharInputs)
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

              <div
                className={`controller-stage ${
                  selectedController.stageVariant === "vertical" ? "controller-stage-vertical" : ""
                }`}
                aria-label={`Mapping ${selectedController.label}`}
              >
                <img
                  className="controller-image"
                  src={selectedController.imageSrc}
                  alt=""
                  aria-hidden="true"
                  draggable={false}
                />
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
  imageSrc: string,
  inputs: EmulatedInputDefinition[],
  stageVariant: "default" | "vertical" = "default"
): EmulatedControllerDefinition {
  return { id, label, description, imageSrc, stageVariant, inputs };
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
