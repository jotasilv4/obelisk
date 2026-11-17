#!/usr/bin/env bash
# Testa a ISO do Obelisk numa máquina virtual (QEMU + KVM, UEFI, GPU virtio com 3D).
#   scripts/test-vm.sh            → dá boot na ISO mais nova de out/ com um disco de 40 GB
#   scripts/test-vm.sh --disk     → dá boot no disco já instalado (sem a ISO)
# Precisa de: qemu-desktop edk2-ovmf   (sudo pacman -S --needed qemu-desktop edk2-ovmf)
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p work-vm
DISK=work-vm/obelisk.qcow2
[ -f "$DISK" ] || qemu-img create -f qcow2 "$DISK" 40G >/dev/null
cp -n /usr/share/edk2/x64/OVMF_VARS.4m.fd work-vm/vars.fd 2>/dev/null || true
ISO=$(ls -1t out/*.iso 2>/dev/null | head -1 || true)
BOOT=()
if [[ "${1:-}" != "--disk" ]]; then
  [ -n "$ISO" ] || { echo "nenhuma ISO em out/ (rode scripts/build-iso.sh)"; exit 1; }
  BOOT=(-cdrom "$ISO" -boot order=d)
fi
exec qemu-system-x86_64 \
  -enable-kvm -machine q35 -cpu host -smp 4 -m 6G \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd \
  -drive if=pflash,format=raw,file=work-vm/vars.fd \
  -drive file="$DISK",if=virtio \
  -device virtio-vga-gl -display gtk,gl=on \
  -device virtio-net,netdev=n0 -netdev user,id=n0 \
  -device qemu-xhci -device usb-tablet \
  -audiodev pipewire,id=snd0 -device ich9-intel-hda -device hda-output,audiodev=snd0 \
  "${BOOT[@]}"
