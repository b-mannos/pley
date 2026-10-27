#!/usr/bin/env bash
# Makes one video from "<image>|<seconds>|<video>": the picture held for the length, 1080p
# H.264 at 1 frame a second, with silent 5.1 AAC.
IFS='|' read -r image seconds video <<< "$1"
tmp="${video%.mkv}.part.mkv"
ffmpeg -v error -y -loop 1 -framerate 1 -i "$image" -f lavfi -i "anullsrc=channel_layout=5.1:sample_rate=48000" \
  -t "$seconds" -vf "scale=1920:1080,format=yuv420p" -c:v libx264 -preset ultrafast -tune stillimage -crf 30 -g 30 \
  -c:a aac -b:a 64k -metadata:s:a:0 language=eng -shortest "$tmp" && mv "$tmp" "$video" && echo "made $(basename "$video")"
