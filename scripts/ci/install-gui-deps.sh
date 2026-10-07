#!/usr/bin/env bash
# System libraries the desktop app (egui/eframe/wgpu, rfd) needs to build and test on Ubuntu,
# including Mesa's llvmpipe/lavapipe software renderers so GUI tests run without a GPU.
set -euo pipefail
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxrandr-dev libxi-dev libxtst-dev libx11-dev libgl-dev libegl-dev libvulkan-dev \
  mesa-vulkan-drivers libegl-mesa0 libgl1-mesa-dri
