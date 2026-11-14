# Sostituisce il driver Prolific recente (che blocca i chip PL2303 vecchi/clone)
# con il driver 3.3.2.102 (2008) contenuto in C:\PLx64 (installato da "PL2303 Code 10 Fix").
# Va eseguito come amministratore: si riavvia da solo con i permessi necessari.

$ErrorActionPreference = 'Stop'
$inf = 'C:\PLx64\Ser2pl.inf'

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
    Start-Process powershell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

try {
    if (-not (Test-Path $inf)) { throw "File $inf non trovato: installa prima 'PL2303 Code 10 Fix'." }

    # 1. Trova e rimuove dall'archivio driver tutti i pacchetti Prolific diversi dal 3.3.2.102
    $blocchi = ((pnputil /enum-drivers) -join "`n") -split "`n\s*`n"
    foreach ($b in $blocchi) {
        if ($b -match 'ser2pl' -and $b -notmatch '3\.3\.2\.102') {
            $oem = [regex]::Match($b, 'oem\d+\.inf').Value
            if ($oem) {
                Write-Host "Rimuovo il driver Prolific recente: $oem"
                pnputil /delete-driver $oem /uninstall /force
            }
        }
    }

    # 2. Aggiunge e installa il driver 3.3.2.102
    Write-Host "`nInstallo il driver 3.3.2.102..."
    pnputil /add-driver $inf /install

    # 3. Riesamina i dispositivi
    pnputil /scan-devices | Out-Null

    Write-Host "`nStato attuale:"
    Get-CimInstance Win32_PnPSignedDriver | Where-Object { $_.DeviceID -like 'USB\VID_067B*' } |
        Select-Object DeviceName, DriverVersion | Format-List
    Write-Host "Ora SCOLLEGA e RICOLLEGA il cavo. La versione deve essere 3.3.2.102."
} catch {
    Write-Host "ERRORE: $_" -ForegroundColor Red
}
Read-Host "`nPremi Invio per chiudere"
