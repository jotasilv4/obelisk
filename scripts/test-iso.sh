#!/usr/bin/env bash
# Boot rápido da ISO em QEMU (UEFI). Precisa: qemu-desktop, edk2-ovmf.
set -euo pipefail
cd "$(dirname "$0")/.."
ISO="${1:-$(ls -1t out/*.iso | head -1)}"
exec qemu-system-x86_64 -enable-kvm -m 4G -smp 4 -cpu host \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd \
  -device virtio-vga-gl -display gtk,gl=on \
  -device qemu-xhci -device usb-tablet -device usb-kbd \
  -cdrom "$ISO" -boot d
