#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: normalize-account-database.sh SHADOW_PATH FIXED_LAST_CHANGE_DAYS" >&2
  exit 2
fi

shadow_path=$1
fixed_last_change_days=$2

case "$fixed_last_change_days" in
  ''|*[!0-9]*) echo "fixed last-change day must be decimal" >&2; exit 2 ;;
esac
test -f "$shadow_path"
test ! -L "$shadow_path"

shadow_dir=$(dirname -- "$shadow_path")
shadow_name=$(basename -- "$shadow_path")
normalized_path="$shadow_dir/.$shadow_name.vela-normalized"
test ! -e "$normalized_path"
trap 'rm -f -- "$normalized_path"' EXIT HUP INT TERM
cp -p -- "$shadow_path" "$normalized_path"

LC_ALL=C awk -F: -v OFS=: -v fixed="$fixed_last_change_days" '
  BEGIN { seen = 0 }
  $1 == "participant" {
    if (seen != 0 || NF != 9 || $2 != "!" || $3 !~ /^[0-9]+$/ || $4 != "0" ||
        $5 != "99999" || $6 != "7" || $7 != "" || $8 != "" || $9 != "") {
      exit 21
    }
    $3 = fixed
    seen = 1
  }
  { print }
  END { if (seen != 1) exit 22 }
' "$shadow_path" > "$normalized_path"

mv -- "$normalized_path" "$shadow_path"
trap - EXIT HUP INT TERM

expected="participant:!:$fixed_last_change_days:0:99999:7:::"
test "$(LC_ALL=C awk -F: '$1 == "participant" { print }' "$shadow_path")" = "$expected"
