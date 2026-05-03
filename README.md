
# Keyboim

<p align="center">
  <img src="./icon.svg" width="96" height="96" alt="Keyboim icon">
</p>

Keyboim is a Windows key input overlay. It captures global keyboard and mouse input, shows the latest key combination on screen, and can switch into a transparent click-through overlay mode when you want the display to sit above other windows.

## Screenshots

Normal mode:

![Keyboim normal mode](https://github.com/user-attachments/assets/e939f520-d554-4373-b8c5-9a2ac0686f52)

Overlay mode:

![Keyboim overlay mode](https://github.com/user-attachments/assets/7ca45742-72fe-4da6-ae3f-dc3979874fb1)

## Features

- Global keyboard input display
- Global mouse button state display
- Toggleable text outline
- Toggleable mouse icon
- Transparent overlay mode
- Click-through behavior in overlay mode

## Usage

Run the release executable:

```powershell
.\target\release\keyboim.exe
```

After launch, press keys in any application and Keyboim will display the latest key combination.

Main controls:

- `Outline Text`: toggles the outline around the large key text.
- `Show Mouse`: toggles the mouse button indicator.
- `Overlay`: hides the frame/background and enables transparent click-through overlay mode.
- `x`: closes the app.
- Title bar drag: moves the window.

To leave overlay mode, press:

```text
Ctrl + Shift + Alt + Q + E
```

If you need to display input from an elevated application, Keyboim may also need to be run as administrator.

## Build

### Requirements

- Windows 10 or 11
- Rust stable toolchain
- MSVC C++ Build Tools
- Graphics driver with OpenGL 3.3 support

Build from the project root:

```powershell
cargo build
```

For development:

```powershell
cargo run
```

For a release-mode run:

```powershell
cargo run --release
```

## License

This project is licensed under the MIT License. See [LICENSE](./LICENSE) for details.
