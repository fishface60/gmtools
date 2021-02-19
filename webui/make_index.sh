wasm-pack build --target no-modules
cat >index.html <<EOF
<html>
  <head>
  </head>
  <body>
    <script id='wasm-bindgen' type='application/javascript;base64'>$(base64 -w0 pkg/hello_wasm.js)</script>
    <script id='wasm' type='application/wasm;base64'>$(base64 -w0 pkg/hello_wasm_bg.wasm)</script>
    <script>
      let bindgen_b64 = document.getElementById('wasm-bindgen').innerText;
      let bindgen_source = atob(bindgen_b64);
      let wasm_bindgen = Function(bindgen_source + 'return wasm_bindgen;')();
      const wasm_b64 = document.getElementById('wasm').innerText;
      const wasm = Uint8Array.from(atob(wasm_b64), c => c.charCodeAt(0));
      async function run() {
        await wasm_bindgen(new Promise((resolve, reject) => {
          WebAssembly.compile(wasm).then(resolve).catch(reject);
        }), {});
      }
      run();
    </script>
  </body>
</html>
EOF
