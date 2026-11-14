// Niente finestra della console nella versione compilata.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod driver;
mod orologio;
mod sistema;

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Documenti\Mares Apneist: qui finiscono i dump (.bin) e i CSV esportati.
fn cartella_dati(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().document_dir().map_err(|e| format!("Cartella Documenti non trovata: {e}"))?;
    Ok(base.join("Mares Apneist"))
}

fn crea(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Impossibile creare {}: {e}", dir.display()))
}

/// Esegue un lavoro lungo fuori dal thread dell'interfaccia.
async fn in_background<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct Dump {
    nome: String,
    percorso: String,
    dati: Vec<u8>,
}

/// Scarica la memoria dall'orologio e la salva in Documenti\Mares Apneist\dump.
/// Durante il download emette l'evento "progresso" (0-100).
#[tauri::command]
async fn scarica_orologio(app: AppHandle) -> Result<Dump, String> {
    let dir = cartella_dati(&app)?.join("dump");
    in_background(move || {
        let porta = orologio::trova_porta()?;
        let _ = app.emit("porta", &porta);
        let dati = orologio::scarica(
            &porta,
            |secondi| {
                let _ = app.emit("attesa", secondi);
            },
            |pct| {
                let _ = app.emit("progresso", pct);
            },
        )?;
        crea(&dir)?;
        let nome = format!("apneist_{}.bin", chrono::Local::now().format("%Y%m%d_%H%M%S"));
        let percorso = dir.join(&nome);
        std::fs::write(&percorso, &dati).map_err(|e| format!("Impossibile salvare {}: {e}", percorso.display()))?;
        Ok(Dump { nome, percorso: percorso.display().to_string(), dati })
    })
    .await
}

#[derive(Serialize)]
struct Lettura {
    nome: String,
    data: String,
    kb: u64,
}

/// Elenco delle letture salvate in Documenti\Mares Apneist\dump, dalla piu' recente.
#[tauri::command]
fn elenco_letture(app: AppHandle) -> Result<Vec<Lettura>, String> {
    let dir = cartella_dati(&app)?.join("dump");
    let Ok(voci) = std::fs::read_dir(&dir) else { return Ok(vec![]) };
    let mut letture: Vec<(std::time::SystemTime, Lettura)> = voci
        .flatten()
        .filter(|v| v.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("bin")))
        .filter_map(|v| {
            let meta = v.metadata().ok()?;
            let quando = meta.modified().ok()?;
            let data = chrono::DateTime::<chrono::Local>::from(quando).format("%d/%m/%Y %H:%M").to_string();
            Some((quando, Lettura { nome: v.file_name().to_string_lossy().into_owned(), data, kb: meta.len().div_ceil(1024) }))
        })
        .collect();
    letture.sort_by_key(|l| std::cmp::Reverse(l.0));
    Ok(letture.into_iter().map(|(_, l)| l).collect())
}

/// Riapre una lettura salvata.
#[tauri::command]
fn apri_lettura(app: AppHandle, nome: String) -> Result<Dump, String> {
    let nome = Path::new(&nome).file_name().ok_or("Nome di file non valido")?.to_owned();
    let percorso = cartella_dati(&app)?.join("dump").join(&nome);
    let dati = std::fs::read(&percorso).map_err(|e| format!("Impossibile leggere {}: {e}", percorso.display()))?;
    Ok(Dump { nome: nome.to_string_lossy().into_owned(), percorso: percorso.display().to_string(), dati })
}

/// Salva un file di testo (CSV) in Documenti\Mares Apneist\export e ne restituisce il percorso.
#[tauri::command]
fn salva_file(app: AppHandle, nome: String, contenuto: String) -> Result<String, String> {
    let nome = Path::new(&nome).file_name().ok_or("Nome di file non valido")?.to_owned();
    let dir = cartella_dati(&app)?.join("export");
    crea(&dir)?;
    let percorso = dir.join(nome);
    // BOM UTF-8: Excel riconosce cosi' le lettere accentate.
    std::fs::write(&percorso, [&[0xEF, 0xBB, 0xBF][..], contenuto.as_bytes()].concat())
        .map_err(|e| format!("Impossibile salvare {}: {e}", percorso.display()))?;
    Ok(percorso.display().to_string())
}

/// Apre la cartella dei dati in Esplora file.
#[tauri::command]
fn apri_cartella(app: AppHandle) -> Result<(), String> {
    let dir = cartella_dati(&app)?;
    crea(&dir)?;
    std::process::Command::new("explorer.exe").arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn stato_driver() -> Result<driver::Stato, String> {
    in_background(driver::stato).await
}

/// Scarica il driver dall'indirizzo configurato e lo installa. Emette "driver-log" con i messaggi.
#[tauri::command]
async fn installa_driver_da_url(app: AppHandle) -> Result<driver::Stato, String> {
    in_background(move || {
        driver::installa_da_url(&|m| {
            let _ = app.emit("driver-log", m);
        })?;
        driver::stato()
    })
    .await
}

/// Installa il driver da un file scelto dall'utente (.zip, .exe o ser2pl.inf).
#[tauri::command]
async fn installa_driver_da_file(app: AppHandle, percorso: String) -> Result<driver::Stato, String> {
    in_background(move || {
        driver::installa_da_file(Path::new(&percorso), &|m| {
            let _ = app.emit("driver-log", m);
        })?;
        driver::stato()
    })
    .await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scarica_orologio,
            salva_file,
            apri_cartella,
            elenco_letture,
            apri_lettura,
            stato_driver,
            installa_driver_da_url,
            installa_driver_da_file
        ])
        .run(tauri::generate_context!())
        .expect("errore durante l'avvio dell'applicazione");
}
