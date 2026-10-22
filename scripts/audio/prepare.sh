#!/usr/bin/env sh
# Run inside the devcontainer. No recording is fetched by the application itself.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
source_dir="$(mktemp -d)"
trap 'rm -rf "$source_dir"' EXIT
curl -fLsS --retry 3 -o "$source_dir/wind.mp3" https://cdn.freesound.org/previews/183/183496_2337290-hq.mp3
curl -fLsS --retry 3 -o "$source_dir/bike.mp3" https://cdn.freesound.org/previews/380/380288_6941033-hq.mp3
curl -fLsS --retry 3 -o "$source_dir/freewheel.mp3" https://cdn.freesound.org/previews/274/274481_2023784-hq.mp3
curl -fLsS --retry 3 -o "$source_dir/shift.mp3" https://cdn.freesound.org/previews/267/267910_59530-hq.mp3
curl -fLsS --retry 3 -o "$source_dir/transfer.mp3" https://cdn.freesound.org/previews/494/494322_470543-hq.mp3
cd "$source_dir"
sha256sum -c <<'CHECKSUMS'
a804e75f0c57acd77488a63b8452d8de8593a7f8ed1c0a5b546d0b41d770b8fa  wind.mp3
5ad76458161f34cb548dcb379d6f3b235991a68b1a4afef054ec2de8c6bea885  bike.mp3
38a8739ea78134a139855e57a8bfa8c0f6dcbea0948442b7fe3bf84e645ec62e  freewheel.mp3
28f86e49e45627ebaa491f8eda8c1e3f410fe9257851a45502891c71cc1793d5  shift.mp3
1277a1cf2a29a434d5bbda9d048a88dd6982fdfde77aa9b0b59fe5c4b8bf34a0  transfer.mp3
CHECKSUMS
cd "$root/core"
cargo run --locked -p orqwa-app --example prepare_ambient -- "$source_dir"
