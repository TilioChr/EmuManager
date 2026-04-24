# EmuManager

EmuManager is a portable interface to centralize emulator installation, management, and game launching, with direct integration to RomM for ROM handling.

## Goal

Make emulation as simple as possible:
- easy and fast installation
- direct game launching
- minimal setup required

## Supported Emulators

- Dolphin (GameCube / Wii)
- PCSX2 (PS2)
- melonDS (Nintendo DS)
- Azahar (Nintendo 3DS)
- Eden (Nintendo Switch)

## Features

- Easy emulator installation
- Fully portable setup (everything in one folder)
- RomM integration:
  - server connection
  - library browsing
  - direct game download
- Automatic ROM organization by platform
- Auto-launch with the correct emulator
- Installed emulator version detection
- Clean, usage-focused UI

## Stack

- Frontend: React + Vite
- Backend: Rust (Tauri)
- Communication: Tauri IPC

## Project Status

Version 0.1:
- fully working foundation
- focused on simplicity and plug & play
- more improvements coming (UI, controller support, advanced config)

## Run the project

```bash
npm install
npm run tauri dev
```
