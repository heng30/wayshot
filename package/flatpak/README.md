- Build release binary first: `make`

- Install dependencies: `flatpak`, `flatpak-builder`, `ImageMagick` (for icon resize) : `sudo apt install flatpak flatpak-builder imagemagick`

- Run: `./pkg-flatpak.sh`

- The output is `wayshot.flatpak`

- Install Flatpak: `flatpak install wayshot.flatpak`

- Run: `flatpak run xyz.heng30.wayshot`

- Uninstall: `flatpak uninstall wayshot`
