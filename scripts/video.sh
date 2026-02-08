#!/usr/bin/env bash
set -euo pipefail

# Fixed input path as requested
INPUT="/home/thavlik/Videos/OBS/2026-02-07 10-49-09.mp4"

# Optional output name (default: thumbnail.webm)
OUTPUT="${1:-thumbnail.webm}"

# Time ranges in seconds:
# 00:01:12 → 72   to 00:01:17 → 77
# 00:10:27 → 627  to 00:10:32 → 632

ffmpeg -y -i "$INPUT" \
  -filter_complex "\
    [0:v]crop=iw:ih-192:0:128,format=yuv420p[v]; \
    [v]split=2[v0][v1]; \
    [v0]trim=start=72:end=77,setpts=PTS-STARTPTS[v0t]; \
    [v1]trim=start=627:end=632,setpts=PTS-STARTPTS[v1t]; \
    [v0t][v1t]concat=n=2:v=1:a=0[vc]; \
    [vc]fps=30,scale=-1:240[outv] \
  " \
  -map "[outv]" \
  -an \
  -c:v libvpx-vp9 \
  -b:v 0 \
  -crf 40 \
  -preset good \
  -row-mt 1 \
  -cpu-used 4 \
  "$OUTPUT"
