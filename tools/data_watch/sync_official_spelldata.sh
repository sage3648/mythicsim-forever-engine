#!/usr/bin/env bash
# Copies the official project's client-generated spell tables into our tree.
#
# wowsims/forever (MIT) generates sim/<class>/spell_data_auto_gen.go from the Forever beta client with
# tools/db2tool + tools/database/gen_spelldata. That pipeline reads a local WoW install (CASC) or
# pre-extracted .db2 files, so it cannot run here; we take its output instead. Same file paths as
# theirs, so Track B (forever-next) lines up. Only the module path changes.
#
# sim/common/shared/spell_data.go (the row types) is hand code, adapted once to our core; this script
# does not touch it.
#
#   tools/data_watch/sync_official_spelldata.sh [path to a wowsims/forever checkout]
set -euo pipefail

src=${1:-../wowsims-forever-official}
rev=$(git -C "$src" rev-parse --short=9 HEAD)
cd "$(git rev-parse --show-toplevel)"

vendor() {
	sed -e 's#github.com/wowsims/forever/#github.com/wowsims/classic/#g' \
		-e "1a // Vendored from wowsims/forever@$rev (MIT) by tools/data_watch/sync_official_spelldata.sh." \
		"$src/$1" >"$1"
}

vendor sim/common/shared/spell_data_enums_auto_gen.go
for f in "$src"/sim/*/spell_data_auto_gen.go; do
	vendor "sim/$(basename "$(dirname "$f")")/spell_data_auto_gen.go"
done
gofmt -l sim/common/shared sim/*/spell_data_auto_gen.go
echo "synced from wowsims/forever@$rev"
