//! Controllo e installazione del driver Prolific PL2303 3.3.2.102.
//!
//! I driver Prolific recenti (3.9.x e successivi) rifiutano i chip PL2303 vecchi del cavo
//! (Gestione dispositivi: codice 10, "THIS IS NOT PROLIFIC PL2303"). Serve il 3.3.2.102 (2008).

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::sistema::{esegui_come_admin, powershell, ps_str};

// ------------------------------------------------------------------ configurazione

/// Indirizzo da cui scaricare il pacchetto del driver: un .zip che contiene ser2pl.inf,
/// oppure un installer .exe (per esempio "PL2303 Code 10 Fix", che estrae il driver in C:\PLx64).
/// Vuoto = download disattivato: resta disponibile "Installa da un file".
pub const DRIVER_URL: &str = "";

/// SHA-256 atteso del file scaricato (64 caratteri esadecimali). Vuoto = nessun controllo.
/// Consigliato: il driver non viene da una fonte ufficiale.
pub const DRIVER_SHA256: &str = "";

/// Versione del driver che funziona con il cavo.
pub const VERSIONE_OK: &str = "3.3.2.102";

/// Cartella in cui l'installer "PL2303 Code 10 Fix" estrae il driver.
const CARTELLA_PLX64: &str = r"C:\PLx64";

// ------------------------------------------------------------------ stato

#[derive(Serialize)]
pub struct Dispositivo {
    nome: String,
    porta: Option<String>,
    versione: String,
    codice: i64,
    problema: Option<String>,
}

#[derive(Serialize)]
pub struct Pacchetto {
    inf: String,
    versione: String,
}

#[derive(Serialize)]
pub struct Stato {
    /// "ok", "da_sistemare" oppure "non_collegato"
    esito: &'static str,
    titolo: String,
    messaggio: String,
    dispositivi: Vec<Dispositivo>,
    archivio: Vec<Pacchetto>,
    ok_in_archivio: bool,
    url_configurato: bool,
}

const SCRIPT_STATO: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$drv = @{}
Get-CimInstance Win32_PnPSignedDriver | Where-Object { $_.DeviceID -like 'USB\VID_067B*' } |
    ForEach-Object { $drv[$_.DeviceID] = [string]$_.DriverVersion }
$dev = @(Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPDeviceID -like 'USB\VID_067B*' } | ForEach-Object {
    [pscustomobject]@{ nome = [string]$_.Name; codice = [int]$_.ConfigManagerErrorCode; versione = [string]$drv[$_.PNPDeviceID] }
})
$arc = @(((pnputil /enum-drivers) -join "`n") -split "`n\s*`n" | Where-Object { $_ -match 'ser2pl' } | ForEach-Object {
    [pscustomobject]@{ inf = [regex]::Match($_, 'oem\d+\.inf').Value; versione = [regex]::Match($_, '\d+\.\d+\.\d+\.\d+').Value }
})
ConvertTo-Json -Compress -Depth 3 -InputObject ([pscustomobject]@{ dispositivi = $dev; archivio = $arc })
"#;

/// ConvertTo-Json di PowerShell 5.1 a volte trasforma un array di un elemento in un oggetto.
fn lista(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => vec![],
        altro => vec![altro],
    }
}

