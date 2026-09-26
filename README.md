# Monitor

Monitor de desempenho e serviços para Windows e Linux, escrito em Rust, com
interface gráfica, ícone na bandeja e relatório diário em HTML.

O plano completo está em [`docs/PLANO.md`](docs/PLANO.md).

## Status: Fase 0 (base e protótipos de risco)

Esta fase valida as partes mais arriscadas antes de construir o resto:

| Protótipo | Onde |
|---|---|
| (a) Janela que esconde na bandeja e reabre, com a coleta rodando em segundo plano | `monitor` |
| (b) Temperaturas (Windows: LibreHardwareMonitor; Linux: sensores do kernel) | `monitor` e `diagnostico` |
| (c) Uso e VRAM da GPU AMD (Windows: contadores PDH; Linux: driver `amdgpu`) | `monitor` e `diagnostico` |

## Como testar

### Baixando os executáveis prontos

1. Abra a aba **Actions** do repositório no GitHub e clique na execução mais recente do CI.
2. Em **Artifacts**, baixe `monitor-Windows` ou `monitor-Linux`.
3. Descompacte. Há dois programas:
   - `monitor`: a janela com temperaturas e GPU ao vivo.
   - `diagnostico`: imprime as mesmas leituras no terminal, cinco vezes.

### Compilando

Requer Rust 1.95 ou mais novo (`rustup update stable`).

```bash
cargo run                        # janela
cargo run --example diagnostico  # terminal
```

No Linux, instale antes as dependências de compilação:

```bash
sudo apt install libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
```

### Windows: temperaturas

Baixe o [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/releases)
e abra-o **como administrador** antes do monitor. Sem ele, as temperaturas
aparecem como indisponíveis e o resto funciona normalmente.

### O que verificar

1. **Temperaturas** aparecem para CPU e GPU.
2. **GPU**: a placa AMD aparece com uso e VRAM. Abra um jogo ou vídeo e veja o uso subir.
3. **Bandeja**: feche a janela; o ícone continua na bandeja. Espere um pouco e
   reabra (Windows: clique esquerdo no ícone ou "Abrir monitor" no menu; Linux: menu do ícone).
   O contador "Leituras feitas" deve ter continuado subindo.
4. **Sair** pelo menu do ícone encerra o programa e remove o ícone.

No GNOME, o ícone da bandeja precisa da extensão "AppIndicator Support" (já vem no Ubuntu).
