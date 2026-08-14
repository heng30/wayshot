# Arch Linux packaging

Two ways to produce an Arch Linux package (`.pkg.tar.zst`):

## Quick pack (uses the already-built release binary)

Prerequisites (on Arch Linux):
- `makepkg` (provided by `pacman`), `fakeroot`, `imagemagick`
- A release binary built first: `make build-release` (or `make`)

```bash
cd package/archlinux && ./pkg-arch.sh
```

or from the project root:

```bash
make archlinux
```

Output: `package/archlinux/wayshot-<version>-1-x86_64.pkg.tar.zst`

Install / uninstall:

```bash
sudo pacman -U wayshot-<version>-1-x86_64.pkg.tar.zst
sudo pacman -R wayshot
```

## From source (PKGBUILD)

Suitable for publishing to the AUR or full rebuilds:

```bash
cd package/archlinux
makepkg -si
```

Builds with the default `wayland-wlr` capture feature (for wlroots compositors like sway/Hyprland).
