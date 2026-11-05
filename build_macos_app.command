#!/usr/bin/env bash
cd "$(dirname "$0")"
./build_macos_app.sh
printf '\nPress Enter to close...'
read -r _