fn testo(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn descrivi_codice(codice: i64) -> Option<String> {
    match codice {
        0 => None,
        10 => Some("Il dispositivo non puo' essere avviato (codice 10): tipico del driver Prolific recente che rifiuta i chip vecchi.".into()),
        28 => Some("Nessun driver installato per il dispositivo (codice 28).".into()),
        22 => Some("Il dispositivo e' disattivato in Gestione dispositivi (codice 22).".into()),
        c => Some(format!("Gestione dispositivi segnala un problema (codice {c}).")),
    }
}

fn porta_dal_nome(nome: &str) -> Option<String> {
    let i = nome.rfind("(COM")?;
    let f = nome[i..].find(')')?;
    Some(nome[i + 1..i + f].to_string())
}

pub fn stato() -> Result<Stato, String> {
    let out = powershell(SCRIPT_STATO)?;
    let json: Value = serde_json::from_str(out.trim())
        .map_err(|e| format!("Risposta inattesa dal controllo del driver: {e}\n{}", out.trim()))?;

    let dispositivi: Vec<Dispositivo> = lista(&json["dispositivi"])
        .into_iter()
        .map(|d| {
            let nome = testo(d, "nome");
            let codice = d.get("codice").and_then(Value::as_i64).unwrap_or(0);
            Dispositivo { porta: porta_dal_nome(&nome), versione: testo(d, "versione"), problema: descrivi_codice(codice), nome, codice }
        })
        .collect();
    let archivio: Vec<Pacchetto> = lista(&json["archivio"])
        .into_iter()
        .map(|p| Pacchetto { inf: testo(p, "inf"), versione: testo(p, "versione") })
        .collect();
    let ok_in_archivio = archivio.iter().any(|p| p.versione == VERSIONE_OK);

    let (esito, titolo, messaggio) = if dispositivi.is_empty() {
        (
            "non_collegato",
            "Cavo non collegato".to_string(),
            if ok_in_archivio {
                format!("Il driver {VERSIONE_OK} e' installato. Collega il cavo e premi \"Ricontrolla\" per verificare che venga usato.")
            } else {
                format!("Il driver {VERSIONE_OK} non risulta installato. Collega il cavo e premi \"Ricontrolla\", oppure installalo subito.")
            },
        )
    } else if dispositivi.iter().any(|d| d.codice == 0 && d.versione == VERSIONE_OK) {
        ("ok", "Driver corretto".to_string(), format!("Il cavo usa il driver {VERSIONE_OK}: puoi scaricare i dati dall'orologio."))
    } else {
        let d = &dispositivi[0];
        let dettaglio = d.problema.clone().unwrap_or_else(|| {
            if d.versione.is_empty() {
                "Versione del driver sconosciuta.".into()
            } else {
                format!("Il cavo usa il driver {}.", d.versione)
            }
        });
        ("da_sistemare", "Driver da sostituire".to_string(), format!("{dettaglio} Serve il driver {VERSIONE_OK}."))
    };

    Ok(Stato { esito, titolo, messaggio, dispositivi, archivio, ok_in_archivio, url_configurato: !DRIVER_URL.is_empty() })
}

// ------------------------------------------------------------------ installazione

const SCRIPT_INSTALLA: &str = r#"
$ErrorActionPreference = 'Stop'
$inf = @@INF@@
$log = @@LOG@@
function Log([string]$m) { Add-Content -LiteralPath $log -Value $m -Encoding UTF8 }
function Esegui([string[]]$a) { & pnputil @a | Where-Object { $_.Trim() } | ForEach-Object { Log "  $_" } }
try {
    $blocchi = ((pnputil /enum-drivers) -join "`n") -split "`n\s*`n"
    foreach ($b in $blocchi) {
        if ($b -match 'ser2pl' -and $b -notmatch '3\.3\.2\.102') {
            $oem = [regex]::Match($b, 'oem\d+\.inf').Value
            if ($oem) {
                Log "Rimuovo il driver Prolific recente: $oem"
                Esegui @('/delete-driver', $oem, '/uninstall', '/force')
            }
        }
    }
    Log "Installo il driver: $inf"
    Esegui @('/add-driver', $inf, '/install')
    pnputil /scan-devices | Out-Null
    Log "FINE"
} catch {
    Log "ERRORE: $_"
}
"#;

fn cartella_lavoro() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("mares-apneist-driver");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Impossibile creare {}: {e}", dir.display()))?;
    Ok(dir)
}

