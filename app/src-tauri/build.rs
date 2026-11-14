use std::path::Path;

fn main() {
    // L'interfaccia e' visualizzatore.html (alla radice del repository), lo stesso file usato
    // nel browser: viene copiato in app/ui/index.html, la cartella servita da Tauri.
    let sorgente = Path::new("../../visualizzatore.html");
    let cartella = Path::new("../ui");
    println!("cargo:rerun-if-changed={}", sorgente.display());
    std::fs::create_dir_all(cartella).expect("impossibile creare app/ui");
    std::fs::copy(sorgente, cartella.join("index.html")).expect("impossibile copiare visualizzatore.html");

    tauri_build::build()
}
