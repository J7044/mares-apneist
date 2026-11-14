# App Windows (Tauri)

Versione "tutto in uno" del lettore: un solo `.exe` che scarica i dati dall'orologio, li mostra
con il visualizzatore e controlla o installa il driver del cavo. Non serve Python.

## Struttura

| Percorso | Contenuto |
|---|---|
| `../visualizzatore.html` | L'interfaccia. È lo stesso file usato nel browser: i pulsanti dell'app compaiono solo quando c'è `window.__TAURI__` |
| `src-tauri/build.rs` | A ogni compilazione copia il visualizzatore in `ui/index.html` (la cartella `ui/` resta nel repository vuota, perché la CLI di Tauri ne controlla l'esistenza prima di compilare) |
| `src-tauri/src/orologio.rs` | Protocollo Darwin/M1: trova il cavo (VID `067B`) e legge gli 8 KB, solo comandi di lettura |
| `src-tauri/src/driver.rs` | Controllo del driver, download e installazione del 3.3.2.102 |
| `src-tauri/src/sistema.rs` | Esecuzione di PowerShell, anche con i permessi di amministratore (UAC) |
| `src-tauri/src/main.rs` | Comandi chiamati dall'interfaccia e salvataggio dei file |

La decodifica della memoria è solo in JavaScript, nel visualizzatore. Rust si occupa della porta
seriale e del driver.

## Dove vengono salvati i dati

`Documenti\Mares Apneist\`:

- `dump\apneist_AAAAMMGG_HHMMSS.bin`: copia grezza della memoria, salvata a ogni download.
  Il pulsante **Letture salvate** le elenca dalla più recente e le riapre con un clic;
- `export\..._tuffi.csv`: CSV esportati con il pulsante **Esporta CSV**.

## Sezione driver

All'avvio l'app controlla il cavo. Se il driver va sostituito, compare un avviso. Il pulsante
**Driver del cavo** mostra:

- il dispositivo trovato, la porta COM, la versione del driver in uso e l'eventuale codice di
  errore di Gestione dispositivi (10 = driver recente che blocca il chip);
- se il driver 3.3.2.102 è presente nell'archivio driver di Windows e quali altri driver Prolific
  ci sono.

Per installare il driver:

- **Scarica e installa il driver**: scarica il pacchetto da `DRIVER_URL`. È attivo solo se
  l'indirizzo è configurato.
- **Installa da un file…**: usa un file già scaricato.

Formati accettati:

| File | Cosa fa l'app |
|---|---|
| `.zip` | Lo estrae, cerca `ser2pl.inf` (preferisce la versione a 64 bit) e lo installa |
| `.inf` | Installa direttamente quel `ser2pl.inf`. I file `.sys` e `.cat` devono stare nella stessa cartella |
| `.exe` | Avvia l'installer come amministratore. Se poi trova il driver in `C:\PLx64` (pacchetto "PL2303 Code 10 Fix"), lo installa |

L'installazione chiede i permessi di amministratore solo in quel momento. Fa gli stessi passi
di `installa_driver.ps1`: rimuove i driver Prolific diversi dal 3.3.2.102 e installa quello
scelto con `pnputil`. Alla fine scollega e ricollega il cavo e premi **Ricontrolla**.

### Configurare il download

In `src-tauri/src/driver.rs`:

```rust
pub const DRIVER_URL: &str = "https://.../driver.zip";
pub const DRIVER_SHA256: &str = "hash sha-256 del file, 64 caratteri esadecimali";
```

Imposta sempre anche `DRIVER_SHA256`: il driver non viene da una fonte ufficiale. Per calcolarlo:

```
certutil -hashfile driver.zip SHA256
```

Senza hash l'app scrive comunque l'SHA-256 del file nei messaggi di installazione.

## Compilare

Requisiti, una volta sola:

- [Rust](https://rustup.rs/) (`winget install Rustlang.Rustup`);
- Visual Studio Build Tools con "Sviluppo di applicazioni desktop con C++"
  (`winget install Microsoft.VisualStudio.2022.BuildTools`);
- [Node.js](https://nodejs.org/), solo per la riga di comando di Tauri;
- WebView2: già presente su Windows 10 e 11.

Poi, dalla cartella `app`:

```
npm install
npm run dev      # avvia l'app in modalità sviluppo
npm run build    # compila la versione finale
```

Risultato di `npm run build`:

- `src-tauri\target\release\mares-apneist.exe`: eseguibile **portatile**, basta copiarlo
  sull'altro PC;
- `src-tauri\target\release\bundle\nsis\Mares Apneist_0.1.0_x64-setup.exe`: installer, con
  collegamento nel menu Start.

Test del protocollo:

```
cd src-tauri
cargo test
```

## Note

- L'eseguibile non è firmato: al primo avvio su un altro PC Windows mostra l'avviso SmartScreen
  ("Windows ha protetto il PC"). Clicca **Ulteriori informazioni → Esegui comunque**.
- L'app usa WebView2, già incluso in Windows 10 e 11. Su sistemi molto vecchi si installa da
  Microsoft.

## Pubblicare una nuova versione

1. Aggiorna `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` e `package.json`.
2. Crea e invia il tag corrispondente:

   ```
   git tag v0.2.0
   git push origin v0.2.0
   ```

GitHub Actions (`.github/workflows/release.yml`) esegue i test, compila l'app su Windows e
pubblica la release con l'installer e l'eseguibile portatile (`Mares-Apneist-portatile.exe`).
