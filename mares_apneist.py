#!/usr/bin/env python3
"""
Lettore per il vecchio orologio da apnea Mares Apneist (anni '90) tramite cavo USB (PL2303).

Protocollo: quello dei Mares Darwin / M1 descritto in libdivecomputer
(src/mares_common.c, src/mares_darwin.c, licenza LGPL): il PC invia comandi di
LETTURA, l'orologio risponde. Nessun comando di scrittura viene mai inviato.

Il formato della memoria e' stato ricavato analizzando un dump reale
(vedi PROTOCOLLO.md): alcuni campi sono interpretazioni da confermare.

Uso:
    python mares_apneist.py porte                 # elenca le porte seriali
    python mares_apneist.py leggi [--porta COM3]  # scarica la memoria e la analizza
    python mares_apneist.py analizza FILE.bin     # analizza un dump salvato
"""

import argparse
import csv
import datetime as dt
import os
import sys
import time

# ---------------------------------------------------------------- costanti

MEMORYSIZE = 0x2000      # 8 KB (oltre questo indirizzo la memoria si ripete)
PACKETSIZE = 0x20        # byte per richiesta di lettura (massimo del protocollo)

LOG_BEGIN, LOG_END, LOG_SIZE = 0x0100, 0x0780, 16   # logbook: record da 16 byte
PROF_BEGIN, PROF_END = 0x0801, 0x1FFF                # profili: buffer circolare (fine esclusa)
SAMPLE_INTERVAL = 2      # secondi tra due campioni del profilo
DEPTH_UNIT = 1.0         # metri per unita' di profondita' (DA VERIFICARE)
MIN_DURATA = 3           # i "tuffi" piu' brevi (s) sono attivazioni accidentali dei contatti

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
DUMP_DIR = os.path.join(BASE_DIR, "dump")
EXPORT_DIR = os.path.join(BASE_DIR, "export")


def bcd(b):
    return (b >> 4) * 10 + (b & 0x0F)


# ---------------------------------------------------------------- seriale

def elenca_porte():
    from serial.tools import list_ports
    return list(list_ports.comports())


def trova_porta():
    """Cerca automaticamente l'adattatore PL2303 (VID 067B)."""
    for p in elenca_porte():
        if p.vid == 0x067B:
            return p.device
    return None


def comando_lettura(indirizzo, n):
    """'<' + HEX(51 ind_lo ind_hi n) + HEX(checksum) + '>' (checksum = somma dei caratteri ASCII)."""
    corpo = bytes([0x51, indirizzo & 0xFF, indirizzo >> 8, n]).hex().upper().encode()
    return b"<" + corpo + b"%02X" % (sum(corpo) & 0xFF) + b">"


def risposta_valida(r, n):
    if len(r) != 2 * (n + 2) or r[:1] != b"<" or r[-1:] != b">":
        return False
    try:
        return (sum(r[1:-3]) & 0xFF) == int(r[-3:-1], 16)
    except ValueError:
        return False


def scarica(porta):
    import serial

    print(f"Apro {porta} a 9600 baud...")
    try:
        ser = serial.Serial(porta, 9600, bytesize=8, parity="N", stopbits=1, timeout=1.0)
    except serial.SerialException as e:
        print(f"\nERRORE: impossibile aprire {porta}: {e}")
        print("Se in Gestione dispositivi compare 'THIS IS NOT PROLIFIC PL2303',")
        print("installa il driver Prolific 3.3.2.102 (vedi README.md).")
        sys.exit(1)

    with ser:
        # DTR e RTS alimentano l'interfaccia del cavo.
        ser.dtr = True
        ser.rts = True
        time.sleep(0.1)
        ser.reset_input_buffer()

        # L'orologio vede il PC solo a porta aperta (DTR/RTS alimentano le clip):
        # mostra "PC" e risponde dopo circa 20 secondi.
        print("In attesa dell'orologio (fino a 90 s)...", end="", flush=True)
        inizio = time.time()
        while True:
            cmd = comando_lettura(0, PACKETSIZE)
            ser.write(cmd)
            ser.read(len(cmd))
            if risposta_valida(ser.read(2 * (PACKETSIZE + 2)), PACKETSIZE):
                print(" pronto.")
                break
            if time.time() - inizio > 90:
                print("\n\nL'orologio non risponde: appoggialo sulla basetta, deve comparire 'PC'.")
                print("Se l'orologio si spegne o si riavvia, la pila e' probabilmente scarica.")
                sys.exit(2)
            time.sleep(0.5)
            ser.reset_input_buffer()

        mem = bytearray()
        for ind in range(0, MEMORYSIZE, PACKETSIZE):
            cmd = comando_lettura(ind, PACKETSIZE)
            for tentativo in range(5):
                time.sleep(0.05)
                ser.write(cmd)
                ser.read(len(cmd))                      # eco del comando (cavo a filo unico)
                r = ser.read(2 * (PACKETSIZE + 2))
                if risposta_valida(r, PACKETSIZE):
                    break
                time.sleep(0.1)
                ser.reset_input_buffer()
            else:
                print("\n\nNessuna risposta valida dall'orologio.")
                print("Controlla che sia appoggiato sulla basetta e che sul display compaia 'PC'.")
                print("Se l'orologio si spegne o si riavvia, la pila e' probabilmente scarica.")
                sys.exit(2)
            mem += bytes.fromhex(r[1:-3].decode())
            print(f"\rDownload: {len(mem) * 100 // MEMORYSIZE:3d}%", end="", flush=True)
        print()
    return bytes(mem)


