- Build release binary first: `cargo build --release`

- Install dependencies: `flatpak`, `flatpak-builder`, `ImageMagick` (for icon resize)

- Run: `./pkg-flatpak.sh`

- The output is `wayshot.flatpak`

- Install Flatpak: `flatpak install wayshot.flatpak`

- Run: `flatpak run wayshot`

- Uninstall: `flatpak uninstall wayshot`