fn sha256(percorso: &Path) -> Result<String, String> {
    let mut f = File::open(percorso).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn scarica(dest: &Path, log: &dyn Fn(String)) -> Result<(), String> {
    log(format!("Scarico il driver da {DRIVER_URL}"));
    let resp = ureq::get(DRIVER_URL).call().map_err(|e| format!("Download non riuscito: {e}"))?;
    let mut reader = resp.into_reader();
    let mut file = File::create(dest).map_err(|e| e.to_string())?;
    let n = std::io::copy(&mut reader, &mut file).map_err(|e| format!("Download interrotto: {e}"))?;
    file.flush().map_err(|e| e.to_string())?;
    log(format!("Scaricati {} KB", n / 1024));
    Ok(())
}

fn trova_inf(dir: &Path) -> Option<PathBuf> {
    fn visita(dir: &Path, trovati: &mut Vec<PathBuf>) {
        let Ok(voci) = fs::read_dir(dir) else { return };
        for v in voci.flatten() {
            let p = v.path();
            if p.is_dir() {
                visita(&p, trovati);
            } else {
                let nome = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                if nome.starts_with("ser2pl") && nome.ends_with(".inf") {
                    trovati.push(p);
                }
            }
        }
    }
    let mut trovati = Vec::new();
    visita(dir, &mut trovati);
    // Con piu' versioni (32/64 bit) preferisce quella a 64 bit.
    trovati.sort_by_key(|p| !p.to_string_lossy().contains("64"));
    trovati.into_iter().next()
}

enum Tipo {
    Zip,
    Exe,
    Inf,
}

fn tipo_file(percorso: &Path) -> Result<Tipo, String> {
    let mut testa = [0u8; 2];
    File::open(percorso).and_then(|mut f| f.read_exact(&mut testa)).map_err(|e| format!("File illeggibile: {e}"))?;
    let est = percorso.extension().unwrap_or_default().to_string_lossy().to_lowercase();
    match (&testa, est.as_str()) {
        (b"PK", _) => Ok(Tipo::Zip),
        (b"MZ", _) => Ok(Tipo::Exe),
        (_, "inf") => Ok(Tipo::Inf),
        _ => Err("Formato non riconosciuto: serve un file .zip, .exe oppure ser2pl.inf.".into()),
    }
}

fn installa_inf(inf: &Path, lavoro: &Path, log: &dyn Fn(String)) -> Result<(), String> {
    let script = lavoro.join("installa.ps1");
    let file_log = lavoro.join("installa.log");
    let testo = SCRIPT_INSTALLA
        .replace("@@INF@@", &ps_str(&inf.to_string_lossy()))
        .replace("@@LOG@@", &ps_str(&file_log.to_string_lossy()));
    // BOM UTF-8: PowerShell 5.1 legge gli script senza BOM come ANSI.
    fs::write(&script, [&[0xEF, 0xBB, 0xBF][..], testo.as_bytes()].concat()).map_err(|e| e.to_string())?;

    log("Installo il driver: Windows chiedera' i permessi di amministratore.".into());
    esegui_come_admin(
        "powershell.exe",
        &format!("-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"", script.display()),
        true,
    )?;

    let esito = fs::read_to_string(&file_log).unwrap_or_default();
    let esito = esito.trim_start_matches('\u{feff}');
    for riga in esito.lines() {
        log(riga.to_string());
    }
    if esito.contains("ERRORE") || !esito.contains("FINE") {
        return Err("L'installazione non e' andata a buon fine: vedi i messaggi qui sopra.".into());
    }
    log("Fatto. Scollega e ricollega il cavo, poi premi \"Ricontrolla\".".into());
    Ok(())
}

fn installa_pacchetto(percorso: &Path, lavoro: &Path, log: &dyn Fn(String)) -> Result<(), String> {
    log(format!("SHA-256: {}", sha256(percorso)?));
    match tipo_file(percorso)? {
        Tipo::Inf => installa_inf(percorso, lavoro, log),
        Tipo::Zip => {
            let dest = lavoro.join("estratto");
            let f = File::open(percorso).map_err(|e| e.to_string())?;
            zip::ZipArchive::new(f)
                .and_then(|mut z| z.extract(&dest))
                .map_err(|e| format!("Archivio .zip non valido: {e}"))?;
            let inf = trova_inf(&dest).ok_or("Nell'archivio non c'e' nessun file ser2pl.inf.")?;
            installa_inf(&inf, lavoro, log)
        }
        Tipo::Exe => {
            log("Avvio l'installer: Windows chiedera' i permessi di amministratore.".into());
            esegui_come_admin(&percorso.to_string_lossy(), "", false)?;
            match trova_inf(Path::new(CARTELLA_PLX64)) {
                Some(inf) => {
                    log(format!("L'installer ha estratto il driver in {}", inf.display()));
                    installa_inf(&inf, lavoro, log)
                }
                None => {
                    log("L'installer ha terminato. Scollega e ricollega il cavo, poi premi \"Ricontrolla\".".into());
                    Ok(())
                }
            }
        }
    }
}

pub fn installa_da_url(log: &dyn Fn(String)) -> Result<(), String> {
    if DRIVER_URL.is_empty() {
        return Err("Indirizzo di download non configurato (DRIVER_URL in driver.rs). Usa \"Installa da un file\".".into());
    }
    let lavoro = cartella_lavoro()?;
    let nome: String = DRIVER_URL
        .split(['?', '#'])
        .next()
        .and_then(|u| u.rsplit('/').next())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c))
        .collect();
    let dest = lavoro.join(if nome.is_empty() { "driver.bin".to_string() } else { nome });
    scarica(&dest, log)?;
    if !DRIVER_SHA256.is_empty() {
        let h = sha256(&dest)?;
        if !h.eq_ignore_ascii_case(DRIVER_SHA256) {
            let _ = fs::remove_file(&dest);
            return Err(format!("Il file scaricato non corrisponde a quello atteso (SHA-256 {h}). Installazione annullata."));
        }
        log("Controllo SHA-256 superato.".into());
    }
    installa_pacchetto(&dest, &lavoro, log)
}

pub fn installa_da_file(percorso: &Path, log: &dyn Fn(String)) -> Result<(), String> {
    let lavoro = cartella_lavoro()?;
    log(format!("File scelto: {}", percorso.display()));
    installa_pacchetto(percorso, &lavoro, log)
}
