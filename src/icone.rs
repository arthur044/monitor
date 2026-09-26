//! Ícone do programa, desenhado em código para não depender de arquivos.

/// Quadrado arredondado verde-azulado com três barras brancas, em RGBA.
pub fn rgba(lado: u32) -> Vec<u8> {
    const FUNDO: [u8; 4] = [15, 118, 110, 255];
    const BARRA: [u8; 4] = [255, 255, 255, 255];
    // (início em x, topo em y), em fração do lado. Largura 0,16; base em 0,8.
    const BARRAS: [(f32, f32); 3] = [(0.20, 0.55), (0.42, 0.35), (0.64, 0.18)];

    let l = lado as f32;
    let raio = l * 0.2;
    let mut pixels = vec![0u8; (lado * lado * 4) as usize];

    for y in 0..lado {
        for x in 0..lado {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let dx = (raio - px).max(px - (l - raio)).max(0.0);
            let dy = (raio - py).max(py - (l - raio)).max(0.0);
            if dx * dx + dy * dy > raio * raio {
                continue; // fora do canto arredondado: transparente
            }
            let (nx, ny) = (px / l, py / l);
            let na_barra = BARRAS
                .iter()
                .any(|&(x0, topo)| nx >= x0 && nx < x0 + 0.16 && ny >= topo && ny < 0.8);
            let i = ((y * lado + x) * 4) as usize;
            pixels[i..i + 4].copy_from_slice(if na_barra { &BARRA } else { &FUNDO });
        }
    }
    pixels
}
