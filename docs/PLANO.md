# Plano — Monitor de Desempenho e Serviços para Windows

## Decisões tomadas

| Tema | Decisão |
|---|---|
| Sistema | Windows 10/11 |
| Linguagem | Rust (100%) |
| Interface | `egui` / `eframe` com gráficos em `egui_plot` |
| Execução | Um único `.exe` que inicia com o Windows e fica na bandeja do sistema, coletando mesmo com a janela fechada |
| Métricas | CPU, RAM, disco, rede, temperaturas, GPU, serviços, processos e tempo de uso por aplicativo |
| GPU | AMD: uso e VRAM pelos contadores do Windows (PDH); temperatura pelo LibreHardwareMonitor |
| Temperatura da CPU | Lida do LibreHardwareMonitor via WMI (opcional: sem ele, o resto funciona) |
| Serviços | Lista escolhida pelo usuário na interface |
| Alertas | Notificação do Windows (toast) **e** registro no relatório |
| Relatório | HTML autocontido com gráficos SVG, gerado em horário fixo **e** por um botão manual |
| Histórico | SQLite, 90 dias de retenção |
| Horário do relatório | 23:00 (padrão, configurável) |
| Idioma da interface | Português (pt-BR) |
| Distribuição | `.exe` avulso (build release, sem instalador) |

---

## Arquitetura

```
┌──────────────────────────── monitor.exe ────────────────────────────┐
│                                                                     │
│  Thread coletora (1 s)          Thread de persistência / agendador  │
│  ├─ sysinfo  → CPU/RAM/disco/rede/processos                         │
│  ├─ PDH      → uso GPU / VRAM  ──► canal ──► SQLite (WAL)           │
│  ├─ WMI/LHM  → temperaturas              ├─ regras de alerta → toast│
│  ├─ SCM      → estado dos serviços       ├─ relatório no horário    │
│  └─ Win32    → app em foco / ocioso      └─ limpeza (> 90 dias)     │
│         │                                                           │
│         ▼ estado ao vivo (Arc<RwLock<…>>, buffer circular 10 min)   │
│  UI egui (janela) ◄──── ícone de bandeja (tray-icon)                │
└─────────────────────────────────────────────────────────────────────┘
```

A coleta roda em threads próprias, independentes da janela. Fechar a janela só a esconde; "Sair" fica no menu da bandeja.

### Crates previstas

| Função | Crate |
|---|---|
| Janela e gráficos | `eframe`, `egui`, `egui_plot` |
| Ícone de bandeja | `tray-icon` |
| CPU, RAM, disco, rede, processos | `sysinfo` |
| Serviços, PDH, janela em foco, ociosidade | `windows` (windows-rs) e/ou `windows-service` |
| Leitura do LibreHardwareMonitor | `wmi` (namespace `root\LibreHardwareMonitor`, classe `Sensor`) |
| Banco de dados | `rusqlite` (feature `bundled`) |
| Notificações | `tauri-winrt-notification` |
| Relatório HTML | `askama` (template) + `plotters` (backend SVG) |
| Configuração | `serde` + `toml`, salvo em `%APPDATA%\monitor\config.toml` |
| Iniciar com o Windows | `auto-launch` (chave `HKCU\...\Run`, sem precisar de admin) |
| Logs | `tracing` + `tracing-appender` |
| Datas | `chrono` |

### Estrutura do projeto

```
monitor/
├─ Cargo.toml
├─ src/
│  ├─ main.rs            # inicialização, bandeja, threads
│  ├─ config.rs          # limites, serviços vigiados, horário do relatório
│  ├─ collect/
│  │  ├─ system.rs       # sysinfo: CPU, RAM, disco, rede
│  │  ├─ gpu.rs          # PDH: uso e VRAM
│  │  ├─ sensors.rs      # WMI/LHM: temperaturas
│  │  ├─ services.rs     # SCM: estado dos serviços
│  │  └─ apps.rs         # app em foco, tempo ocioso
│  ├─ storage.rs         # SQLite: schema, inserts, consultas, retenção
│  ├─ alerts.rs          # regras, histerese, toasts
│  ├─ report/
│  │  ├─ mod.rs          # agregação do dia
│  │  ├─ charts.rs       # SVG com plotters
│  │  └─ templates/day.html
│  └─ ui/
│     ├─ dashboard.rs    # visão geral ao vivo
│     ├─ processes.rs
│     ├─ services.rs
│     ├─ history.rs      # dias anteriores + botão "gerar relatório"
│     └─ settings.rs
└─ .github/workflows/ci.yml   # build + clippy + testes no windows-latest
```

---

## Dados

### O que é coletado e com que frequência

| Dado | Coleta | Gravação no banco |
|---|---|---|
| CPU total e por núcleo, RAM, swap | 1 s | média/máximo a cada 10 s |
| Disco (uso %, leitura/escrita) e rede (up/down) | 1 s | média/máximo a cada 10 s |
| GPU (uso %, VRAM) | 2 s | média/máximo a cada 10 s |
| Temperaturas (CPU, GPU e o que o LHM expuser) | 5 s | média/máximo a cada 10 s |
| Top 10 processos por CPU e por RAM | 5 s | 1 amostra por minuto |
| App em foco + se o usuário está ocioso | 1 s | intervalos (início, fim, app) |
| Serviços vigiados | 5 s | apenas quando o estado muda |
| Alertas | quando disparam | 1 linha por alerta |

