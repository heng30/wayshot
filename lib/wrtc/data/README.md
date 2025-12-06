```bash
ffmpeg -i $INPUT_FILE -an -c:v libx264 -bsf:v h264_mp4toannexb -b:v 2M -max_delay 0 -bf 0 output.h264
ffmpeg -i $INPUT_FILE -c:a libopus -page_duration 20000 -vn output.ogg
```
