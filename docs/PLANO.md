# Plano — Monitor de Desempenho e Serviços (Windows + Linux)

## Decisões tomadas

| Tema | Decisão |
|---|---|
| Sistemas | Windows 10/11 e Linux (X11 completo; Wayland com uma limitação, ver abaixo) |
| Linguagem | Rust (100%) |
| Interface | `egui` / `eframe` com gráficos em `egui_plot`, em português (pt-BR) |
| Execução | Um único executável que inicia com o sistema e fica na bandeja, coletando mesmo com a janela fechada |
| Métricas | CPU, RAM, disco, rede, temperaturas, GPU, serviços, processos e tempo de uso por aplicativo |
| GPU | AMD. Windows: uso e VRAM pelos contadores do Windows (PDH). Linux: arquivos do driver `amdgpu` |
| Temperaturas | Windows: LibreHardwareMonitor via WMI (opcional). Linux: sensores do kernel (`hwmon`), nativo |
| Serviços | Lista escolhida pelo usuário na interface (Windows: SCM; Linux: systemd) |
| Alertas | Notificação do sistema **e** registro no relatório |
| Relatório | HTML autocontido com gráficos SVG, gerado às 23:00 (configurável) **e** por um botão manual |
| Histórico | SQLite, 90 dias de retenção |
| Distribuição | Binário avulso: `monitor.exe` no Windows, `monitor` no Linux |

---

## Arquitetura

```
┌─────────────────────────────── monitor ───────────────────────────────┐
│                                                                       │
│  Thread coletora (1 s)                Thread de persistência/agendador│
│  ├─ sysinfo → CPU/RAM/disco/rede/processos  (comum)                   │
│  └─ platform::*  (uma implementação por SO) ──► canal ──► SQLite (WAL)│
│       ├─ Sensores  → temperaturas              ├─ alertas → notificação│
│       ├─ Gpu       → uso / VRAM                ├─ relatório às 23:00   │
│       ├─ Servicos  → estado dos serviços       └─ limpeza (> 90 dias)  │
│       └─ Atividade → app em foco / ociosidade                          │
│         │                                                             │
│         ▼ estado ao vivo (Arc<RwLock<…>>, buffer circular de 10 min)  │
│  UI egui (janela) ◄──── ícone de bandeja (tray-icon)                  │
└───────────────────────────────────────────────────────────────────────┘
```

A coleta roda em threads próprias, independentes da janela. Fechar a janela só a esconde; "Sair" fica no menu da bandeja.

### Camada de plataforma

Tudo que depende do sistema operacional fica atrás de uma interface (trait) comum, com uma implementação por SO escolhida em tempo de compilação (`#[cfg(windows)]` / `#[cfg(target_os = "linux")]`). O resto do programa (UI, banco, alertas, relatório) não sabe em qual sistema está rodando.

```rust
pub trait Sensores  { fn ler(&mut self) -> Result<Vec<LeituraTemperatura>>; }
pub trait Gpu       { fn ler(&mut self) -> Result<Vec<EstadoGpu>>; }
pub trait Servicos  { fn listar(&mut self) -> Result<Vec<EstadoServico>>; }
pub trait Atividade { fn app_em_foco(&mut self) -> Result<Option<AppEmFoco>>;
                      fn segundos_ocioso(&mut self) -> Result<Option<u64>>; }
```

Quando uma fonte não está disponível (LibreHardwareMonitor fechado, Wayland sem suporte etc.), a implementação devolve um erro ou `None` e a interface mostra "indisponível", sem derrubar o resto.

### Implementação por sistema

| Parte | Windows | Linux |
|---|---|---|
| CPU, RAM, disco, rede, processos | `sysinfo` | `sysinfo` |
| Temperaturas | WMI `root\LibreHardwareMonitor` (crate `wmi`) | `/sys/class/hwmon` (`k10temp`, `coretemp`, `amdgpu`) |
| GPU AMD (uso, VRAM) | PDH: `\GPU Engine(*)\Utilization Percentage`, `\GPU Adapter Memory(*)\Dedicated Usage` | `/sys/class/drm/card*/device/gpu_busy_percent`, `mem_info_vram_used` e `mem_info_vram_total` |
| Serviços | Service Control Manager (crate `windows`) | systemd via D-Bus (crate `zbus`) |
| App em foco | `GetForegroundWindow` + nome do processo | X11: `_NET_ACTIVE_WINDOW` + `_NET_WM_PID` (crate `x11rb`) |
| Ociosidade | `GetLastInputInfo` | X11: extensão XScreenSaver; Wayland: `IdleHint` do logind |
| Notificação | `notify-rust` (toast do Windows) | `notify-rust` (padrão freedesktop) |
| Bandeja | `tray-icon` | `tray-icon` (GTK + AppIndicator, em thread própria) |
| Iniciar com o sistema | `auto-launch` (registro `HKCU\...\Run`) | `auto-launch` (`~/.config/autostart/*.desktop`) |
| Pastas | `%APPDATA%\monitor`, `Documentos\Monitor\Relatorios` | `~/.config/monitor`, `~/.local/share/monitor`, `~/Documentos/Monitor/Relatorios` (crate `directories`) |

