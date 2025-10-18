#!/bin/sh

template_name="wayshot"

convert  ../$template_name/ui/images/png/brand.png -define icon:auto-resize=256 ../$template_name/windows/icon.ico