# ---------------------------------------------------------------- analisi

def estrai_tuffi(data):
    """Restituisce i tuffi in ordine cronologico.

    Record del logbook (16 byte):
      0 giorno (BCD)   1 mese? (BCD)   2 minuti (BCD)   3 ore (BCD)
      4 n. tuffo nella sessione        5 allarme profondita' (m)   6-7 ?
      8-9 puntatore al profilo (LE)    10 secondi (BCD)  11 minuti (BCD)
      12 profondita' massima           13 temperatura (C)  14-15 ?
    Profilo: 1 byte di profondita' ogni 2 secondi.
    """
    recs = []
    for o in range(LOG_BEGIN, LOG_END, LOG_SIZE):
        r = data[o:o + LOG_SIZE]
        if r[0] == 0xFF:
            continue
        recs.append(r)
    recs.sort(key=lambda r: r[8] | (r[9] << 8))   # il puntatore al profilo cresce nel tempo

    tuffi = []
    for i, r in enumerate(recs):
        p = r[8] | (r[9] << 8)
        durata = bcd(r[10]) + 60 * bcd(r[11])
        # floor(durata/2) + 1 campioni, buffer circolare 0x0801-0x1FFE (0x0800 e 0x1FFF = FF di confine)
        n = durata // SAMPLE_INTERVAL + 1
        campioni = [data[a if a < PROF_END else PROF_BEGIN + a - PROF_END] for a in range(p, p + n)]
        tuffi.append({
            "giorno": f"{bcd(r[0]):02d}/{bcd(r[1]):02d}",
            "ora": f"{bcd(r[3]):02d}:{bcd(r[2]):02d}",
            "n": r[4],
            "durata": durata,
            "prof_max": r[12] * DEPTH_UNIT,
            "temp": r[13],
            "profilo": [(k * SAMPLE_INTERVAL + 1, v * DEPTH_UNIT) for k, v in enumerate(campioni)],
            "valido": durata >= MIN_DURATA,
        })
    return tuffi


def mmss(s):
    return f"{s // 60}:{s % 60:02d}"


