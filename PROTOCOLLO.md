# Protocollo e formato della memoria – Mares Apneist (anni '90)

Note tecniche per chi vuole capire come funziona il programma, verificarlo sul proprio
orologio o aiutare a completare la decodifica.

## Hardware

| Elemento | Dettaglio |
|---|---|
| Cavo | USB-seriale con chip Prolific PL2303 (ID `067B:2303`) |
| Driver Windows | Prolific **3.3.2.102** (2008). I driver 3.9.x rifiutano i chip vecchi |
| Basetta | L'orologio poggia su 2 clip laterali, che sono anche i contatti "acqua" per l'accensione in immersione |
| Orologio | 2 tasti. Entra da solo in modalità **PC** quando è sulla basetta; in quel momento i tasti non rispondono |

## Protocollo di comunicazione

È quello della famiglia **Mares Darwin / M1 / M2**, descritto in libdivecomputer
(`mares_common.c`, `mares_darwin.c`). Diverso da quello del Nemo Apneist, in cui è
l'orologio a trasmettere da solo: qui l'orologio resta in silenzio finché il PC non chiede i dati.

- Seriale **9600 baud, 8N1**, nessun controllo di flusso, **DTR = 1, RTS = 1**
  (le due linee alimentano l'interfaccia del cavo).
- Attendere ~50 ms prima di ogni comando.
- Comando di lettura, in ASCII esadecimale maiuscolo:

  ```
  '<' + HEX(51, ind_basso, ind_alto, lunghezza) + HEX(checksum) + '>'
  ```

  Il checksum è la somma (mod 256) dei **caratteri ASCII** tra `<` e il checksum.
  Esempio: leggere 16 byte dall'indirizzo 0 → `<5100001087>`.
- Il cavo restituisce l'**eco** del comando (i contatti sono in comune): va letto e scartato.
- La risposta è `'<' + HEX(dati) + HEX(checksum) + '>'`, con lo stesso tipo di checksum.
- Lunghezza massima per richiesta: 32 byte (`0x20`).

Il programma usa solo il comando di lettura `0x51`.

## Memoria (8 KB)

La memoria reale è di **8 KB**: leggendo oltre `0x1FFF` l'indirizzo "gira" e i dati si ripetono.

| Indirizzo | Contenuto |
|---|---|
| `0x0000` | Identificativo (4 byte; probabilmente modello o numero di serie) |
| `0x0082` | Profondità massima di sempre (m) |
| `0x0088` | Allarme di profondità impostato (m) |
| `0x0100`–`0x077F` | Logbook: record da 16 byte, uno per tuffo (circolare, ~100 record) |
| `0x0800`–`0x1FFF` | Profili: 1 byte di profondità ogni 2 secondi (circolare) |

### Record del logbook

| Byte | Significato |
|---|---|
| 0 | giorno (BCD) |
| 1 | mese? (BCD) – **da confermare** |
| 2, 3 | minuti, ore di inizio (BCD) |
| 4 | numero del tuffo nella sessione |
| 5 | allarme di profondità attivo durante il tuffo (m) |
| 8–9 | indirizzo di inizio del profilo (little endian) |
| 10, 11 | durata: secondi, minuti (BCD) |
| 12 | profondità massima (m) |
| 13 | temperatura (°C) |

I record vengono ordinati nel tempo tramite l'indirizzo del profilo (byte 8–9), perché il
logbook è circolare. Un byte `0xFF` in posizione 0 indica un record vuoto.

## Come è stato verificato

Sull'esemplare analizzato:

- **durata** (byte 10–11) = 2 s × numero di campioni del profilo in tutti i record: il
  profilo ha quindi un campione ogni 2 secondi;
- **profondità massima** (byte 12) = massimo del profilo nella grande maggioranza dei casi;
  le eccezioni sono differenze di 1 unità o "tuffi" di 1 secondo (attivazioni accidentali
  dei contatti, che il programma scarta);
- **ora di inizio**: valori sempre crescenti all'interno di una sessione;
- **temperatura**: valori plausibili in °C;
- **unità di profondità = 1 m**: la massima di sempre e l'allarme letti in memoria
  coincidono con quelli ricordati dal proprietario dell'orologio; inoltre con 1 m le
  velocità di discesa risultano tipiche dell'apnea (~0,5–1 m/s), con 0,5 m o 2 m no.

## Da confermare

- **Byte 1 della data**: sull'esemplare analizzato vale sempre `01`, e i giorni delle
  sessioni non sono in ordine crescente. Forse il mese non è memorizzato, oppure
  l'orologio è stato reimpostato tra una sessione e l'altra.
- **Byte 6, 7, 14, 15** del record: significato sconosciuto.
- **Identificativo** in `0x0000`: va confrontato con altri esemplari per capire se è il
  modello o il numero di serie.

Se hai un Mares Apneist e ricordi data, profondità o durata di qualche tuffo, confronta i
valori con la scheda dei dati grezzi del visualizzatore e apri una segnalazione.

## Cose provate che non funzionano

Utili a chi incontra lo stesso orologio:

- **Protocollo Nemo Apneist** (attendere i byte `0xEE` trasmessi dall'orologio): nessun dato.
  Questo orologio non trasmette mai da solo.
- **Ascolto passivo della porta** e prove con tutte le combinazioni di DTR/RTS: nessun dato.
- **Pila scarica**: l'orologio emette suoni, si spegne e si riaccende; va sostituita prima
  di qualsiasi prova.
- **Test del cavo**: toccando le due clip con un oggetto metallico (senza orologio) il PC
  riceve byte di "rumore". Se succede, il percorso clip → cavo → PC funziona.
