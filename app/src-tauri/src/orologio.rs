//! Lettura della memoria dell'orologio (protocollo Mares Darwin / M1, da libdivecomputer).
//! Viene inviato solo il comando di LETTURA 0x51: nessuna scrittura sull'orologio.

use std::io::{ErrorKind, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serialport::{ClearBuffer, SerialPort, SerialPortType};

pub const MEMORYSIZE: usize = 0x2000; // 8 KB: oltre questo indirizzo la memoria si ripete
const PACKETSIZE: usize = 0x20; // byte per richiesta (massimo del protocollo)
const TENTATIVI: usize = 5;
const PROLIFIC_VID: u16 = 0x067B;

/// Cerca la porta del cavo Prolific PL2303.
pub fn trova_porta() -> Result<String, String> {
    let porte = serialport::available_ports().map_err(|e| format!("Impossibile elencare le porte seriali: {e}"))?;
    for p in &porte {
        if let SerialPortType::UsbPort(u) = &p.port_type {
            if u.vid == PROLIFIC_VID {
                return Ok(p.port_name.clone());
            }
        }
    }
    let altre: Vec<&str> = porte.iter().map(|p| p.port_name.as_str()).collect();
    Err(format!(
        "Cavo non trovato: nessuna porta Prolific PL2303 (ID 067B:2303).\n\
         Controlla che il cavo sia collegato e che il driver sia quello giusto (pulsante \"Driver\").{}",
        if altre.is_empty() { String::new() } else { format!("\nPorte presenti: {}", altre.join(", ")) }
    ))
}

/// '<' + HEX(51 ind_lo ind_hi n) + HEX(checksum) + '>' (checksum = somma dei caratteri ASCII).
fn comando_lettura(indirizzo: usize, n: usize) -> Vec<u8> {
    let corpo = format!("{:02X}{:02X}{:02X}{:02X}", 0x51, indirizzo & 0xFF, indirizzo >> 8, n);
    let cs = corpo.bytes().map(u32::from).sum::<u32>() & 0xFF;
    format!("<{corpo}{cs:02X}>").into_bytes()
}

/// Verifica la risposta '<' + HEX(dati) + HEX(checksum) + '>' e restituisce i dati.
fn decodifica(r: &[u8], n: usize) -> Option<Vec<u8>> {
    if r.len() != 2 * (n + 2) || r[0] != b'<' || r[r.len() - 1] != b'>' {
        return None;
    }
    let corpo = &r[1..r.len() - 3];
    let somma = corpo.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    let hex = |s: &[u8]| u8::from_str_radix(std::str::from_utf8(s).ok()?, 16).ok();
    if somma != u32::from(hex(&r[r.len() - 3..r.len() - 1])?) {
        return None;
    }
    corpo.chunks(2).map(hex).collect()
}

/// Legge fino a `n` byte, o meno se scade il tempo.
fn leggi(porta: &mut dyn SerialPort, n: usize, attesa: Duration) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    let mut letti = 0;
    let inizio = Instant::now();
    while letti < n && inizio.elapsed() < attesa {
        match porta.read(&mut buf[letti..]) {
            Ok(k) => letti += k,
            Err(e) if e.kind() == ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
    buf.truncate(letti);
    buf
}

/// Tempo massimo di attesa perche' l'orologio entri in modalita' PC.
/// L'orologio "vede" il PC solo quando la porta e' aperta con DTR/RTS accesi
/// (alimentano le clip): mostra "PC" e risponde dopo circa 20 secondi.
const ATTESA_PC: Duration = Duration::from_secs(90);

/// Scarica gli 8 KB di memoria. `attesa` riceve i secondi trascorsi in attesa della
/// modalita' PC, `progresso` la percentuale del download (0-100).
pub fn scarica(porta_nome: &str, mut attesa: impl FnMut(u64), mut progresso: impl FnMut(u8)) -> Result<Vec<u8>, String> {
    let mut porta = serialport::new(porta_nome, 9600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| {
            format!(
                "Impossibile aprire {porta_nome}: {e}\n\
                 La porta potrebbe essere usata da un altro programma, oppure il driver non e' quello giusto."
            )
        })?;

    // DTR e RTS alimentano l'interfaccia del cavo.
    porta.write_data_terminal_ready(true).map_err(|e| e.to_string())?;
    porta.write_request_to_send(true).map_err(|e| e.to_string())?;
    sleep(Duration::from_millis(100));
    let _ = porta.clear(ClearBuffer::Input);

    // Attende che l'orologio entri in modalita' PC e risponda.
    let inizio = Instant::now();
    let primo = comando_lettura(0, PACKETSIZE);
    loop {
        porta.write_all(&primo).map_err(|e| format!("Errore di scrittura sulla porta: {e}"))?;
        leggi(porta.as_mut(), primo.len(), Duration::from_secs(1));
        let r = leggi(porta.as_mut(), 2 * (PACKETSIZE + 2), Duration::from_secs(1));
        if decodifica(&r, PACKETSIZE).is_some() {
            break;
        }
        if inizio.elapsed() > ATTESA_PC {
            return Err(String::from(
                "L'orologio non risponde.\n\
                 - Appoggialo sulla basetta con le clip ben a contatto: dopo qualche secondo deve comparire \"PC\".\n\
                 - Pulisci le clip e i contatti e riprova.\n\
                 - Se l'orologio emette suoni, si spegne o si riavvia, la pila e' scarica.",
            ));
        }
        attesa(inizio.elapsed().as_secs());
        sleep(Duration::from_millis(500));
        let _ = porta.clear(ClearBuffer::Input);
    }

    let mut mem = Vec::with_capacity(MEMORYSIZE);
    let mut ultimo = u8::MAX;
    for ind in (0..MEMORYSIZE).step_by(PACKETSIZE) {
        let cmd = comando_lettura(ind, PACKETSIZE);
        let mut dati = None;
        for _ in 0..TENTATIVI {
            sleep(Duration::from_millis(50));
            porta.write_all(&cmd).map_err(|e| format!("Errore di scrittura sulla porta: {e}"))?;
            leggi(porta.as_mut(), cmd.len(), Duration::from_secs(1)); // eco del comando (contatti in comune)
            let r = leggi(porta.as_mut(), 2 * (PACKETSIZE + 2), Duration::from_secs(1));
            dati = decodifica(&r, PACKETSIZE);
            if dati.is_some() {
                break;
            }
            sleep(Duration::from_millis(100));
            let _ = porta.clear(ClearBuffer::Input);
        }
        let dati = dati.ok_or_else(|| {
            String::from(
                "Nessuna risposta valida dall'orologio.\n\
                 - Controlla che sia appoggiato sulla basetta e che sul display compaia \"PC\".\n\
                 - Pulisci le clip e i contatti e riprova.\n\
                 - Se l'orologio emette suoni, si spegne o si riavvia, la pila e' scarica.",
            )
        })?;
        mem.extend_from_slice(&dati);
        let pct = (mem.len() * 100 / MEMORYSIZE) as u8;
        if pct != ultimo {
            ultimo = pct;
            progresso(pct);
        }
    }
    Ok(mem)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn comando_di_esempio() {
        // Esempio documentato: 16 byte dall'indirizzo 0.
        assert_eq!(comando_lettura(0, 0x10), b"<5100001087>");
    }

    #[test]
    fn risposta_valida_e_non() {
        let corpo = "00".repeat(PACKETSIZE);
        let cs = corpo.bytes().map(u32::from).sum::<u32>() & 0xFF;
        let ok = format!("<{corpo}{cs:02X}>");
        assert_eq!(decodifica(ok.as_bytes(), PACKETSIZE), Some(vec![0; PACKETSIZE]));
        let ko = format!("<{corpo}{:02X}>", (cs + 1) & 0xFF);
        assert_eq!(decodifica(ko.as_bytes(), PACKETSIZE), None);
    }
}
