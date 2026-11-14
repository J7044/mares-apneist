// Copia il visualizzatore (unica fonte dell'interfaccia) nella cartella servita da Tauri.
// visualizzatore.html funziona sia aperto nel browser sia dentro l'app: le funzioni
// dell'app (download, driver) compaiono solo quando e' presente window.__TAURI__.
const fs = require("fs");
const path = require("path");

const src = path.join(__dirname, "..", "..", "visualizzatore.html");
const dir = path.join(__dirname, "..", "ui");
fs.mkdirSync(dir, { recursive: true });
fs.copyFileSync(src, path.join(dir, "index.html"));
console.log("Interfaccia copiata in app/ui/index.html");
