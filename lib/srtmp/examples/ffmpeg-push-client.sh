#!/bin/bash

ffmpeg -re -f lavfi \
  -i "testsrc=size=1280x720:rate=30" \
  -f lavfi -i "sine=frequency=1000:duration=3600" \
  -c:v libx264 -preset veryfast \
  -c:a aac -b:a 128k \
  -f flv "rtmp://localhost/live/stream"
