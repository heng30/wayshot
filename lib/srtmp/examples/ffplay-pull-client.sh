#!/bin/bash

ffplay -fflags nobuffer -flags low_delay -max_delay 100000 \
       -i "rtmp://localhost:1935/live/stream"
