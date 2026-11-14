# Lettore Mares Apneist

Programma per scaricare i tuffi in apnea da un vecchio orologio **Mares Apneist** (anni '90)
con il cavo USB originale (chip PL2303 e basetta a due clip). Sostituisce il software
originale, che funzionava solo su Windows XP.

Cosa ottieni:

- una **copia di backup** della memoria dell'orologio (file `.bin` da 8 KB);
- i tuffi esportati in **CSV** (apribili con Excel): sessioni, tuffi, profili profondità/tempo;
- un **visualizzatore** nel browser con grafici, tabelle e profili di ogni tuffo.

Il programma invia all'orologio **solo comandi di lettura**: non modifica né cancella nulla.

> **Attenzione al modello.** Questo orologio **non** è il più recente "Mares Nemo Apneist",
> già supportato da [Subsurface](https://subsurface-divelog.org/). L'Apneist anni '90 usa il
> protocollo dei Mares **Darwin / M1**: quando è sulla basetta mostra **PC** e aspetta che sia
> il computer a chiedere i dati.

## Cosa serve

- L'orologio Mares Apneist e il suo cavo USB con basetta (chip Prolific PL2303, ID `067B:2303`).
- Un PC **Windows** (testato su Windows 11). Linux e macOS dovrebbero funzionare per la parte
  Python, ma non sono stati provati.
- **Python 3.8 o successivo**: scaricalo da [python.org](https://www.python.org/downloads/).
  Durante l'installazione spunta **"Add python.exe to PATH"**.
- Un browser moderno (Chrome, Edge, Firefox) per il visualizzatore.

## 1. Scarica il programma

Con Git:

```
git clone https://github.com/J7044/mares-apneist.git
```

Oppure, dalla pagina GitHub, **Code → Download ZIP** ed estrai la cartella dove preferisci.

## 2. Driver del cavo (solo Windows, una volta sola)

Il cavo usa un chip PL2303 vecchio, che il driver Prolific recente (3.9.x e successivi)
blocca. In **Gestione dispositivi** la porta compare con un triangolo giallo e il messaggio
*"THIS IS NOT PROLIFIC PL2303. PLEASE CONTACT YOUR SUPPLIER"* (oppure "Codice 10").

Serve il driver vecchio **3.3.2.102** (del 2008).

**Procedura manuale**

1. Procurati il driver Prolific PL2303 **3.3.2.102**. Prolific non lo distribuisce più:
   scaricalo da una fonte affidabile e controllalo con l'antivirus.
2. Collega il cavo e apri **Gestione dispositivi** → **Porte (COM e LPT)**.
3. Clic destro sulla porta Prolific → **Aggiorna driver** → **Cerca i driver nel computer** →
   **Scegli da un elenco di driver disponibili**.
4. Seleziona la versione **3.3.2.102** (2008).
5. Scollega e ricollega il cavo. Il triangolo giallo deve sparire.

**Procedura automatica**

Se hai usato il pacchetto "PL2303 Code 10 Fix" e il driver è stato estratto in `C:\PLx64`,
fai doppio clic su **`installa_driver.bat`**: chiede i permessi di amministratore, rimuove i
driver Prolific recenti e installa il 3.3.2.102.

Windows Update a volte reinstalla il driver nuovo. Se il cavo smette di funzionare, ripeti la
procedura.

## 3. Installa la dipendenza Python

Apri un prompt dei comandi nella cartella del programma ed esegui:

```
pip install -r requirements.txt
```

(Installa solo [pyserial](https://pypi.org/project/pyserial/).) Se usi `leggi_orologio.bat`
questo passaggio viene fatto in automatico.

## 4. Scarica i dati dall'orologio

1. Collega il cavo al PC.
2. **Appoggia l'orologio sulla basetta**, con le due clip ben a contatto: sul display compare
   **PC** e i tasti smettono di rispondere. È normale.
3. Fai doppio clic su **`leggi_orologio.bat`**.

Il download dura circa un minuto. Alla fine il programma mostra l'elenco dei tuffi e salva i file.

Da riga di comando sono disponibili anche questi comandi:

```
python mares_apneist.py porte                   # elenca le porte seriali (il cavo ha ID 067B:2303)
python mares_apneist.py leggi                   # trova il cavo da solo e scarica la memoria
python mares_apneist.py leggi --porta COM3      # indica tu la porta, se ce n'è più di una
python mares_apneist.py analizza dump\FILE.bin  # rianalizza un dump già salvato, senza orologio
```

## 5. I file prodotti

| File | Contenuto |
|---|---|
| `dump\apneist_AAAAMMGG_HHMMSS.bin` | Copia grezza della memoria (8 KB). **Conservala**: è il tuo backup e si può rianalizzare in ogni momento. |
| `export\..._sessioni.csv` | Una riga per sessione (giornata). |
| `export\..._tuffi.csv` | Una riga per tuffo: ora, profondità massima, durata, temperatura. |
| `export\..._profili.csv` | Profilo profondità/tempo di ogni tuffo, un campione ogni 2 secondi. |

I CSV usano il `;` come separatore e si aprono direttamente con Excel in italiano.

Le cartelle `dump\` ed `export\` sono escluse da Git (vedi `.gitignore`), così i tuoi dati
non finiscono per errore in un repository pubblico.

## 6. Il visualizzatore

Apri **`visualizzatore.html`** con il browser (doppio clic) e trascinaci sopra il file `.bin`
della cartella `dump\`. La pagina decodifica la memoria direttamente nel browser, **funziona
offline e non invia dati da nessuna parte**.

Mostra:

- un riepilogo: numero di tuffi, il più profondo, il più lungo, il tempo totale in apnea;
- una vista per sessione singola e una con tutte le sessioni;
- il profilo profondità/tempo di ogni tuffo (frecce ← → per scorrere i tuffi);
- la tabella completa e ordinabile, con i dati grezzi di ogni record;
- la massima di sempre e l'allarme di profondità registrati nell'orologio;
- il contenuto esadecimale completo della memoria.

Il pulsante **Esporta CSV** salva tuffi e profili. L'ultimo file aperto viene ricordato dal
browser (solo sul tuo computer).

## Problemi comuni

| Sintomo | Causa probabile e soluzione |
|---|---|
| "Nessuna porta trovata" o errore all'apertura della porta | Driver del cavo: vedi il punto 2. Controlla in Gestione dispositivi che la porta non abbia il triangolo giallo. |
| Il programma non riceve risposta dall'orologio | Verifica che sul display compaia **PC**. Pulisci le clip e i contatti dell'orologio e riappoggialo. Chiudi altri programmi che usano la stessa porta COM. |
| L'orologio emette suoni, si spegne o si riavvia | Pila scarica: sostituiscila prima di riprovare. |
| `python` non è riconosciuto come comando | Python non è nel PATH: reinstallalo spuntando "Add python.exe to PATH". |
| Errori di checksum ripetuti | Contatto instabile sulla basetta: tieni fermo l'orologio durante il download e riprova. |

## Limiti noti

- Il formato della memoria è stato ricavato analizzando la memoria di **un solo esemplare**.
  Alcuni campi sono interpretazioni: in particolare il **mese** della data non è ancora
  stato decodificato, quindi le date dei tuffi possono essere incomplete.
- L'orologio conserva solo gli ultimi ~100 tuffi circa: i più vecchi vengono sovrascritti.
- I "tuffi" più brevi di 3 secondi vengono scartati: sono attivazioni accidentali dei contatti.

I dettagli del protocollo e del formato della memoria sono in [PROTOCOLLO.md](PROTOCOLLO.md).

## Contribuire

Se hai un Mares Apneist e i risultati non ti tornano (date, profondità, temperature), apri
una segnalazione su GitHub indicando cosa ti aspettavi e cosa vedi. Non allegare il file `.bin`
se non vuoi rendere pubblici i tuoi tuffi: bastano i valori del visualizzatore (scheda dati
grezzi) dei record che non tornano.

## Licenza e crediti

Il protocollo di comunicazione deriva da
[libdivecomputer](https://github.com/libdivecomputer/libdivecomputer) (LGPL 2.1),
file `mares_common.c` e `mares_darwin.c`.
