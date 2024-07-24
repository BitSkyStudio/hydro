cargo build --target wasm32-unknown-unknowncargo build --release --package hydro_client --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/hydro_client.wasm hydro_server/host