Isso gera cerca de 8.600 linhas de métricas por dia, ou uns 780 mil em 90 dias, o que o SQLite aguenta sem esforço.

### Schema (rascunho)

```sql
CREATE TABLE metrics      (ts INTEGER, key TEXT, avg REAL, max REAL);           -- ex.: key='cpu', 'ram', 'gpu', 'temp.cpu'
CREATE TABLE proc_samples (ts INTEGER, name TEXT, cpu REAL, mem_mb REAL);
CREATE TABLE app_usage    (start INTEGER, end INTEGER, exe TEXT, title TEXT, idle INTEGER);
CREATE TABLE service_events(ts INTEGER, service TEXT, old_state TEXT, new_state TEXT);
CREATE TABLE alerts       (ts INTEGER, kind TEXT, message TEXT, value REAL);
CREATE TABLE reports      (day TEXT PRIMARY KEY, path TEXT, generated_at INTEGER);
CREATE INDEX idx_metrics_ts ON metrics(ts, key);
```

---

## Interface (abas)

1. **Visão geral:** cards com CPU, RAM, GPU, temperaturas, disco e rede, cada um com mini-gráfico dos últimos 10 min. Os cards ficam amarelos ou vermelhos ao passar dos limites.
2. **Processos:** tabela ordenável (CPU, RAM, disco) com busca.
3. **Serviços:** lista de todos os serviços com filtro; um checkbox "vigiar" define quais entram nos alertas.
4. **Histórico:** escolher um dia e ver os gráficos, além dos botões **"Gerar relatório agora"** e "Abrir pasta de relatórios".
5. **Configurações:** horário do relatório, pasta de saída, limites de alerta, iniciar com o Windows e status do LibreHardwareMonitor (detectado ou não).

---

## Alertas

| Regra (padrão, configurável) | Condição |
|---|---|
| CPU alta | > 90% por 5 min |
| RAM alta | > 90% por 5 min |
| Temperatura CPU | > 90 °C por 1 min |
| Temperatura GPU | > 85 °C por 1 min |
| Disco cheio | partição com < 10% livre |
| Serviço caiu | serviço vigiado saiu de "Running" |

- **Histerese:** o alerta só "rearma" depois que o valor volta abaixo do limite (menos 5 pontos), para não gerar spam.
- **Cooldown:** no máximo uma notificação do mesmo tipo a cada 15 min.
- Todo alerta vai para a tabela `alerts` e aparece no relatório, mesmo quando a notificação é suprimida.

---

## Relatório de fim do dia

- **Quando:** no horário configurado (23:00) e pelo botão manual. Como os dados ficam no SQLite, o botão também gera relatórios de **dias passados**, o que cobre os dias em que o PC estava desligado no horário.
- **Onde:** `Documentos\Monitor\Relatorios\2026-09-26.html` (configurável).
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

| Fase | Entrega | Critério de pronto |
|---|---|---|
| **0. Base e protótipos de risco** | Cargo, CI no `windows-latest`, protótipos: (a) eframe escondendo na bandeja e reabrindo, (b) leitura do LHM via WMI, (c) GPU AMD via PDH | Os três protótipos funcionando no seu PC |
| **1. Coleta + banco** | Módulos `collect/*` (menos apps) e `storage.rs` | Roda headless por 1 h e grava no SQLite |
| **2. Dashboard** | Aba Visão geral + Processos com dados ao vivo | Gráficos atualizando a 1 s, uso de CPU do próprio monitor < 2% |
| **3. Serviços** | Aba Serviços, vigiar/desvigiar, eventos de mudança | Parar um serviço vigiado aparece no banco |
| **4. Uso de apps** | App em foco + ociosidade | Tempo por app bate com o uso real |
| **5. Alertas** | Regras, histerese, toasts | Teste de estresse dispara o alerta de CPU uma única vez |
| **6. Relatório** | HTML com gráficos, agendador, botão manual, aba Histórico | Relatório de um dia real gerado e revisado |
| **7. Acabamento** | Configurações, iniciar com Windows, retenção de 90 dias, ícone, build release | `.exe` único instalado e rodando por uma semana |

---

## Riscos conhecidos

| Risco | Mitigação |
|---|---|
| eframe não foi feito para "esconder na bandeja": com a janela oculta, `update()` não roda | Os eventos da bandeja são tratados em outra thread, que guarda o `egui::Context` e envia `ViewportCommand::Visible(true)`. Se não funcionar bem, a alternativa é fechar a janela de verdade e recriá-la, com a coleta seguindo independente. É o protótipo (a) da fase 0. |
| O LibreHardwareMonitor precisa rodar como admin e usa um driver que alguns antivírus sinalizam | Ele é opcional: sem LHM, temperaturas aparecem como "indisponível" e o resto funciona. A aba Configurações mostra o status. |
| Contadores PDH de GPU têm uma instância por engine/processo | Somar por adaptador (`luid`) e pegar o máximo entre as engines 3D, Compute e Video, como faz o Gerenciador de Tarefas. |
| Uso de CPU do próprio monitor | Coleta de processos a cada 5 s, sem repaint da UI quando a janela está oculta, gravação em lote a cada 10 s. |
| Relatório no horário com o PC em suspensão | O agendador compara "agora ≥ horário" e verifica se o relatório de hoje já existe. Ao acordar, gera se ainda estiver no mesmo dia. Dias perdidos se geram pela aba Histórico. |

---
