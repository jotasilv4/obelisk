# Telas, inicialização, instalador, atualizações e loja (2026-11-16)

Legenda: ✅ feito e visto funcionando · 🟡 feito, sem teste real (precisa de VM ou hardware).

## Telas: arrumar como no macOS

- **Ajustes › Telas:** retângulos das telas que você arrasta. A principal tem a faixa branca da barra de menus. Ao soltar, a tela encosta na borda mais próxima de outra, sem sobrepor. ✅ (UI e encaixe testados com uma tela simulada)
- **Posições** salvas por monitor (`pos:<conector>`, relativas à principal) e reaplicadas ao ligar ou plugar. 🟡
- **"Usar como principal":** a barra, o Dock e o notch vão para a tela escolhida (`primary`). 🟡
- **"Espelhar telas":** as outras telas mostram a principal reescalada (para projetor). 🟡
- **Resolução, taxa e escala** por tela, como já existia. 🟡

## Inicialização com a cara do Obelisk

- **Plymouth** (`branding/plymouth`): fundo preto, o símbolo aparece suavemente e uma barra fina de progresso, como no Mac. No desligar e reiniciar fica só o símbolo. A senha do disco criptografado aparece num campo arredondado com pontos. 🟡 (o Plymouth só roda como root; vi uma prévia estática com a mesma geometria)
- **ISO:** gancho `plymouth` no initramfs, `quiet splash`, menu em português, tela inicial do syslinux com o símbolo e cores do Obelisk.
- **Sistema instalado:**
  - UEFI: `systemd-boot` sem menu (segure Espaço ao ligar para ver as opções) e uma entrada de recuperação.
  - BIOS: GRUB escondido, com tema próprio quando aparece (`branding/grub/obelisk`).

## Instalador gráfico

- **Interface:** o próprio Obelisk em modo instalador, como o "Instalar macOS". Abre sozinho no sistema live e tem também "Experimentar primeiro", o item no menu do símbolo e o atalho no Launchpad. ✅ (todas as telas vistas)
  1. Idioma.
  2. Teclado (muda na hora) e fuso com busca.
  3. Disco (ignora o pendrive do live), com criptografia opcional (LUKS, como o FileVault; só em UEFI).
  4. Conta, com o nome de usuário derivado do nome e validação.
  5. Resumo, com aviso se estiver sem internet.
  6. Progresso e fim ("Reiniciar", ou "Tentar de novo" com o registro).
- **`obelisk-install-auto`** (root, sem perguntas):
  - Partições GPT e btrfs com `@`, `@home`, `@log` e `@cache`.
  - `pacstrap` com o progresso real (pacote n de N).
  - Idioma, fuso, teclado, conta com sudo e root bloqueado.
  - initramfs com Plymouth (e `encrypt`), carregador de inicialização, drivers da GPU, Steam e multilib.
  - Ativa greetd, rede, Bluetooth, energia, fstrim e Instantâneos (snapper), adiciona o Flathub e copia a rede Wi‑Fi do live.
  - 🟡 Nunca rodou: só passou na checagem de sintaxe do bash.

## Atualizações sem terminal

- **Verificação:** a cada 6 h (a primeira 2 min depois de entrar, nunca no Modo Jogo), contando pacman e Flatpak.
- **Aviso:** sai no notch; clicar abre Ajustes › Atualizações. O menu do símbolo mostra a contagem.
- **"Atualizar agora":** roda dentro dos Ajustes com progresso. A senha é pedida pelo notch (polkit).
- 🟡 O fluxo de tarefa com progresso foi testado com um comando simulado.

## App Store

- **Janela:** a mesma dos Ajustes no modo loja: Descobrir, Jogos, Criatividade, Produtividade, Internet, Entretenimento, Utilitários, Instalados e Atualizações, mais a busca no catálogo inteiro. ✅
- **Fonte:** Flathub, instalado para o usuário (sem senha). Catálogo com cerca de 3500 apps guardado por um dia; nomes, descrições e ícones vêm do appstream.
- **Botões:** "Obter", "Abrir", "Remover" e "Atualizar", como tarefas com faixa de progresso.
- **Abertura:** pelo Dock (fixada por padrão), Launchpad, Busca e menu do símbolo, via `obelisk-open store` nos arquivos `.desktop`.
- 🟡 Instalar um app de verdade não foi testado: o primeiro baixa centenas de MB de runtime.

## Observação

Num dos testes da loja o Firefox abriu sozinho dentro da janela de teste. Não se repetiu em duas tentativas, uma delas com o catálogo baixado do zero. Nenhum comando do Obelisk o lançou: o registro não mostra a chamada.