def analizza(data, nome_base):
    data = data[:MEMORYSIZE]
    if len(data) != MEMORYSIZE:
        print(f"Attenzione: dimensione dump {len(data)} byte (attesi {MEMORYSIZE}).")
    print(f"\nIdentificativo orologio: {data[:4].hex(' ').upper()}")
    # 0x80-0x83: tuffo record (il piu' profondo di sempre): durata s/min BCD, profondita', temperatura
    print(f"Tuffo record (di sempre):      {data[0x82] * DEPTH_UNIT:.0f} m, "
          f"{bcd(data[0x81])}:{bcd(data[0x80]):02d}, {data[0x83]} C")
    print(f"Allarme di profondita':        {data[0x88] * DEPTH_UNIT:.0f} m")

    tutti = estrai_tuffi(data)
    tuffi = [t for t in tutti if t["valido"]]
    scartati = len(tutti) - len(tuffi)

    # Sessioni: tuffi consecutivi con lo stesso giorno
    sessioni = []
    for t in tuffi:
        if not sessioni or sessioni[-1]["giorno"] != t["giorno"]:
            sessioni.append({"giorno": t["giorno"], "tuffi": []})
        sessioni[-1]["tuffi"].append(t)

    if not sessioni:
        print("Nessun tuffo in memoria.")
        return

    print(f"\n{len(tuffi)} tuffi in {len(sessioni)} sessioni"
          + (f" ({scartati} attivazioni brevissime scartate)" if scartati else "") + ":\n")
    print(f"{'#':>3}  {'Giorno':6}  {'Inizio':5}  {'Tuffi':>5}  {'Prof.max':>8}  {'Tuffo piu lungo':>15}  {'Temp':>5}")
    for i, s in enumerate(sessioni, 1):
        tt = s["tuffi"]
        s["prof_max"] = max(t["prof_max"] for t in tt)
        s["durata_max"] = max(t["durata"] for t in tt)
        s["temp_min"] = min(t["temp"] for t in tt)
        print(f"{i:>3}  {s['giorno']:6}  {tt[0]['ora']:5}  {len(tt):>5}  {s['prof_max']:>6.1f} m"
              f"  {mmss(s['durata_max']):>15}  {s['temp_min']:>3} C")

    os.makedirs(EXPORT_DIR, exist_ok=True)
    f_sess = os.path.join(EXPORT_DIR, f"{nome_base}_sessioni.csv")
    f_tuffi = os.path.join(EXPORT_DIR, f"{nome_base}_tuffi.csv")
    f_prof = os.path.join(EXPORT_DIR, f"{nome_base}_profili.csv")

    with open(f_sess, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, delimiter=";")
        w.writerow(["sessione", "giorno", "inizio", "n_tuffi", "prof_max_m", "durata_max_s", "temp_min_C"])
        for i, s in enumerate(sessioni, 1):
            w.writerow([i, s["giorno"], s["tuffi"][0]["ora"], len(s["tuffi"]),
                        s["prof_max"], s["durata_max"], s["temp_min"]])

    with open(f_tuffi, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, delimiter=";")
        w.writerow(["sessione", "giorno", "ora", "tuffo", "prof_max_m", "durata_s", "temp_C"])
        for i, s in enumerate(sessioni, 1):
            for t in s["tuffi"]:
                w.writerow([i, t["giorno"], t["ora"], t["n"], t["prof_max"], t["durata"], t["temp"]])

    with open(f_prof, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, delimiter=";")
        w.writerow(["sessione", "tuffo", "tempo_s", "profondita_m"])
        for i, s in enumerate(sessioni, 1):
            for t in s["tuffi"]:
                for sec, d in t["profilo"]:
                    w.writerow([i, t["n"], sec, d])

    print(f"\nFile esportati (separatore ';', apribili con Excel):\n  {f_sess}\n  {f_tuffi}\n  {f_prof}")
    print("\nNota: unita' di profondita' e significato della data sono ancora da confermare (vedi README).")


# ---------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description="Lettore Mares Apneist (protocollo Darwin/M1)")
    sub = ap.add_subparsers(dest="cmd")
    sub.add_parser("porte", help="elenca le porte seriali")
    p_l = sub.add_parser("leggi", help="scarica la memoria dall'orologio")
    p_l.add_argument("--porta", help="es. COM3 (default: rilevamento automatico)")
    p_a = sub.add_parser("analizza", help="analizza un dump .bin salvato")
    p_a.add_argument("file")
    args = ap.parse_args()

    if args.cmd == "porte":
        for p in elenca_porte():
            vid = f"{p.vid:04X}:{p.pid:04X}" if p.vid else "-"
            print(f"{p.device:8} {vid:10} {p.description}")
    elif args.cmd == "leggi":
        porta = args.porta or trova_porta()
        if not porta:
            print("Cavo PL2303 non trovato. Usa --porta COMx (vedi 'python mares_apneist.py porte').")
            sys.exit(1)
        print("Ora appoggia l'orologio sulla basetta e tienilo fermo: compare 'PC' e dopo 20-30 s parte il download.")
        data = scarica(porta)
        os.makedirs(DUMP_DIR, exist_ok=True)
        nome = dt.datetime.now().strftime("apneist_%Y%m%d_%H%M%S")
        f_bin = os.path.join(DUMP_DIR, nome + ".bin")
        with open(f_bin, "wb") as f:
            f.write(data)
        print(f"Memoria salvata in {f_bin}")
        analizza(data, nome)
    elif args.cmd == "analizza":
        with open(args.file, "rb") as f:
            data = f.read()
        analizza(data, os.path.splitext(os.path.basename(args.file))[0])
    else:
        ap.print_help()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nAnnullato.")
