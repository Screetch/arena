#!/bin/sh
# Builds the web client, the client of arena for a browser, into target/web: its page, its WebAssembly and the glue of
# wasm-bindgen that starts it. The build holds a slot of script/build-slot.sh. With serve, the script then serves that
# folder on http://127.0.0.1:$PORT, 8000 by default, and opens no browser. With version, it prints the version of
# wasm-bindgen that Cargo.lock holds: the command wasm-bindgen must have that version, since its glue and the crate
# agree only then. With test, it runs the tests of crates/arena-client/tests in place of the build, in a headless
# browser, with wasm-bindgen-test-runner, which comes with wasm-bindgen, through the driver that CHROMEDRIVER or
# GECKODRIVER names, else the one on the PATH.
# Usage: sh script/web.sh [serve | test | version]
top=$(git rev-parse --show-toplevel) || exit 1
cd "$top" || exit 1
version=$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;}' Cargo.lock)
case ${1:-} in
  version)
    echo "$version"
    exit 0
    ;;
  '' | serve | test) ;;
  *)
    echo "usage: sh script/web.sh [serve | test | version]" >&2
    exit 2
    ;;
esac
if [ "$(wasm-bindgen --version 2>/dev/null)" != "wasm-bindgen $version" ]; then
  echo "the web client needs wasm-bindgen $version: cargo install wasm-bindgen-cli --version $version --locked" >&2
  exit 1
fi
if [ "$1" = test ]; then
  export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
  exec sh script/build-slot.sh cargo test --locked -p arena-client --target wasm32-unknown-unknown --test canvas
fi
sh script/build-slot.sh cargo build --locked --release -p arena-client --bin arena-client \
  --target wasm32-unknown-unknown || exit 1
rm -rf target/web
wasm-bindgen --target web --no-typescript --out-dir target/web \
  target/wasm32-unknown-unknown/release/arena-client.wasm || exit 1
cp crates/arena-client/web/index.html target/web/ || exit 1
echo "target/web holds the web client"
if [ "${1:-}" = serve ]; then
  exec python3 -m http.server --bind 127.0.0.1 --directory target/web "${PORT:-8000}"
fi
