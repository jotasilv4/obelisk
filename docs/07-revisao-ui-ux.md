# Revisão de UI/UX (2026-11-16)

Revisão de todas as superfícies do Obelisk, a partir de capturas no modo escuro e no claro, comparadas com o macOS Sonoma/Sequoia.

## Marca

- O símbolo oficial é `branding/obelisk-symbol.svg`, um hexágono de quatro peças. O chacal (`branding/obelisk-jackal.svg`) está guardado como mascote e ainda não é usado.
- O compositor desenha o símbolo a partir do próprio SVG (`ui::text_glyph::obelisk_logo`). Ele aparece:
  - na barra de menus, no lugar da maçã;
  - no cartão do usuário e no cabeçalho "Sobre" dos Ajustes;
  - no avatar da tela de bloqueio.
- `branding/gen.py` gera a partir do símbolo:
  - a marca em branco, em cor e em preto;
  - o ícone do app (squircle azul);
  - os wordmarks claro e escuro;
  - o desenho de fundo dos papéis de parede e da tela de login.
- `branding/render.sh` gera os PNGs, que já foram copiados para o perfil da ISO.

## Corrigido nesta rodada

| Onde | Problema | O que mudou |
|---|---|---|
| Barra de menus | Faltavam Arquivo, Editar, Visualizar e Ajuda | Ordem do macOS: App, Arquivo, Editar, Visualizar, Ir, Janela, Ajuda. As ações mandam o atalho ao app em foco (⌘N, ⌘Z, ⌘C…). Em terminais, Copiar e Colar usam Ctrl+Shift. |
| Barra de menus | O notch cobria os títulos | O que não cabe antes do notch passa para depois dele, como no MacBook. Em tela estreita, os pontos das mesas somem da barra (continuam no menu Ir). |
| "Sobre este Obelisk" | Abria o site do Arch | Abre Ajustes › Sobre |
| Forçar Encerrar (⌥⌘⎋) | Era um menu sem botões nem ícones | Virou janela própria: título, texto de ajuda, lista com ícones e botão azul "Forçar Encerrar". ↑/↓ escolhe, Enter encerra, Esc fecha. |
| Central de Notificações | Título solto e "Sem notificações" num retângulo | Agora segue o Sonoma: notificações em cima e "Limpar" numa pílula de vidro. Embaixo ficam os widgets: dia da semana com o dia do mês, relógio com fuso e calendário do mês. |

## Notch igual ao do kyro (segunda rodada)

| Recurso | Estado |
|---|---|
| Aba **Bandeja** no notch aberto. Clique abre o arquivo, botão direito o tira da Bandeja, a lixeira limpa tudo. | ✅ |
| **Soltar arquivo** sobre o notch: ele vira dois cartões, Bandeja de Arquivos (tracejado) e Copiar (manda `text/uri-list` para a área de transferência) | 🟡 desenho visto; arrastar de um app real não testado |
| Cartão da **captura de tela**: miniatura, Copiar, Anotar (satty/swappy), Salvar como… (abre a pasta). Dura 8 s e pausa com o mouse em cima. | ✅ |
| **Tocando agora**: capa 96 com brilho na cor da capa, selo do player, título em 2 linhas, álbum e artista | ✅ (capa de `mpris:artUrl`: file, http ou data) |
| Música com o notch fechado: capa real e barrinhas na cor da capa | ✅ |
| Faixa de dias do kyro: hoje em azul e grande, fins de semana em vermelho, dias distantes esmaecidos | ✅ |
| Conteúdo entra crescendo do topo (modos abertos) ou das laterais (asas), recortado pela silhueta | ✅ |
| Botão direito numa notificação passa para a próxima | ✅ |
| Aviso de fone ou controle Bluetooth conectado ou desconectado | 🟡 |
| Arrastar um arquivo da Bandeja para outro app | ✅ arrasto iniciado pelo compositor (`text/uri-list`); testado soltando no `foot`, que colou o caminho |

## Itens da lista anterior (feitos)

- **Ícones na mesa:** os arquivos da pasta Área de Trabalho aparecem em colunas a partir da direita, à esquerda dos widgets. Imagens viram miniatura e atalhos `.desktop` mostram o ícone do app. Clique seleciona e duplo clique abre.
- **Clique direito na mesa:** Nova Pasta, Mudar Papel de Parede…, Mostrar/Ocultar Widgets e Ajustes da Mesa e Dock. Num ícone: Abrir, Mostrar no Arquivos e Mover para o Lixo.
- **Central de Controle:** "Tocando agora" com a capa do álbum.
- **Widgets:** com um app em foco, ficam monocromáticos e mais discretos, como no Sonoma.
- **Launchpad:** esconde variantes do mesmo app, como "Foot Client" e "Foot Server".
- **Fechar janela:** a janela encolhe e esmaece em 150 ms antes de fechar. Se o app recusar (por exemplo, "salvar alterações?"), ela volta a aparecer.
- **Consumo parado:** medido em 0,5 % de CPU, sem mudança.

## Terceira rodada

- **Arrastar arquivos para fora:** os itens da Bandeja e os ícones da mesa podem ser arrastados para qualquer app que aceite arquivos. Um fantasma do ícone segue o cursor. Soltar fora de um app tem três destinos:
  - na mesa, o ícone vai para a célula da grade (posição salva em `~/.config/obelisk/desktop`);
  - no Lixo do Dock, o arquivo vai para o lixo (`gio trash`);
  - no notch, o arquivo entra na Bandeja.
- **Renomear na mesa:** Enter com o ícone selecionado (como no Finder) ou Renomear no menu. Enter confirma e Esc cancela.
- **Sobre o App** (menu do app): ícone, nome, versão (`pacman -Qo` ou `flatpak info`), descrição do `.desktop` e o pacote.
- **Correção de travamento:** ao soltar um arrasto, o código pedia a posição do ponteiro enquanto o ponteiro estava travado pelo arrasto, e o compositor travava. Agora a posição fica guardada no estado (`last_pointer`).

## Ainda diferente do macOS

Nada da lista anterior ficou pendente. O backend nativo (DRM) continua sem teste em hardware real.
