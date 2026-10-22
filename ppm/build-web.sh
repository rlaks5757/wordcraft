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
# 인자 없음: release(LTO). --fast: 확인용 빠른 빌드(web-fast, 최적화는 낮음).
stamp="$(mktemp)"
trap 'rm -f "$stamp"' EXIT
case "${1:-}" in
  "") trunk build --release --features ppm-host ;;
  --fast) trunk build --cargo-profile web-fast --features ppm-host ;;
  *) echo "usage: $0 [--fast]" >&2; exit 2 ;;
esac
# ppm-host가 실제로 켜졌는지: 이번 빌드의 wasm에 잠금 화면 문자열이 없으면 실패한다(feature가 빠지면 잠금이 소리 없이 꺼진다).
host_wasm=0
while IFS= read -r wasm; do
  if grep -aq "PPM에서 열어 주세요" "$wasm"; then host_wasm=1; fi
done < <(find "$root/dist/web" -name '*_bg.wasm' -newer "$stamp")
if [ "$host_wasm" != 1 ]; then
  echo "error: 이번 빌드의 wasm에 ppm-host 잠금 문자열이 없다(feature가 적용되지 않음)" >&2
  exit 1
fi
# OFL-1.1 §2: the license travels with the embedded font.
mkdir -p "$root/dist/web/licenses"
cp "$root/ppm/fonts-ko/fonts/nanum-gothic/OFL.txt" "$root/dist/web/licenses/NanumGothic-OFL.txt"
# 관리자 화면이 읽는 명령 목록(공통 형식: app, schema, 명령마다 id, label, labelKo, location 배열, grades.builder).
# 잠금 표에서 만들어 표와 어긋나지 않는다. 임시 파일에 쓰고 성공했을 때만 옮겨서, 실패해도 빈 파일이 남지 않는다.
cd "$root"
tmp_cmds="$(mktemp)"
trap 'rm -f "$stamp" "$tmp_cmds"' EXIT
cargo run -q -p wordcraft-ui-egui --example commands_json > "$tmp_cmds"
mv "$tmp_cmds" "$root/dist/web/commands.json"
