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
> **Estado atual:** a captura da câmera e a exibição do vídeo em tempo real estão funcionando. A câmera agora tenta abrir num formato já próximo da resolução do modelo (640 px de largura). O modelo escolhido é o **YOLO26 hand pose** (ONNX) e o `ort` já foi adicionado ao projeto. O pré-processamento com letterbox está em protótipo. O próximo passo é rodar o modelo na thread de inferência.

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
| 2 | **Inferência** | Envia cada frame ao modelo, que extrai os landmarks da mão | 🟡 modelo e runtime prontos |
| 3 | **Estado** | Armazena e suaviza as posições dos landmarks | ⏳ |
| 4 | **Visualização** | Desenha o vídeo e os landmarks numa janela | 🟡 vídeo pronto |
| 5 | **Controle** | Converte os landmarks em movimento e cliques do mouse | ⏳ |

## Arquitetura atual

```mermaid
flowchart LR
    subgraph T1[Thread de captura]
        A[Camera::frame] --> B[decode RGBA]
        B --> C[espelhar]
        C --> R[cópia RGBA]
        C --> D[RGBA → BGRA]
        D --> E["Arc&lt;RenderImage&gt;"]
    end

    subgraph T3[Thread de inferência]
        I[recebe o frame mais recente] --> K["letterbox 640×640 (protótipo)"]
        K --> J["YOLO26 hand pose (a fazer)"]
    end

    subgraph T2[Thread principal · GPUI]
        F[task assíncrona] --> G[MainWindow]
        G --> H["img() na tela"]
    end

    R -->|watch| I
    E -->|"watch (só o frame mais recente)"| F
```

- **Captura** roda numa `std::thread` dedicada, já que `Camera::frame()` é bloqueante. O `Camera` do `nokhwa` não é `Send`, então ele é criado **dentro** da thread. De fora, ela só recebe o `CameraIndex`.
- **Escolha do formato da câmera:** a câmera abre primeiro com `AbsoluteHighestFrameRate`. A partir da resolução nativa, calcula-se a resolução alvo com 640 px de largura mantendo a proporção (`calculate_aspect_ratio_from`). Se a câmera tiver um formato `NV12` exatamente nessa resolução, ela é reaberta nele (o de maior FPS). Se não tiver, mantém o formato original e a resolução alvo segue junto com o frame, para a inferência redimensionar.
- **Inferência** roda noutra `std::thread`, porque o modelo vai consumir CPU continuamente. Ela recebe uma cópia RGBA do frame (feita antes da troca para BGRA) e, opcionalmente, a resolução para redimensionar (`LastFrameType`).
- **Letterbox (protótipo):** o frame é redimensionado para 640 px de largura e centralizado verticalmente numa imagem 640×640 preenchida com cinza (114, 114, 114), o padrão do YOLO.
- Se a câmera não abrir, a thread de captura cancela o token e retorna o erro, em vez de dar `unwrap`.
- **UI** roda na thread principal, como o GPUI exige.
- **Canal `tokio::sync::watch`** guarda apenas o último frame: se a UI atrasar, ela pula direto para o mais recente, sem acumular fila nem latência.
- A cada frame novo, a textura do anterior é liberada da GPU com `drop_image`, o que evita vazamento de memória de vídeo.
- **`CancellationToken`** compartilhado entre as threads: se uma falha, cancela o token para as outras pararem. Para encerrar o app usa-se `cx.quit()`, não `shutdown()`, porque o `shutdown()` só fecha as janelas e deixa o loop de eventos rodando.

## Checklist

### 1. 📷 Captura
- [x] Listar as câmeras disponíveis
- [x] Abrir a câmera e capturar frames
- [x] Captura contínua numa thread dedicada
- [x] Decodificar o frame (RGBA)
- [x] Espelhar o frame na horizontal
- [x] Converter RGBA → BGRA para o GPUI
- [x] Fixar uma resolução leve: procura um formato `NV12` com 640 px de largura na proporção nativa da câmera
- [x] Fallback: se não houver esse formato, enviar a resolução alvo junto com o frame para redimensionar depois
- [x] Módulo próprio para a captura (`capture.rs`)
- [x] Validar o índice da câmera e falhar se nenhuma for encontrada
- [x] Erros tipados na captura (`GetFrameError` com `thiserror`)
- [x] Cancelar o token em vez de dar `unwrap` quando a câmera não abre
- [ ] Tratar os erros restantes sem `unwrap` (ignorar frames corrompidos, parar só em falhas fatais)
- [x] Manter uma cópia do frame para o modelo (RGBA, antes da troca para BGRA)

### 2. 🧠 Inferência
- [x] Thread dedicada recebendo o frame mais recente (por enquanto só lê o frame)
- [ ] Esperar o frame novo sem busy-wait (`changed()` em vez de `has_changed()` em loop)
- [x] Escolher e baixar o modelo: YOLO26 hand pose (`models/yolo26_hand_pose_fp32.onnx`), que detecta a mão e os landmarks numa passada só
- [x] Adicionar o runtime de inferência ([`ort`](https://crates.io/crates/ort), com a feature `cuda`)
- [ ] Pré-processar o frame: letterbox 640×640 (protótipo comentado no `main.rs`) + normalização
- [ ] Carregar o modelo numa `Session` do `ort`
- [ ] Rodar o modelo e decodificar a saída (caixa + landmarks)

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
- [x] Canal entre a câmera e a inferência, também descartando frames velhos
- [x] Módulo de captura separado (`capture.rs`)
- [ ] Encerramento limpo: cancelar o token quando a janela fechar e fazer `join` nas threads
- [ ] Fechar a UI (`cx.quit()`) quando uma thread cancelar o token
- [ ] Separar o restante em módulos (`model`, `tracking`, `input`)

## Stack

| Crate | Uso |
|-------|-----|
| [`nokhwa`](https://crates.io/crates/nokhwa) | Captura da webcam |
| [`image`](https://crates.io/crates/image) | Buffers de imagem e `Frame` para o `RenderImage` |
| [`gpui-kit`](https://crates.io/crates/gpui-kit) | Interface gráfica acelerada por GPU |
| [`tokio`](https://crates.io/crates/tokio) | Canais `watch` entre as threads (sem runtime, só `tokio::sync`) |
| [`tokio-util`](https://crates.io/crates/tokio-util) | `CancellationToken` para parar as threads |
| [`tracing`](https://crates.io/crates/tracing) | Logs estruturados |
| [`anyhow`](https://crates.io/crates/anyhow) | Tratamento de erros |
| [`thiserror`](https://crates.io/crates/thiserror) | Erros tipados da captura (`GetFrameError`) |
| [`ort`](https://crates.io/crates/ort) | Inferência do modelo ONNX (feature `cuda`) |
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
│   ├── main.rs      # entrada: threads de captura e inferência + inicialização do GPUI
│   ├── capture.rs   # Capture: lista as câmeras e processa cada frame
│   ├── ui.rs        # MainWindow: recebe os frames e desenha o vídeo
│   └── utils.rs     # LastFrameType e cálculo da resolução alvo
├── models/
│   └── yolo26_hand_pose_fp32.onnx   # modelo de pose da mão
└── Cargo.toml
```

---

<div align="center">
<sub>Feito com 🦀 Rust</sub>
</div>
