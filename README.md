<div align="center">

# ✋ motion_track

**Controle o mouse com as mãos, usando apenas uma webcam.**

Rastreamento de landmarks da mão em tempo real, escrito em Rust.

![Rust](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/plataforma-Windows-0078D6?logo=windows&logoColor=white)
![Status](https://img.shields.io/badge/status-em_desenvolvimento-orange)

</div>

---

## Sobre

`motion_track` captura o vídeo da sua webcam, detecta os pontos-chave (*landmarks*) da mão com um modelo de visão computacional e transforma esses movimentos em comandos de mouse: mover o cursor, clicar e arrastar, sem tocar em nada.

> [!NOTE]
> **Estado atual:** a captura da câmera e a exibição do vídeo em tempo real estão funcionando. O próximo passo é a inferência dos landmarks.

## Pipeline

```mermaid
flowchart LR
    A[📷 Câmera] -->|frames| B[🧠 Modelo]
    B -->|landmarks| C[💾 Estado + suavização]
    C --> D[🖥️ UI de debug]
    C --> E[🖱️ Controle do mouse]
```

| # | Etapa | Descrição | Status |
|:-:|-------|-----------|:------:|
| 1 | **Captura** | Lê os frames da webcam | ✅ |
| 2 | **Inferência** | Envia cada frame ao modelo, que extrai os landmarks da mão | ⏳ |
| 3 | **Estado** | Armazena e suaviza as posições dos landmarks | ⏳ |
| 4 | **Visualização** | Desenha o vídeo e os landmarks numa janela | 🟡 vídeo pronto |
| 5 | **Controle** | Converte os landmarks em movimento e cliques do mouse | ⏳ |

## Arquitetura atual

```mermaid
flowchart LR
    subgraph T1[Thread de captura]
        A[Camera::frame] --> B[decode RGBA]
        B --> C[espelhar]
        C --> D[RGBA → BGRA]
        D --> E["Arc&lt;RenderImage&gt;"]
    end

    subgraph T2[Thread principal · GPUI]
        F[task assíncrona] --> G[MainWindow]
        G --> H["img() na tela"]
    end

    E -->|"watch (só o frame mais recente)"| F
```

- **Captura** roda numa `std::thread` dedicada, já que `Camera::frame()` é bloqueante.
- **UI** roda na thread principal, como o GPUI exige.
- **Canal `tokio::sync::watch`** guarda apenas o último frame: se a UI atrasar, ela pula direto para o mais recente, sem acumular fila nem latência.
- A cada frame novo, a textura do anterior é liberada da GPU com `drop_image`, o que evita vazamento de memória de vídeo.

## Checklist

### 1. 📷 Captura
- [x] Listar as câmeras disponíveis
- [x] Abrir a câmera e capturar frames
- [x] Captura contínua numa thread dedicada
- [x] Decodificar o frame (RGBA)
- [x] Espelhar o frame na horizontal
- [x] Converter RGBA → BGRA para o GPUI
- [ ] Fixar uma resolução leve (ex.: 640×480). Hoje usa `AbsoluteHighestFrameRate`
- [ ] Tratar erros sem `unwrap` (ignorar frames corrompidos, parar só em falhas fatais)
- [ ] Manter uma cópia em RGB para o modelo

### 2. 🧠 Inferência
- [ ] Escolher e baixar os modelos (detector de palma + landmarks, em ONNX)
- [ ] Integrar o runtime de inferência ([`ort`](https://crates.io/crates/ort))
- [ ] Pré-processar o frame (resize com letterbox + normalização)
- [ ] Rodar o detector de palma
- [ ] Recortar a mão e rodar o modelo de landmarks (21 pontos)
- [ ] Reaproveitar o recorte do frame anterior (rodar o detector só quando a mão for perdida)

### 3. 💾 Estado
- [ ] Estrutura para guardar os 21 landmarks por frame
- [ ] Suavização com One Euro Filter
- [ ] Compartilhar o estado entre as threads

### 4. 🖥️ Visualização
- [x] Janela base com GPUI
- [x] Mostrar o dispositivo selecionado
- [x] Exibir o vídeo da câmera em tempo real (`RenderImage` em BGRA)
- [x] Liberar a textura do frame anterior da GPU (`drop_image`)
- [ ] Desenhar os landmarks e as conexões sobre o vídeo
- [ ] Mostrar o FPS e a latência
- [ ] Sliders para calibração

### 5. 🖱️ Controle do mouse
- [ ] Integrar o [`enigo`](https://crates.io/crates/enigo)
- [ ] Mapear uma área útil da câmera para a tela inteira
- [ ] Mover o cursor com o indicador
- [ ] Clique por pinça (polegar + indicador), normalizado pelo tamanho da mão
- [ ] Histerese no clique, para ele não ficar "piscando"
- [ ] Arrastar (manter a pinça)
- [ ] Atalho para ligar e desligar o controle

### ⚙️ Arquitetura
- [x] Canal `watch` entre a captura e a UI (só o frame mais recente)
- [x] UI separada em módulo próprio (`ui.rs`)
- [ ] Canal entre a câmera e a inferência, também descartando frames velhos
- [ ] Encerramento limpo: parar a captura quando a janela fechar
- [ ] Separar o restante em módulos (`camera`, `model`, `tracking`, `input`)

## Stack

| Crate | Uso |
|-------|-----|
| [`nokhwa`](https://crates.io/crates/nokhwa) | Captura da webcam |
| [`image`](https://crates.io/crates/image) | Buffers de imagem e `Frame` para o `RenderImage` |
| [`gpui-kit`](https://crates.io/crates/gpui-kit) | Interface gráfica acelerada por GPU |
| [`tokio`](https://crates.io/crates/tokio) | Canal `watch` entre a captura e a UI |
| [`tokio-util`](https://crates.io/crates/tokio-util) | `CancellationToken` para parar a captura |
| [`tracing`](https://crates.io/crates/tracing) | Logs estruturados |
| [`anyhow`](https://crates.io/crates/anyhow) | Tratamento de erros |
| [`ort`](https://crates.io/crates/ort) *(planejado)* | Inferência dos modelos ONNX |
| [`enigo`](https://crates.io/crates/enigo) *(planejado)* | Controle do mouse |

## Como rodar

**Pré-requisitos:** [Rust](https://rustup.rs) atualizado (edition 2024) e uma webcam conectada.

```bash
git clone <url-do-repositorio>
cd motion_track
cargo run
```

> [!TIP]
> O perfil `dev` já está configurado para otimizar as dependências (`opt-level = 3`) e o código do projeto (`opt-level = 1`). Assim o `cargo run` comum já roda com FPS bom. Para a versão final, use `cargo run --release`.

> [!NOTE]
> Por enquanto o app usa sempre a **primeira câmera** encontrada.

## Estrutura

```
motion_track/
├── src/
│   ├── main.rs      # entrada: thread de captura + inicialização do GPUI
│   └── ui.rs        # MainWindow: recebe os frames e desenha o vídeo
└── Cargo.toml
```

---

<div align="center">
<sub>Feito com 🦀 Rust</sub>
</div>