### Crates comuns

| Função | Crate |
|---|---|
| Janela e gráficos | `eframe`, `egui`, `egui_plot` |
| Banco de dados | `rusqlite` (feature `bundled`) |
| Relatório HTML | `askama` (template) + `plotters` (backend SVG) |
| Configuração | `serde` + `toml` |
| Logs | `tracing` + `tracing-subscriber` + `tracing-appender` |
| Datas | `chrono` |
| Erros | `anyhow` |

### Estrutura do projeto

```
monitor/
├─ Cargo.toml
├─ src/
│  ├─ main.rs              # inicialização, bandeja, threads
│  ├─ lib.rs
│  ├─ config.rs            # limites, serviços vigiados, horário do relatório
│  ├─ modelo.rs            # tipos comuns (leituras, estados)
│  ├─ coleta.rs            # laço de coleta: sysinfo + plataforma
│  ├─ plataforma/
│  │  ├─ mod.rs            # traits + escolha da implementação por SO
│  │  ├─ windows/          # sensores.rs, gpu.rs, servicos.rs, atividade.rs
│  │  └─ linux/            # sensores.rs, gpu.rs, servicos.rs, atividade.rs
│  ├─ armazenamento.rs     # SQLite: schema, inserts, consultas, retenção
│  ├─ alertas.rs           # regras, histerese, notificações
│  ├─ relatorio/           # agregação do dia, gráficos SVG, template HTML
│  └─ ui/                  # visao_geral, processos, servicos, historico, configuracoes
├─ examples/               # protótipos da fase 0
└─ .github/workflows/ci.yml   # build + clippy + testes em Windows e Linux
```

---

## Dados

### O que é coletado e com que frequência

| Dado | Coleta | Gravação no banco |
|---|---|---|
| CPU total e por núcleo, RAM, swap | 1 s | média/máximo a cada 10 s |
| Disco (uso %, leitura/escrita) e rede (up/down) | 1 s | média/máximo a cada 10 s |
| GPU (uso %, VRAM) | 2 s | média/máximo a cada 10 s |
| Temperaturas (CPU, GPU e demais sensores) | 5 s | média/máximo a cada 10 s |
| Top 10 processos por CPU e por RAM | 5 s | 1 amostra por minuto |
| App em foco + se o usuário está ocioso | 1 s | intervalos (início, fim, app) |
| Serviços vigiados | 5 s | apenas quando o estado muda |
| Alertas | quando disparam | 1 linha por alerta |

Isso gera cerca de 8.600 linhas de métricas por dia, ou uns 780 mil em 90 dias, o que o SQLite aguenta sem esforço.

### Schema (rascunho)

```sql
CREATE TABLE metricas         (ts INTEGER, chave TEXT, media REAL, maximo REAL);  -- ex.: 'cpu', 'ram', 'gpu', 'temp.cpu'
CREATE TABLE amostras_processo(ts INTEGER, nome TEXT, cpu REAL, mem_mb REAL);
CREATE TABLE uso_apps         (inicio INTEGER, fim INTEGER, exe TEXT, titulo TEXT, ocioso INTEGER);
CREATE TABLE eventos_servico  (ts INTEGER, servico TEXT, estado_antigo TEXT, estado_novo TEXT);
CREATE TABLE alertas          (ts INTEGER, tipo TEXT, mensagem TEXT, valor REAL);
CREATE TABLE relatorios       (dia TEXT PRIMARY KEY, caminho TEXT, gerado_em INTEGER);
CREATE INDEX idx_metricas_ts ON metricas(ts, chave);
```

---

## Interface (abas)

1. **Visão geral:** cards com CPU, RAM, GPU, temperaturas, disco e rede, cada um com mini-gráfico dos últimos 10 min. Os cards ficam amarelos ou vermelhos ao passar dos limites.
2. **Processos:** tabela ordenável (CPU, RAM, disco) com busca.
3. **Serviços:** lista de todos os serviços com filtro; um checkbox "vigiar" define quais entram nos alertas.
4. **Histórico:** escolher um dia e ver os gráficos, além dos botões **"Gerar relatório agora"** e "Abrir pasta de relatórios".
5. **Configurações:** horário do relatório, pasta de saída, limites de alerta, iniciar com o sistema e status das fontes de dados (ex.: LibreHardwareMonitor detectado, sessão Wayland).

---

## Alertas

| Regra (padrão, configurável) | Condição |
|---|---|
| CPU alta | > 90% por 5 min |
| RAM alta | > 90% por 5 min |
| Temperatura CPU | > 90 °C por 1 min |
| Temperatura GPU | > 85 °C por 1 min |
| Disco cheio | partição com < 10% livre |
| Serviço caiu | serviço vigiado saiu de "rodando" (Windows: `Running`; Linux: `active`) |

