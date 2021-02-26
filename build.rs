#![allow(clippy::single_component_path_imports)]

use std::{env, fs, path::Path, process::Command};

use base64;
use json::{self, JsonValue};

const MOD_NAME: &str = "webui";

// build.rs cribbed from https://github.com/rustwasm/wasm-pack/issues/916#issuecomment-698173427
// then modified heavily, perhaps should contribute back the message-format parsing

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir);
    let output = Command::new("wasm-pack")
        .args(&[
            "build",
            "--target",
            "no-modules",
            "--no-typescript",
            "--out-dir",
        ])
        .arg(&dest_path)
        .arg("webui")
        .args(&["--", "--message-format=json"])
        .output()
        .expect("To build wasm files successfully");

    if !output.status.success() {
        panic!(
            "Error while compiling:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    let mut found: bool = false;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let message = if let Ok(message) = json::parse(line) {
            message
        } else {
            continue;
        };
        let object = if let JsonValue::Object(object) = message {
            object
        } else {
            continue;
        };

        if object.get("reason")
            != Some(&JsonValue::String("compiler-artifact".to_string()))
        {
            continue;
        }
        let target_entry = object.get("target");
        let target_value = if let Some(target_value) = target_entry {
            target_value
        } else {
            continue;
        };
        let target_object =
            if let JsonValue::Object(target_object) = target_value {
                target_object
            } else {
                continue;
            };
        if target_object.get("name")
            != Some(&JsonValue::String(MOD_NAME.to_string()))
        {
            continue;
        }

        if let JsonValue::String(src_path) =
            target_object.get("src_path").unwrap()
        {
            println!("cargo:rerun-if-changed={}", src_path);
            found = true;
            break;
        } else {
            panic!("src_path not String");
        }
    }

    if !found {
        panic!(
            "Did not encounter compiler artifact message for {}",
            MOD_NAME
        );
    }

    let js_path = dest_path.join(format!("{}.js", MOD_NAME));
    let wasm_path = dest_path.join(format!("{}_bg.wasm", MOD_NAME));

    for path in &[&js_path, &wasm_path] {
        let file = fs::metadata(path).expect("file to exist");
        assert!(file.is_file());
    }

    let html = format!("\
        <!DOCTYPE html>
        <html>
          <head>
            <meta charset='utf-8'/>
            <meta name='viewport' content='width=device-width, initial-scale=1'/>
          </head>
          <body>
            <script id='wasm-bindgen' type='application/javascript;base64'>{wasm_bind}</script>
            <script id='wasm' type='application/wasm;base64'>{wasm}</script>
            <script>
              let bindgen_b64 = document.getElementById('wasm-bindgen').innerText;
              let bindgen_source = atob(bindgen_b64);
              let wasm_bindgen = Function(bindgen_source + 'return wasm_bindgen;')();
              const wasm_b64 = document.getElementById('wasm').innerText;
              const wasm = Uint8Array.from(atob(wasm_b64), c => c.charCodeAt(0));
              async function run() {{
                await wasm_bindgen(new Promise((resolve, reject) => {{
                  WebAssembly.compile(wasm).then(resolve).catch(reject);
                }}), {{}});
              }}
              run();
            </script>
          </body>
        </html>
        ",
        wasm_bind = base64::encode(&fs::read(js_path).expect("Read javascript")),
        wasm = base64::encode(&fs::read(wasm_path).expect("Read wasm"))
    );
    let html_path = dest_path.join("index.html");
    fs::write(&html_path, html).unwrap();

    println!("cargo:rustc-env=WEBUI_HTML_PATH={}", html_path.display());
}
