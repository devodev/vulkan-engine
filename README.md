# Vulkan Engine

- [Overview](#overview)
- [Dependencies](#dependencies)
  - [Install windows-msvc](#install-windows-msvc)

## Overview

This project is an attempt at writing a game engine in Rust from scratch.

It uses Vulkan as graphics API with the help of [vulkano-rs/vulkano](https://github.com/vulkano-rs/vulkano) rust library.

## Dependencies

> Assumed OS: Windows 10

We will be compiling libshaderc ourselves (vulkano dep), which means we will need to have these tools available in our PATH:

- CMake
- Ninja
- Python

### Install windows-msvc

1. Set the default Rust toolchain to msvc: `rustup default stable-x86_64-pc-windows-msvc`.
2. Install [Build Tools for Visual Studio 2022](https://visualstudio.microsoft.com/thank-you-downloading-visual-studio/?sku=Community&channel=Release&version=VS2022&source=VSLandingPage&cid=2030&passive=false).
   1. In the launcher, select the bundle for game dev.
   2. Add bin dir to PATH: `C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.31.31103\bin\Hostx64\x64`
3. Install [msys2](https://www.msys2.org/), following ALL of the instructions.
   1. Run Installer
   2. Start MSYS2 MSYS from start menu
   3. Run: `pacman -Syu`
      1. Enter Y for everything
   4. Start MSYS2 MSYS from start menu
   5. Run `pacman -Syu`
      1. Enter Y for everything
   6. Install GCC and other build packages: `pacman -S --needed base-devel mingw-w64-x86_64-toolchain`.
      1. Press Enter (default=all), then Y.
4. Then in the msys2 terminal run: `pacman --noconfirm -Syu mingw-w64-x86_64-cmake mingw-w64-x86_64-python2 mingw-w64-x86_64-ninja`
5. Add the msys2 mingw64 binary path to the PATH environment variable.
   1. `C:\msys64`
