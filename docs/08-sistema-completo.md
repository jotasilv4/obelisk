# Do desktop ao sistema operacional (2026-11-16)

O que foi feito para fechar as lacunas da análise de completude, na ordem recomendada.
Legenda: ✅ feito e visto funcionando · 🟡 feito, sem teste real (precisa de VM ou hardware).

## 1. Modo nativo, várias telas e escala fracionada

| Item | Estado | Onde |
|---|---|---|
| Sessão padrão nativa (DRM/KMS). Se não subir em 10 s, cai para o modo compatível (cage). | 🟡 | `packaging/obelisk-desktop/files/obelisk-session` |
| Várias telas: a principal (barra, Dock, notch) em 0,0 e as outras à direita, com papel de parede e janelas | 🟡 | `backend/drm.rs`, `render.rs::build_elements_secondary` |
| Plugar/desplugar monitor (udev). Janelas de uma tela removida voltam à principal. | 🟡 | `backend/drm.rs::scan_connectors` |
| Resolução e taxa por tela, nos Ajustes › Telas | 🟡 | `mode:<conector>` nos ajustes |
| Escala 100/125/150/175/200 % (wp-fractional-scale). A UI é desenhada na escala inteira acima e a GPU reduz. | ✅ no aninhado | `state.rs::set_output_scale` |
| Escala automática pela densidade da tela (≥ 135 dpi → 150 %, ≥ 170 dpi → 200 %) | 🟡 | |
| Maximizar, tela cheia, encaixe e limites do ponteiro por tela | 🟡 | `state.rs::output_rect_at` |
| Night Shift e VRR por tela; presentation-time com o horário do vblank | 🟡 | |

## 2. Jogos de fábrica

- `multilib` ligado na ISO e no sistema instalado.
- O instalador detecta a GPU e instala os drivers:
  - NVIDIA: `nvidia-open`, `nvidia-utils`, `lib32-nvidia-utils`;
  - AMD: RADV 64 e 32 bits;
  - Intel: ANV 64 e 32 bits.
- Também instala Steam, `lib32-mesa`, `lib32-pipewire`, `lib32-gamemode` e MangoHud (64 e 32 bits).
- A ISO live leva o driver NVIDIA para funcionar nessas placas.
- Todas as dependências foram resolvidas com uma base temporária do pacman (694 pacotes na ISO).

## 3. Compartilhar a tela e protocolos que faltavam

| Protocolo | Estado |
|---|---|
| wlr-screencopy v3 (portal de compartilhamento, OBS, `grim`) | ✅ `grim` capturou a tela inteira e uma região |
| Portal: `obelisk-portals.conf` (arquivos pelo GTK, tela pelo wlr) e o ambiente exportado ao systemd na sessão real | 🟡 |
| xdg-activation (links e notificações trazem o app para a frente; pedido antigo só faz o ícone pular no Dock) | 🟡 |
| fractional-scale, presentation-time, keyboard-shortcuts-inhibit (VM e acesso remoto ficam com os atalhos, menos ⌥⌘⎋) | 🟡 |
| text-input + input-method (fcitx5/ibus), single-pixel-buffer, xdg-foreign | 🟡 |
| security-context: apps Flatpak não veem a captura de tela | 🟡 |

## 4. Pacote, repositório, identidade e ISO

- Os pacotes `obelisk-desktop` (compositor, sessão, greeter, PAM, portais, marca) e `obelisk-themes` (WhiteSur e MacTahoe) são gerados por `scripts/build-repo.sh` no repositório `repo/x86_64`.
- **Identidade:** o gancho do pacman `obelisk-os-release.hook` aplica o `os-release` do Obelisk e o reaplica sempre que o pacote `filesystem` do Arch é atualizado.
- **ISO:** `scripts/build-iso.sh` gera o repositório, coloca uma cópia dele na ISO e roda o `mkarchiso`. Com `OBELISK_REPO_URL`, o sistema instalado também busca atualizações nesse endereço.
- **Instalador:** depois do `archinstall`, liga o multilib e o repositório do Obelisk, instala o desktop, os jogos e o driver da GPU, e ativa a tela de login e os serviços.
- **Teste em VM:** `scripts/test-vm.sh` (QEMU + KVM, UEFI, virtio-gpu com 3D).
- 🟡 A ISO ainda não foi gerada aqui. É preciso `sudo pacman -S archiso qemu-desktop edk2-ovmf` e depois `scripts/build-iso.sh`.

## 5. Tela de login e primeiro uso

- **Tela de login gráfica:** o próprio compositor no modo `--greeter`, com o visual da tela de bloqueio. Mostra a lista de usuários (setas ou ←/→), lembra o último usuário que entrou e tem os botões de energia. Fala com o greetd pelo protocolo IPC dele.
  - ✅ Testado contra um greetd falso: senha errada cancela a sessão; senha certa inicia `obelisk-session` e o greeter sai.
- **Assistente de primeiro uso** (como o "Olá" do macOS), em cinco passos: boas-vindas, aparência, rede, jogos (Modo Jogo automático, abrir a Steam) e tudo pronto. Aparece na primeira sessão real e grava `~/.config/obelisk/welcome-done`. ✅ visto em captura.

## O que falta testar com a VM ou o hardware

- Modo nativo com uma e com várias telas, hotplug, troca de resolução, VRR e Night Shift.
- Instalação completa pela ISO.
- Login real com o greetd.
- Compartilhar a tela no Discord ou no OBS pelo portal.
- Jogo pela Steam.
