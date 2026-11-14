//! Esecuzione di comandi PowerShell, anche con i permessi di amministratore.

use std::process::Command;

use base64::Engine;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Esegue uno script PowerShell senza mostrare finestre e ne restituisce l'output (UTF-8).
pub fn powershell(script: &str) -> Result<String, String> {
    let completo = format!("[Console]::OutputEncoding = [Text.Encoding]::UTF8\n{script}");
    let utf16: Vec<u8> = completo.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let codificato = base64::engine::general_purpose::STANDARD.encode(utf16);

    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &codificato]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output().map_err(|e| format!("Impossibile avviare PowerShell: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{}\n{}", stdout.trim(), stderr.trim()).trim().to_string());
    }
    Ok(stdout)
}

/// Stringa PowerShell tra apici singoli (nessuna espansione di variabili).
pub fn ps_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Avvia un programma con i permessi di amministratore (richiesta UAC) e ne attende la fine.
/// `argomenti` viene passato cosi' com'e': i percorsi vanno gia' racchiusi tra virgolette.
/// Con `nascosto` la finestra del programma non viene mostrata.
pub fn esegui_come_admin(programma: &str, argomenti: &str, nascosto: bool) -> Result<(), String> {
    let mut arg = if argomenti.is_empty() { String::new() } else { format!(" -ArgumentList {}", ps_str(argomenti)) };
    if nascosto {
        arg.push_str(" -WindowStyle Hidden");
    }
    let script = format!(
        "try {{ $p = Start-Process -FilePath {}{arg} -Verb RunAs -Wait -PassThru -ErrorAction Stop; \
         Write-Output (\"USCITA=\" + $p.ExitCode) }} \
         catch {{ Write-Output \"ANNULLATO\"; Write-Output $_.Exception.Message }}",
        ps_str(programma)
    );
    let out = powershell(&script)?;
    if out.contains("ANNULLATO") {
        return Err("Operazione annullata: i permessi di amministratore non sono stati concessi.".into());
    }
    Ok(())
}