- **Histerese:** o alerta só "rearma" depois que o valor volta abaixo do limite (menos 5 pontos), para não gerar spam.
- **Cooldown:** no máximo uma notificação do mesmo tipo a cada 15 min.
- Todo alerta vai para a tabela `alertas` e aparece no relatório, mesmo quando a notificação é suprimida.

---

## Relatório de fim do dia

- **Quando:** às 23:00 (configurável) e pelo botão manual. Como os dados ficam no SQLite, o botão também gera relatórios de **dias passados**, o que cobre os dias em que o PC estava desligado no horário.
- **Onde:** `Documentos/Monitor/Relatorios/2026-09-26.html` (configurável).
- **Conteúdo:**
  1. Resumo: tempo ligado, tempo ativo e tempo ocioso, número de alertas.
  2. CPU, RAM, GPU: média, pico e horário do pico, gráfico do dia.
  3. Temperaturas: média, máxima, minutos acima do limite.
  4. Disco e rede: total lido/escrito, baixado/enviado, espaço livre por partição.
  5. Top 10 apps por **tempo em foco** e top 10 processos por consumo médio de CPU/RAM.
  6. Serviços: linha do tempo das quedas e reinícios dos serviços vigiados.
  7. Lista de alertas do dia.
- Um único arquivo HTML com CSS e SVG embutidos, que abre em qualquer navegador. Para gerar PDF, basta usar "Imprimir → Salvar como PDF".

---

## Fases de implementação

Cada fase entrega Windows e Linux juntos. A CI compila e testa nos dois sistemas a cada push.

| Fase | Entrega | Critério de pronto |
|---|---|---|
| **0. Base e protótipos de risco** | Projeto Cargo, CI (Windows + Linux), camada de plataforma com as traits, protótipos: (a) janela que esconde na bandeja e reabre, (b) temperaturas, (c) GPU AMD | Os três protótipos funcionando no seu PC, nos dois sistemas |
| **1. Coleta + banco** | `coleta.rs` e `armazenamento.rs` | Roda sem janela por 1 h e grava no SQLite |
| **2. Dashboard** | Aba Visão geral + Processos com dados ao vivo | Gráficos atualizando a 1 s, uso de CPU do próprio monitor < 2% |
| **3. Serviços** | SCM e systemd, aba Serviços, vigiar/desvigiar, eventos de mudança | Parar um serviço vigiado aparece no banco |
| **4. Uso de apps** | App em foco + ociosidade | Tempo por app bate com o uso real |
| **5. Alertas** | Regras, histerese, notificações | Teste de estresse dispara o alerta de CPU uma única vez |
| **6. Relatório** | HTML com gráficos, agendador, botão manual, aba Histórico | Relatório de um dia real gerado e revisado |
| **7. Acabamento** | Configurações, iniciar com o sistema, retenção de 90 dias, ícone, build release | Binário rodando por uma semana nos dois sistemas |

---

## Riscos conhecidos

| Risco | Mitigação |
|---|---|
| eframe não foi feito para "esconder na bandeja": com a janela oculta, `update()` não roda | Os eventos da bandeja são tratados em outra thread, que guarda o `egui::Context` e envia `ViewportCommand::Visible(true)`. Se não funcionar bem, a alternativa é fechar a janela de verdade e recriá-la, com a coleta seguindo independente. É o protótipo (a) da fase 0. |
| Bandeja no Linux depende de GTK e AppIndicator; o GNOME puro não mostra ícones de bandeja | O GTK roda numa thread própria. No GNOME, é preciso a extensão "AppIndicator Support" (já vem no Ubuntu). Sem bandeja, o programa abre com a janela visível e minimiza normalmente. |
| **Wayland** bloqueia a leitura do app em foco por segurança | Em Wayland, o "tempo por app" aparece como indisponível; a ociosidade continua via logind. Suporte específico a KDE/GNOME fica para depois. Em X11 funciona completo. |
| O LibreHardwareMonitor (Windows) precisa rodar como admin e usa um driver que alguns antivírus sinalizam | Ele é opcional: sem LHM, temperaturas aparecem como "indisponível" e o resto funciona. |
| Contadores PDH de GPU (Windows) têm uma instância por engine/processo | Somar por adaptador (`luid`) e pegar o máximo entre as engines, como faz o Gerenciador de Tarefas. |
| Uso de CPU do próprio monitor | Coleta de processos a cada 5 s, sem repaint da UI quando a janela está oculta, gravação em lote a cada 10 s. |
| Relatório no horário com o PC em suspensão | O agendador compara "agora ≥ horário" e verifica se o relatório de hoje já existe. Ao acordar, gera se ainda estiver no mesmo dia. Dias perdidos se geram pela aba Histórico. |

---

## Dependências de sistema (Linux, só para compilar)

```bash
sudo apt install libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
```

Para rodar, basta ter GTK 3 e `libayatana-appindicator3` instalados (padrão no Ubuntu/Mint).
