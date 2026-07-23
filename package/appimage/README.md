- Build release binary first: `make`

- Install `appimagetool` from https://github.com/AppImage/AppImageKit:
    - `sudo wget https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage -O /usr/bin/appimagetool`
    - `sudo chmod a+x /usr/bin/appimagetool`


- Install `ImageMagick` (for icon resize) : `sudo apt install imagemagick`

- Run: `./pkg-appimage.sh`

- The output is `wayshot.AppImage`

- Run AppImage: `chmod +x wayshot.AppImage && ./wayshot.AppImage`

- Extract AppImage: `./wayshot.AppImage --appimage-extract`

- `https://github.com/AppImageCommunity/pkg2appimage`
