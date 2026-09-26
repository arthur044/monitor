//! Agregação dos contadores de GPU do Windows (PDH).
//!
//! O contador `\GPU Engine(*)\Utilization Percentage` tem uma instância por
//! processo e por engine, com nomes como
//! `pid_4312_luid_0x00000000_0x0000D1A5_phys_0_eng_3_engtype_VideoDecode`.
//! O Gerenciador de Tarefas soma os processos de cada engine e mostra, por
//! placa, a engine mais ocupada. Fazemos o mesmo.
//!
//! Fica separado do código Windows para os testes rodarem em qualquer SO.

use std::collections::HashMap;

/// Chave de placa no formato usado pelo PDH, em minúsculas: `luid_0x00000000_0x0000d1a5`.
pub fn chave_de_luid(parte_alta: i32, parte_baixa: u32) -> String {
    format!("luid_0x{:08x}_0x{:08x}", parte_alta as u32, parte_baixa)
}

/// Extrai a chave de placa (`luid_…`) do nome de uma instância.
pub fn chave_da_instancia(instancia: &str) -> Option<String> {
    let resto = &instancia[instancia.find("luid_")?..];
    let fim = resto.find("_phys").unwrap_or(resto.len());
    Some(resto[..fim].to_ascii_lowercase())
}

/// Uso por placa (0–100), a partir das instâncias de `GPU Engine`.
pub fn uso_por_placa<'a>(itens: impl IntoIterator<Item = (&'a str, f64)>) -> HashMap<String, f64> {
    let mut por_engine: HashMap<String, f64> = HashMap::new();
    for (instancia, valor) in itens {
        // A engine é tudo a partir de "luid_", sem o "pid_N_" do começo.
        let Some(inicio) = instancia.find("luid_") else {
            continue;
        };
        *por_engine
            .entry(instancia[inicio..].to_ascii_lowercase())
            .or_default() += valor;
    }

    let mut por_placa: HashMap<String, f64> = HashMap::new();
    for (engine, uso) in por_engine {
        let Some(placa) = chave_da_instancia(&engine) else {
            continue;
        };
        let atual = por_placa.entry(placa).or_default();
        *atual = atual.max(uso.min(100.0));
    }
    por_placa
}

/// Soma de valores por placa, para contadores como `GPU Adapter Memory`.
pub fn soma_por_placa<'a>(itens: impl IntoIterator<Item = (&'a str, f64)>) -> HashMap<String, f64> {
    let mut por_placa: HashMap<String, f64> = HashMap::new();
    for (instancia, valor) in itens {
        if let Some(placa) = chave_da_instancia(instancia) {
            *por_placa.entry(placa).or_default() += valor;
        }
    }
    por_placa
}

#[cfg(test)]
mod testes {
    use super::*;

    const PLACA: &str = "luid_0x00000000_0x0000d1a5";

    #[test]
    fn chave_de_luid_bate_com_o_formato_do_pdh() {
        assert_eq!(chave_de_luid(0, 0xD1A5), PLACA);
        assert_eq!(chave_de_luid(-1, 1), "luid_0xffffffff_0x00000001");
    }

    #[test]
    fn extrai_chave_da_instancia() {
        assert_eq!(
            chave_da_instancia("pid_1_luid_0x00000000_0x0000D1A5_phys_0_eng_0_engtype_3D")
                .as_deref(),
            Some(PLACA)
        );
        assert_eq!(
            chave_da_instancia("luid_0x00000000_0x0000D1A5_phys_0").as_deref(),
            Some(PLACA)
        );
        assert_eq!(chave_da_instancia("_Total"), None);
    }

    #[test]
    fn uso_soma_processos_e_pega_a_engine_mais_ocupada() {
        let itens = [
            // Engine 3D: dois processos somam 50%.
            (
                "pid_10_luid_0x00000000_0x0000D1A5_phys_0_eng_0_engtype_3D",
                30.0,
            ),
            (
                "pid_20_luid_0x00000000_0x0000D1A5_phys_0_eng_0_engtype_3D",
                20.0,
            ),
            // Engine de vídeo: 70%, é a mais ocupada.
            (
                "pid_20_luid_0x00000000_0x0000D1A5_phys_0_eng_3_engtype_VideoDecode",
                70.0,
            ),
            // Outra placa.
            (
                "pid_10_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D",
                5.0,
            ),
        ];
        let uso = uso_por_placa(itens);
        assert_eq!(uso[PLACA], 70.0);
        assert_eq!(uso["luid_0x00000000_0x0000aaaa"], 5.0);
    }

    #[test]
    fn uso_nao_passa_de_100() {
        let itens = [
            (
                "pid_1_luid_0x00000000_0x0000D1A5_phys_0_eng_0_engtype_3D",
                80.0,
            ),
            (
                "pid_2_luid_0x00000000_0x0000D1A5_phys_0_eng_0_engtype_3D",
                80.0,
            ),
        ];
        assert_eq!(uso_por_placa(itens)[PLACA], 100.0);
    }

    #[test]
    fn memoria_soma_por_placa() {
        let itens = [
            ("luid_0x00000000_0x0000D1A5_phys_0", 1024.0),
            ("luid_0x00000000_0x0000D1A5_phys_1", 512.0),
        ];
        assert_eq!(soma_por_placa(itens)[PLACA], 1536.0);
    }
}
