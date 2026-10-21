#!/usr/bin/env bash
# PPM 포크 웹 빌드: 한글 글꼴(NanumGothic, OFL-1.1)을 넣어 dist/web 을 만든다.
# 이 글꼴만 들어간다(업스트림 릴리스에 있던 다른 글꼴은 빠진다).
# 필요: Rust 1.95+ (기본 1.99.1), wasm32-unknown-unknown target, trunk 0.21+
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
export CRAFT_FONTS_DIR="$root/ppm/fonts-ko"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.99.1}"
app=wordcraft-web
cd "$root/apps/$app"
# ppm-host: PPM 전용 빌드는 항상 호스트 모드(부모 PPM 창 없이는 문서를 열지 않음). 컴파일 시점 값.
trunk build --release --features ppm-host
# OFL-1.1 §2: the license travels with the embedded font.
mkdir -p "$root/dist/web/licenses"
cp "$root/ppm/fonts-ko/fonts/nanum-gothic/OFL.txt" "$root/dist/web/licenses/NanumGothic-OFL.txt"
