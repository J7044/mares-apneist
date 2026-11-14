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
| `0x0080`–`0x0083` | Tuffo record di sempre: durata (secondi, minuti in BCD), profondità massima (m), temperatura (°C). È quello che l'orologio mostra come tuffo più profondo; il suo profilo di solito non è più in memoria |
| `0x0084` | ? (sull'esemplare analizzato coincide con il numero di tuffi validi del logbook) |
| `0x0088` | Allarme di profondità impostato (m) |
| `0x0100`–`0x077F` | Logbook: record da 16 byte, uno per tuffo (circolare, ~100 record) |
| `0x0800`, `0x1FFF` | `FF`: confini del buffer dei profili |
| `0x0801`–`0x1FFE` | Profili: 1 byte di profondità ogni 2 secondi, buffer circolare (dopo `0x1FFE` si riparte da `0x0801`). L'area è più grande di quanto serve ai 100 record del logbook: contiene anche i profili di tuffi più vecchi, senza data né temperatura (vedi sotto) |

### Record del logbook

| Byte | Significato |
|---|---|
| 0 | giorno (BCD) |
| 1 | mese? (BCD) – **da confermare** |
| 2, 3 | minuti, ore di inizio (BCD) |
| 4 | numero del tuffo nella sessione |
| 5 | allarme di profondità attivo durante il tuffo (m) |
| 6, 7 | sempre `00 00` sull'esemplare analizzato |
| 8–9 | indirizzo di inizio del profilo (little endian) |
| 10, 11 | durata: secondi, minuti (BCD) |
| 12 | profondità massima (m) |
| 13 | temperatura (°C) |
| 14, 15 | sempre `FF F4` sull'esemplare analizzato |

I record vengono ordinati nel tempo tramite l'indirizzo del profilo (byte 8–9), perché il
logbook è circolare (100 posizioni, `0x0100`–`0x073F`). Un byte `0xFF` in posizione 0 indica
un record vuoto.

### Profili

Un tuffo di durata *d* secondi occupa **floor(d/2) + 1** campioni consecutivi, e il tuffo
successivo inizia subito dopo: non ci sono separatori. Il primo campione vale di solito 1 o
2 m, l'ultimo 0 o 1 m.

### Tuffi senza record

Il buffer dei profili contiene alcune ore di immersione; i 100 record del logbook ne
coprono solo una parte. Il resto, dal byte dopo l'ultimo tuffo del logbook fino al primo,
contiene tuffi più vecchi ancora leggibili, ma senza data, ora e temperatura. Il primo di
questi ha l'inizio già sovrascritto e viene scartato.

Per separarli si usa la struttura vista sopra: un tuffo finisce con uno 0, oppure con un 1
seguito da un valore ≥ 2 (l'inizio del tuffo dopo); un campione isolato ≥ 3 m all'inizio è
un'attivazione accidentale. Applicata ai tuffi del logbook, di cui si conoscono i confini
veri, la regola trova l'inizio esatto in 95 casi su 100 (inizio e lunghezza esatti in 91).
Gli errori sono di 1 campione o riguardano attivazioni accidentali da 1 secondo. La regola è
implementata in `recoverOld()` nel visualizzatore.

## Come è stato verificato

Sull'esemplare analizzato:

- **durata** (byte 10–11): in tutti i record la distanza tra l'inizio di un profilo e il
  successivo è floor(durata/2) + 1 campioni, quindi un campione ogni 2 secondi;
- **profondità massima** (byte 12) = massimo del profilo nella grande maggioranza dei casi;
  le eccezioni sono differenze di 1 unità o "tuffi" di 1 secondo (attivazioni accidentali
  dei contatti, che il programma scarta);
- **ora di inizio**: valori sempre crescenti all'interno di una sessione;
- **temperatura**: valori plausibili in °C;
- **unità di profondità = 1 m**: il tuffo record (profondità, durata e temperatura) e
  l'allarme letti in memoria coincidono con quelli mostrati dall'orologio; inoltre con 1 m le
  velocità di discesa risultano tipiche dell'apnea (~0,5–1 m/s), con 0,5 m o 2 m no.

## Da confermare

- **Byte 1 della data**: sull'esemplare analizzato vale sempre `01`, e i giorni delle
  sessioni non sono in ordine crescente. Forse il mese non è memorizzato, oppure
  l'orologio è stato reimpostato tra una sessione e l'altra.
- **Byte 6, 7, 14, 15** del record: costanti sull'esemplare analizzato, significato sconosciuto.
- **"h5"**: l'orologio lo mostra insieme al tuffo record (senza ora, data né numero del
  tuffo). Nell'intestazione non c'è nessun byte che valga 5. Potrebbero essere le ore totali
  di immersione: il buffer da solo ne contiene già alcune ed è stato sovrascritto almeno una
  volta. Per capirlo: confrontare un dump prima e uno dopo qualche nuovo tuffo.
- **Byte `0x0084`–`0x008A`** (tranne `0x0088`), byte `0x0036`–`0x003A` e `0x0780`: sconosciuti.
- **Byte `0x0040`–`0x005F`**: letti come float little endian danno valori piccoli
  (es. 0,085 e 0,15): forse coefficienti di calibrazione del sensore.
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
